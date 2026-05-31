// Test lane: default

use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationRequest {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub report_activation: bool,
    pub report_close: bool,
    pub buttons: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationTracking {
    Tracked,
    Untracked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationEvent {
    Activated {
        protocol_id: String,
        button: Option<u32>,
    },
    Closed {
        protocol_id: String,
    },
}

pub type NotificationCallback = Arc<dyn Fn(NotificationEvent) + Send + Sync + 'static>;

/// Request notification authorization from the OS.
/// On macOS this triggers the permission prompt on first call.
/// No-op on other platforms.
pub fn request_authorization() {
    #[cfg(target_os = "macos")]
    platform::request_authorization();
}

/// Send a desktop notification using the platform's native API.
///
/// - **macOS**: `UNUserNotificationCenter` (requires app bundle with identifier).
/// - **Linux**: D-Bus `org.freedesktop.Notifications`.
/// - **Windows**: Toast notifications via `windows` crate.
///
/// Spawns a background thread so the caller is never blocked.
pub fn send_notification(title: &str, body: &str) {
    let request = NotificationRequest {
        id: None,
        title: normalize_title(title),
        body: body.to_string(),
        report_activation: false,
        report_close: false,
        buttons: Vec::new(),
    };

    std::thread::spawn(move || {
        let _ = platform::notify(request, None);
    });
}

pub fn send_kitty_notification(
    request: NotificationRequest,
    callback: Option<NotificationCallback>,
) -> NotificationTracking {
    platform::notify(request, callback)
}

pub fn close_notification(id: &str) {
    platform::close(id);
}

fn normalize_title(title: &str) -> String {
    if title.is_empty() {
        "Rio".to_string()
    } else {
        title.to_string()
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use block2::RcBlock;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };
    use std::sync::Once;

    pub(crate) fn request_authorization() {
        static INIT: Once = Once::new();
        INIT.call_once(|| unsafe {
            let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
            if bundle.is_null() {
                return;
            }
            let bundle_id: *mut Object = msg_send![bundle, bundleIdentifier];
            if bundle_id.is_null() {
                return;
            }

            let center = UNUserNotificationCenter::currentNotificationCenter();
            center.requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::UNAuthorizationOptionAlert
                    | UNAuthorizationOptions::UNAuthorizationOptionSound,
                &RcBlock::new(|_ok: Bool, _err: *mut NSError| {}),
            );
        });
    }

    pub fn notify(
        request: super::NotificationRequest,
        _callback: Option<super::NotificationCallback>,
    ) -> super::NotificationTracking {
        unsafe {
            // UNUserNotificationCenter crashes if the app has no bundle
            // identifier (e.g. cargo run). Guard like Kitty does.
            let bundle: *mut Object = msg_send![class!(NSBundle), mainBundle];
            if bundle.is_null() {
                return super::NotificationTracking::Untracked;
            }
            let bundle_id: *mut Object = msg_send![bundle, bundleIdentifier];
            if bundle_id.is_null() {
                return super::NotificationTracking::Untracked;
            }

            let center = UNUserNotificationCenter::currentNotificationCenter();

            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(&request.title));
            content.setBody(&NSString::from_str(&request.body));

            let identifier =
                NSString::from_str(request.id.as_deref().unwrap_or("rio-notification"));
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &identifier,
                &content,
                None,
            );

            center.addNotificationRequest_withCompletionHandler(&request, None);
        }
        super::NotificationTracking::Untracked
    }

    pub fn close(_id: &str) {
        // Close-by-id and lifecycle callbacks need a macOS delegate layer.
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
mod platform {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    use zbus::blocking::{Connection, Proxy};

    use super::{
        NotificationCallback, NotificationEvent, NotificationRequest,
        NotificationTracking,
    };

    #[derive(Clone)]
    struct TrackedNotification {
        protocol_id: String,
        report_activation: bool,
        report_close: bool,
        callback: Option<NotificationCallback>,
    }

    #[derive(Default)]
    struct NotificationState {
        by_protocol_id: Mutex<HashMap<String, u32>>,
        by_os_id: Mutex<HashMap<u32, TrackedNotification>>,
    }

    static STATE: OnceLock<Arc<NotificationState>> = OnceLock::new();

    fn state() -> Arc<NotificationState> {
        STATE
            .get_or_init(|| {
                let state = Arc::new(NotificationState::default());
                start_signal_listener(Arc::clone(&state));
                state
            })
            .clone()
    }

    fn notification_proxy(connection: &Connection) -> zbus::Result<Proxy<'_>> {
        Proxy::new(
            connection,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
    }

    pub fn notify(
        request: NotificationRequest,
        callback: Option<NotificationCallback>,
    ) -> NotificationTracking {
        let state = state();
        let replaces_id = request
            .id
            .as_ref()
            .and_then(|id| state.by_protocol_id.lock().ok()?.get(id).copied())
            .unwrap_or(0);

        let Ok(connection) = Connection::session() else {
            return NotificationTracking::Untracked;
        };
        let Ok(proxy) = notification_proxy(&connection) else {
            return NotificationTracking::Untracked;
        };

        let actions = notification_actions(&request);
        let action_refs = actions.iter().map(String::as_str).collect::<Vec<_>>();
        let hints: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
        let Ok(os_id) = proxy.call::<_, _, u32>(
            "Notify",
            &(
                "Rio",          // app_name
                replaces_id,    // replaces_id
                "rio",          // app_icon
                &request.title, // summary
                &request.body,  // body
                &action_refs,   // actions
                &hints,         // hints
                -1i32,          // expire_timeout
            ),
        ) else {
            return NotificationTracking::Untracked;
        };

        if let Some(id) = &request.id {
            if let Ok(mut by_protocol_id) = state.by_protocol_id.lock() {
                by_protocol_id.insert(id.clone(), os_id);
            }
        }

        if request.report_activation || request.report_close {
            let protocol_id = request.id.unwrap_or_else(|| "0".to_string());
            let tracked = TrackedNotification {
                protocol_id,
                report_activation: request.report_activation,
                report_close: request.report_close,
                callback,
            };
            if let Ok(mut by_os_id) = state.by_os_id.lock() {
                by_os_id.insert(os_id, tracked);
            }
        }

        NotificationTracking::Tracked
    }

    pub fn close(id: &str) {
        let state = state();
        let os_id = state
            .by_protocol_id
            .lock()
            .ok()
            .and_then(|mut by_protocol_id| by_protocol_id.remove(id));
        let Some(os_id) = os_id else {
            return;
        };
        if let Ok(mut by_os_id) = state.by_os_id.lock() {
            by_os_id.remove(&os_id);
        }
        if let Ok(connection) = Connection::session() {
            if let Ok(proxy) = notification_proxy(&connection) {
                let _: Result<(), _> = proxy.call("CloseNotification", &(os_id,));
            }
        }
    }

    fn notification_actions(request: &NotificationRequest) -> Vec<String> {
        if !request.report_activation {
            return Vec::new();
        }

        let mut actions = vec!["default".to_string(), "Activate".to_string()];
        for (index, button) in request.buttons.iter().enumerate() {
            actions.push((index + 1).to_string());
            actions.push(button.clone());
        }
        actions
    }

    fn start_signal_listener(state: Arc<NotificationState>) {
        std::thread::spawn(move || {
            let Ok(connection) = Connection::session() else {
                return;
            };
            let Ok(proxy) = notification_proxy(&connection) else {
                return;
            };
            let Ok(mut signals) = proxy.receive_all_signals() else {
                return;
            };

            for message in &mut signals {
                let Some(member) = message
                    .header()
                    .member()
                    .map(|member| member.as_str().to_owned())
                else {
                    continue;
                };

                match member.as_str() {
                    "ActionInvoked" => {
                        if let Ok((os_id, action)) =
                            message.body().deserialize::<(u32, String)>()
                        {
                            handle_action_invoked(&state, os_id, action);
                        }
                    }
                    "NotificationClosed" => {
                        if let Ok((os_id, _reason)) =
                            message.body().deserialize::<(u32, u32)>()
                        {
                            handle_notification_closed(&state, os_id);
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn handle_action_invoked(state: &NotificationState, os_id: u32, action: String) {
        let tracked = state
            .by_os_id
            .lock()
            .ok()
            .and_then(|by_os_id| by_os_id.get(&os_id).cloned());
        let Some(tracked) = tracked else {
            return;
        };
        if !tracked.report_activation {
            return;
        }
        let Some(callback) = tracked.callback else {
            return;
        };
        let button = action.parse::<u32>().ok();
        callback(NotificationEvent::Activated {
            protocol_id: tracked.protocol_id,
            button,
        });
    }

    fn handle_notification_closed(state: &NotificationState, os_id: u32) {
        let tracked = state
            .by_os_id
            .lock()
            .ok()
            .and_then(|mut by_os_id| by_os_id.remove(&os_id));
        let Some(tracked) = tracked else {
            return;
        };
        if tracked.protocol_id != "0" {
            if let Ok(mut by_protocol_id) = state.by_protocol_id.lock() {
                by_protocol_id.remove(&tracked.protocol_id);
            }
        }
        if !tracked.report_close {
            return;
        }
        let Some(callback) = tracked.callback else {
            return;
        };
        callback(NotificationEvent::Closed {
            protocol_id: tracked.protocol_id,
        });
    }

    #[cfg(test)]
    mod tests {
        use std::sync::mpsc;

        use super::*;

        #[test]
        // Defends: freedesktop notification actions expose default activation and 1-based button ids.
        fn notification_actions_include_default_and_numbered_buttons() {
            let actions = notification_actions(&NotificationRequest {
                id: Some("job".to_string()),
                title: "Done".to_string(),
                body: String::new(),
                report_activation: true,
                report_close: false,
                buttons: vec!["Open".to_string(), "Ignore".to_string()],
            });

            assert_eq!(actions, ["default", "Activate", "1", "Open", "2", "Ignore"]);
        }

        #[test]
        // Defends: D-Bus ActionInvoked signals are translated into OSC 99 activation callbacks.
        fn action_invoked_routes_protocol_id_and_button() {
            let state = NotificationState::default();
            let (tx, rx) = mpsc::channel();
            state.by_os_id.lock().unwrap().insert(
                7,
                TrackedNotification {
                    protocol_id: "job".to_string(),
                    report_activation: true,
                    report_close: false,
                    callback: Some(Arc::new(move |event| tx.send(event).unwrap())),
                },
            );

            handle_action_invoked(&state, 7, "2".to_string());

            assert_eq!(
                rx.recv().unwrap(),
                NotificationEvent::Activated {
                    protocol_id: "job".to_string(),
                    button: Some(2),
                }
            );
        }

        #[test]
        // Defends: D-Bus NotificationClosed signals clear tracked handles and emit close callbacks.
        fn notification_closed_clears_maps_and_routes_callback() {
            let state = NotificationState::default();
            let (tx, rx) = mpsc::channel();
            state
                .by_protocol_id
                .lock()
                .unwrap()
                .insert("job".to_string(), 7);
            state.by_os_id.lock().unwrap().insert(
                7,
                TrackedNotification {
                    protocol_id: "job".to_string(),
                    report_activation: false,
                    report_close: true,
                    callback: Some(Arc::new(move |event| tx.send(event).unwrap())),
                },
            );

            handle_notification_closed(&state, 7);

            assert_eq!(
                rx.recv().unwrap(),
                NotificationEvent::Closed {
                    protocol_id: "job".to_string(),
                }
            );
            assert!(state.by_protocol_id.lock().unwrap().is_empty());
            assert!(state.by_os_id.lock().unwrap().is_empty());
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    pub fn notify(
        request: super::NotificationRequest,
        _callback: Option<super::NotificationCallback>,
    ) -> super::NotificationTracking {
        use windows::core::HSTRING;
        use windows::Data::Xml::Dom::XmlDocument;
        use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

        let Ok(xml) = XmlDocument::new() else {
            return super::NotificationTracking::Untracked;
        };
        let toast_xml = format!(
            r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
            request.title, request.body,
        );
        if xml.LoadXml(&HSTRING::from(&toast_xml)).is_err() {
            return super::NotificationTracking::Untracked;
        }
        let Ok(toast) = ToastNotification::CreateToastNotification(&xml) else {
            return super::NotificationTracking::Untracked;
        };
        let Ok(notifier) =
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("Rio"))
        else {
            return super::NotificationTracking::Untracked;
        };
        let _ = notifier.Show(&toast);
        super::NotificationTracking::Untracked
    }

    pub fn close(_id: &str) {
        // Close-by-id and lifecycle callbacks need Windows toast event wiring.
    }
}

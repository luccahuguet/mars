use rio_window::event::ElementState;
use rio_window::keyboard::{Key, ModifiersState, NamedKey, PhysicalKey};
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CapturedKeyOwner {
    Route,
    Screen,
    Terminal(usize),
}

#[derive(Clone, Debug)]
enum CapturedKey<R> {
    Route,
    Screen,
    Terminal {
        route_id: usize,
        release: R,
        modifier: Option<ModifiersState>,
    },
}

impl<R> CapturedKey<R> {
    fn owner(&self) -> CapturedKeyOwner {
        match self {
            Self::Route => CapturedKeyOwner::Route,
            Self::Screen => CapturedKeyOwner::Screen,
            Self::Terminal { route_id, .. } => CapturedKeyOwner::Terminal(*route_id),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScreenKeyOwner {
    Fresh,
    Route,
    Screen,
    Terminal(usize),
    UnownedFollowup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FocusLossRelease<R> {
    pub(crate) route_id: usize,
    pub(crate) event: R,
    pub(crate) modifiers: ModifiersState,
}

#[derive(Debug)]
pub(crate) struct KeyLifecycle<R> {
    captured: FxHashMap<PhysicalKey, CapturedKey<R>>,
    last_modifiers: ModifiersState,
    pressed_modifiers: FxHashMap<PhysicalKey, ModifiersState>,
}

impl<R> Default for KeyLifecycle<R> {
    fn default() -> Self {
        Self {
            captured: FxHashMap::default(),
            last_modifiers: ModifiersState::empty(),
            pressed_modifiers: FxHashMap::default(),
        }
    }
}

impl<R> KeyLifecycle<R> {
    pub(crate) fn route_owns_event(
        &mut self,
        key: PhysicalKey,
        state: ElementState,
        repeat: bool,
    ) -> bool {
        let fresh = state == ElementState::Pressed && !repeat;
        if fresh {
            self.captured.remove(&key);
            self.captured.insert(key, CapturedKey::Route);
            return true;
        }

        let route_owned = self
            .captured
            .get(&key)
            .is_some_and(|capture| capture.owner() == CapturedKeyOwner::Route);
        if route_owned && state == ElementState::Released {
            self.captured.remove(&key);
        }
        route_owned
    }

    pub(crate) fn screen_owner(
        &mut self,
        key: PhysicalKey,
        state: ElementState,
        repeat: bool,
    ) -> ScreenKeyOwner {
        let fresh = state == ElementState::Pressed && !repeat;
        let owner = if fresh {
            self.captured.remove(&key);
            None
        } else {
            self.captured.get(&key).map(CapturedKey::owner)
        };
        if state == ElementState::Released {
            self.captured.remove(&key);
        }

        match owner {
            Some(CapturedKeyOwner::Route) => ScreenKeyOwner::Route,
            Some(CapturedKeyOwner::Screen) => ScreenKeyOwner::Screen,
            Some(CapturedKeyOwner::Terminal(route_id)) => {
                ScreenKeyOwner::Terminal(route_id)
            }
            None if fresh => ScreenKeyOwner::Fresh,
            None => ScreenKeyOwner::UnownedFollowup,
        }
    }

    pub(crate) fn capture_screen(&mut self, key: PhysicalKey) {
        self.captured.insert(key, CapturedKey::Screen);
    }

    pub(crate) fn capture_terminal(
        &mut self,
        key: PhysicalKey,
        logical_key: &Key,
        route_id: usize,
        release: R,
    ) {
        self.captured.insert(
            key,
            CapturedKey::Terminal {
                route_id,
                release,
                modifier: modifier_for_key(logical_key),
            },
        );
    }

    pub(crate) fn track_modifiers(
        &mut self,
        window_modifiers: ModifiersState,
        physical_key: PhysicalKey,
        logical_key: &Key,
        state: ElementState,
        is_synthetic: bool,
    ) {
        let Some(mut modifiers) = modifier_tracking_base(
            self.last_modifiers,
            window_modifiers,
            is_synthetic,
            state,
        ) else {
            return;
        };
        let logical_modifier = modifier_for_key(logical_key).filter(|m| !m.is_empty());
        let key_modifier = match state {
            ElementState::Pressed => logical_modifier.inspect(|&modifier| {
                self.pressed_modifiers.insert(physical_key, modifier);
            }),
            ElementState::Released => self
                .pressed_modifiers
                .remove(&physical_key)
                .or(logical_modifier),
        };
        if let Some(modifier) = key_modifier {
            let pressed = state == ElementState::Pressed
                || self
                    .pressed_modifiers
                    .values()
                    .any(|pressed| *pressed == modifier);
            modifiers.set(modifier, pressed);
        }
        self.last_modifiers = modifiers;
    }

    pub(crate) fn drain_focus_loss(&mut self) -> Vec<FocusLossRelease<R>> {
        let mut releases: Vec<_> = self
            .captured
            .drain()
            .filter_map(|(physical_key, captured)| match captured {
                CapturedKey::Terminal {
                    route_id,
                    release,
                    modifier,
                } => Some((physical_key, route_id, release, modifier)),
                CapturedKey::Route | CapturedKey::Screen => None,
            })
            .collect();
        releases.sort_by_key(|(key, _, _, modifier)| (modifier.is_some(), *key));

        let mut modifiers = self.last_modifiers;
        let releases = releases
            .into_iter()
            .map(|(physical_key, route_id, event, modifier)| {
                if let Some(modifier) = modifier.filter(|m| !m.is_empty()) {
                    self.pressed_modifiers.remove(&physical_key);
                    let pressed = self
                        .pressed_modifiers
                        .values()
                        .any(|pressed| *pressed == modifier);
                    modifiers.set(modifier, pressed);
                }
                FocusLossRelease {
                    route_id,
                    event,
                    modifiers,
                }
            })
            .collect();

        self.last_modifiers = ModifiersState::empty();
        self.pressed_modifiers.clear();
        releases
    }
}

fn modifier_tracking_base(
    retained_modifiers: ModifiersState,
    window_modifiers: ModifiersState,
    is_synthetic: bool,
    state: ElementState,
) -> Option<ModifiersState> {
    if is_synthetic {
        (state == ElementState::Pressed).then_some(retained_modifiers)
    } else {
        Some(window_modifiers)
    }
}

fn modifier_for_key(key: &Key) -> Option<ModifiersState> {
    match key {
        Key::Named(NamedKey::Shift) => Some(ModifiersState::SHIFT),
        Key::Named(NamedKey::Control) => Some(ModifiersState::CONTROL),
        Key::Named(NamedKey::Alt) => Some(ModifiersState::ALT),
        Key::Named(NamedKey::Super) => Some(ModifiersState::SUPER),
        Key::Named(NamedKey::Hyper | NamedKey::Meta) => Some(ModifiersState::empty()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rio_window::keyboard::KeyCode;

    fn physical(key: KeyCode) -> PhysicalKey {
        PhysicalKey::Code(key)
    }

    #[test]
    fn route_capture_owns_repeats_and_one_release() {
        let key = physical(KeyCode::KeyA);
        let mut keys = KeyLifecycle::<()>::default();

        assert!(keys.route_owns_event(key, ElementState::Pressed, false));
        assert!(keys.route_owns_event(key, ElementState::Pressed, true));
        assert!(keys.route_owns_event(key, ElementState::Released, false));
        assert!(!keys.route_owns_event(key, ElementState::Released, false));
    }

    #[test]
    fn captured_owner_survives_current_owner_changes() {
        let screen_key = physical(KeyCode::KeyA);
        let terminal_key = physical(KeyCode::KeyB);
        let mut keys = KeyLifecycle::default();
        keys.capture_screen(screen_key);
        keys.capture_terminal(terminal_key, &Key::Character("b".into()), 17, "release-b");

        assert_eq!(
            keys.screen_owner(screen_key, ElementState::Pressed, true),
            ScreenKeyOwner::Screen
        );
        assert_eq!(
            keys.screen_owner(screen_key, ElementState::Released, false),
            ScreenKeyOwner::Screen
        );
        assert_eq!(
            keys.screen_owner(screen_key, ElementState::Released, false),
            ScreenKeyOwner::UnownedFollowup
        );
        assert_eq!(
            keys.screen_owner(terminal_key, ElementState::Pressed, true),
            ScreenKeyOwner::Terminal(17)
        );
        assert_eq!(
            keys.screen_owner(terminal_key, ElementState::Released, false),
            ScreenKeyOwner::Terminal(17)
        );
        assert_eq!(
            keys.screen_owner(terminal_key, ElementState::Released, false),
            ScreenKeyOwner::UnownedFollowup
        );
    }

    #[test]
    fn fresh_press_replaces_stale_capture() {
        let key = physical(KeyCode::KeyA);
        let mut keys = KeyLifecycle::<()>::default();
        keys.capture_screen(key);

        assert_eq!(
            keys.screen_owner(key, ElementState::Pressed, false),
            ScreenKeyOwner::Fresh
        );
        assert_eq!(
            keys.screen_owner(key, ElementState::Released, false),
            ScreenKeyOwner::UnownedFollowup
        );
    }

    #[test]
    fn focus_loss_releases_terminal_keys_before_modifiers_and_clears_all() {
        let shift_left = physical(KeyCode::ShiftLeft);
        let shift_right = physical(KeyCode::ShiftRight);
        let alt_left = physical(KeyCode::AltLeft);
        let key_a = physical(KeyCode::KeyA);
        let shift = Key::Named(NamedKey::Shift);
        let alt = Key::Named(NamedKey::Alt);
        let mut keys = KeyLifecycle::default();

        keys.track_modifiers(
            ModifiersState::SHIFT,
            shift_left,
            &shift,
            ElementState::Pressed,
            false,
        );
        keys.track_modifiers(
            ModifiersState::SHIFT,
            shift_right,
            &shift,
            ElementState::Pressed,
            false,
        );
        keys.track_modifiers(
            ModifiersState::SHIFT,
            shift_left,
            &shift,
            ElementState::Released,
            false,
        );
        keys.track_modifiers(
            ModifiersState::SHIFT | ModifiersState::ALT,
            alt_left,
            &alt,
            ElementState::Pressed,
            false,
        );

        keys.capture_screen(physical(KeyCode::KeyZ));
        assert!(keys.route_owns_event(
            physical(KeyCode::KeyY),
            ElementState::Pressed,
            false
        ));
        keys.capture_terminal(key_a, &Key::Character("a".into()), 3, "a");
        keys.capture_terminal(shift_right, &shift, 3, "shift");
        keys.capture_terminal(alt_left, &alt, 3, "alt");

        let releases = keys.drain_focus_loss();
        assert_eq!(releases.len(), 3);
        assert_eq!(releases[0].event, "a");
        assert_eq!(
            releases[0].modifiers,
            ModifiersState::SHIFT | ModifiersState::ALT
        );
        assert!(releases[1..]
            .iter()
            .all(|release| { release.event == "shift" || release.event == "alt" }));
        assert_eq!(releases.last().unwrap().modifiers, ModifiersState::empty());
        assert!(keys.drain_focus_loss().is_empty());
    }

    #[test]
    fn physical_release_clears_a_modifier_after_logical_layout_change() {
        let control = physical(KeyCode::ControlLeft);
        let mut keys = KeyLifecycle::<()>::default();
        keys.track_modifiers(
            ModifiersState::CONTROL,
            control,
            &Key::Named(NamedKey::Control),
            ElementState::Pressed,
            false,
        );
        keys.track_modifiers(
            ModifiersState::empty(),
            control,
            &Key::Character("changed-layout".into()),
            ElementState::Released,
            false,
        );

        assert_eq!(keys.last_modifiers, ModifiersState::empty());
        assert!(keys.pressed_modifiers.is_empty());
    }

    #[test]
    fn synthetic_focus_presses_accumulate_from_retained_modifiers() {
        let control = physical(KeyCode::ControlLeft);
        let shift = physical(KeyCode::ShiftRight);
        let mut keys = KeyLifecycle::<()>::default();
        keys.track_modifiers(
            ModifiersState::CONTROL,
            control,
            &Key::Named(NamedKey::Control),
            ElementState::Pressed,
            false,
        );
        keys.track_modifiers(
            ModifiersState::empty(),
            shift,
            &Key::Named(NamedKey::Shift),
            ElementState::Pressed,
            true,
        );
        keys.track_modifiers(
            ModifiersState::empty(),
            shift,
            &Key::Named(NamedKey::Shift),
            ElementState::Released,
            true,
        );

        assert_eq!(
            keys.last_modifiers,
            ModifiersState::CONTROL | ModifiersState::SHIFT
        );
        assert_eq!(keys.pressed_modifiers.len(), 2);
    }
}

mod key_lifecycle;
mod keyboard;
mod link_gesture;
mod pointer;

use crate::hints::HintMatch;
use key_lifecycle::KeyLifecycle;
use link_gesture::LinkGesture;
use rio_backend::crosswords::pos::Pos;
use rio_window::event::{ElementState, KeyEvent};
use rio_window::keyboard::ModifiersState;

pub(crate) use key_lifecycle::{FocusLossRelease, ScreenKeyOwner};
pub(crate) use link_gesture::LinkRelease;
pub(crate) use pointer::{apply_modifiers, PointerOwner};

/// Mars-owned cross-event input state. Keep Rio integration to one Screen field.
#[derive(Debug, Default)]
pub(crate) struct MarsInputState {
    keys: KeyLifecycle<KeyEvent>,
    link_gesture: LinkGesture<HintMatch>,
}

impl MarsInputState {
    pub(crate) fn track_key_event(
        &mut self,
        modifiers: ModifiersState,
        event: &KeyEvent,
        is_synthetic: bool,
    ) {
        self.keys.track_modifiers(
            modifiers,
            event.physical_key,
            &event.logical_key,
            event.state,
            is_synthetic,
        );
    }

    pub(crate) fn route_owns_key_event(&mut self, event: &KeyEvent) -> bool {
        self.keys
            .route_owns_event(event.physical_key, event.state, event.repeat)
    }

    pub(crate) fn screen_key_owner(&mut self, event: &KeyEvent) -> ScreenKeyOwner {
        self.keys
            .screen_owner(event.physical_key, event.state, event.repeat)
    }

    pub(crate) fn capture_screen_key(&mut self, event: &KeyEvent) {
        self.keys.capture_screen(event.physical_key);
    }

    pub(crate) fn capture_terminal_key(&mut self, event: &KeyEvent, route_id: usize) {
        let mut release = event.clone();
        release.state = ElementState::Released;
        release.repeat = false;
        self.keys.capture_terminal(
            event.physical_key,
            &event.logical_key,
            route_id,
            release,
        );
    }

    pub(crate) fn drain_focus_loss_keys(&mut self) -> Vec<FocusLossRelease<KeyEvent>> {
        self.keys.drain_focus_loss()
    }

    pub(crate) fn begin_left_press(&mut self) {
        self.link_gesture.begin_press();
    }

    pub(crate) fn start_link(&mut self, origin: Pos, hint: HintMatch) {
        self.link_gesture.start(origin, hint);
    }

    pub(crate) fn consume_left_press(&mut self) {
        self.link_gesture.consume_press();
    }

    pub(crate) fn cancel_link(&mut self) {
        self.link_gesture.cancel();
    }

    pub(crate) fn cancel_link_if_moved(&mut self, position: Pos) {
        self.link_gesture.cancel_if_moved(position);
    }

    pub(crate) fn finish_link(&mut self) -> LinkRelease<HintMatch> {
        self.link_gesture.finish()
    }
}

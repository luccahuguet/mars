mod link_gesture;

use crate::hints::HintMatch;
use link_gesture::LinkGesture;
use rio_backend::crosswords::pos::Pos;

pub(crate) use link_gesture::LinkRelease;

/// Mars-owned cross-event input state. Keep Rio integration to one Screen field.
#[derive(Debug, Default)]
pub(crate) struct MarsInputState {
    link_gesture: LinkGesture<HintMatch>,
}

impl MarsInputState {
    pub(crate) fn begin_left_press(&mut self) {
        self.link_gesture.begin_press();
    }

    pub(crate) fn start_link(&mut self, origin: Pos, hint: HintMatch) {
        self.link_gesture.start(origin, hint);
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

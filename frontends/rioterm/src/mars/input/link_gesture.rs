use rio_backend::crosswords::pos::Pos;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LinkRelease<T> {
    NotOwned,
    Cancelled,
    Activate { origin: Pos, target: T },
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum LinkGesture<T> {
    #[default]
    Idle,
    Pending {
        origin: Pos,
        target: T,
    },
    /// Activation is cancelled, but the consumed press still owns its release.
    Cancelled,
}

impl<T> LinkGesture<T> {
    pub(super) fn begin_press(&mut self) {
        *self = Self::Idle;
    }

    pub(super) fn start(&mut self, origin: Pos, target: T) {
        *self = Self::Pending { origin, target };
    }

    pub(super) fn cancel(&mut self) {
        if matches!(self, Self::Pending { .. }) {
            *self = Self::Cancelled;
        }
    }

    pub(super) fn cancel_if_moved(&mut self, position: Pos) {
        if matches!(self, Self::Pending { origin, .. } if *origin != position) {
            self.cancel();
        }
    }

    pub(super) fn finish(&mut self) -> LinkRelease<T> {
        match std::mem::take(self) {
            Self::Idle => LinkRelease::NotOwned,
            Self::Cancelled => LinkRelease::Cancelled,
            Self::Pending { origin, target } => LinkRelease::Activate { origin, target },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rio_backend::crosswords::pos::{Column, Line};

    fn pos(column: usize) -> Pos {
        Pos::new(Line(2), Column(column))
    }

    #[test]
    fn eligible_release_returns_the_pressed_target() {
        let mut gesture = LinkGesture::default();
        gesture.start(pos(3), "pressed");

        assert_eq!(
            gesture.finish(),
            LinkRelease::Activate {
                origin: pos(3),
                target: "pressed"
            }
        );
        assert_eq!(gesture.finish(), LinkRelease::NotOwned);
    }

    #[test]
    fn movement_cancels_activation_but_keeps_release_ownership() {
        let mut gesture = LinkGesture::default();
        gesture.start(pos(3), "pressed");
        gesture.cancel_if_moved(pos(4));
        gesture.cancel_if_moved(pos(3));

        assert_eq!(gesture.finish(), LinkRelease::Cancelled);
        assert_eq!(gesture.finish(), LinkRelease::NotOwned);
    }

    #[test]
    fn movement_inside_the_press_cell_stays_pending() {
        let mut gesture = LinkGesture::default();
        gesture.start(pos(3), "pressed");
        gesture.cancel_if_moved(pos(3));

        assert!(matches!(
            gesture.finish(),
            LinkRelease::Activate {
                target: "pressed",
                ..
            }
        ));
    }

    #[test]
    fn hard_cancel_is_idempotent_and_owns_one_release() {
        let mut gesture = LinkGesture::default();
        gesture.start(pos(3), "pressed");
        gesture.cancel();
        gesture.cancel();

        assert_eq!(gesture.finish(), LinkRelease::Cancelled);
        assert_eq!(gesture.finish(), LinkRelease::NotOwned);
    }

    #[test]
    fn a_new_press_discards_stale_state() {
        let mut gesture = LinkGesture::default();
        gesture.start(pos(3), "stale");
        gesture.cancel();
        gesture.begin_press();
        gesture.start(pos(7), "fresh");

        assert_eq!(
            gesture.finish(),
            LinkRelease::Activate {
                origin: pos(7),
                target: "fresh"
            }
        );
    }
}

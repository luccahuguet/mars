use crate::router::Route;
use crate::screen::Screen;
use rio_backend::clipboard::Clipboard;
use rio_window::event::{ElementState, Modifiers, MouseButton};
use rio_window::window::CursorIcon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PointerOwner {
    Terminal,
    Assistant,
    CommandPalette,
    ConfirmQuit,
    OtherRoute,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PointerSnapshot {
    pub(crate) route_is_terminal: bool,
    pub(crate) assistant_active: bool,
    pub(crate) command_palette_active: bool,
    pub(crate) confirm_quit_active: bool,
}

/// Match the current Rio render stack: confirm quit is topmost, followed by
/// command palette, assistant, nonterminal route content, then the terminal.
pub(crate) fn select_pointer_owner(snapshot: PointerSnapshot) -> PointerOwner {
    if snapshot.confirm_quit_active {
        PointerOwner::ConfirmQuit
    } else if snapshot.command_palette_active {
        PointerOwner::CommandPalette
    } else if snapshot.assistant_active {
        PointerOwner::Assistant
    } else if !snapshot.route_is_terminal {
        PointerOwner::OtherRoute
    } else {
        PointerOwner::Terminal
    }
}

pub(crate) fn apply_modifiers(route: &mut Route<'_>, modifiers: Modifiers) {
    let (cursor, should_redraw) = {
        let screen = &mut route.window.screen;
        screen.set_modifiers(modifiers);
        let should_redraw = screen.update_highlighted_hints();
        let cursor = screen.mouse.last_cell.is_some().then(|| {
            terminal_pointer_icon(
                screen.has_highlighted_hint(),
                screen.modifiers.state().shift_key(),
                screen.mouse_mode(),
            )
        });
        (cursor, should_redraw)
    };

    if let Some(cursor) = cursor {
        route.window.winit_window.set_cursor(cursor);
    }
    if should_redraw {
        route.request_redraw();
    }
}

fn terminal_pointer_icon(
    has_highlighted_hint: bool,
    shift_pressed: bool,
    mouse_mode: bool,
) -> CursorIcon {
    match (has_highlighted_hint, shift_pressed, mouse_mode) {
        (true, _, _) => CursorIcon::Pointer,
        (_, false, true) => CursorIcon::Default,
        _ => CursorIcon::Text,
    }
}

impl Screen<'_> {
    pub fn pointer_owner(&self, route_is_terminal: bool) -> PointerOwner {
        select_pointer_owner(PointerSnapshot {
            route_is_terminal,
            assistant_active: self.renderer.assistant.is_active(),
            command_palette_active: self.renderer.command_palette.is_enabled(),
            confirm_quit_active: self.renderer.confirm_quit.is_active(),
        })
    }

    pub fn update_mouse_button_state(
        &mut self,
        button: MouseButton,
        state: ElementState,
    ) {
        match button {
            MouseButton::Left => self.mouse.left_button_state = state,
            MouseButton::Middle => self.mouse.middle_button_state = state,
            MouseButton::Right => self.mouse.right_button_state = state,
            _ => (),
        }
    }

    /// Cancel terminal/chrome interactions when a higher surface owns input.
    pub fn cancel_pointer_interactions(&mut self) -> bool {
        self.cancel_link_gesture();
        self.take_chrome_press();
        self.mouse.left_button_state = ElementState::Released;
        self.mouse.middle_button_state = ElementState::Released;
        self.mouse.right_button_state = ElementState::Released;
        self.mouse.on_border = false;

        let mut changed = self.resize_state.take().is_some();
        if self.renderer.scrollbar.is_dragging() {
            self.renderer.scrollbar.end_drag();
            changed = true;
        }
        if let Some(island) = &mut self.renderer.island {
            if island.is_dragging() {
                island.cancel_drag();
                changed = true;
            }
        }
        if changed {
            self.mark_dirty();
        }
        changed
    }

    /// Dispatch input for full-surface overlays and swallow it before it can
    /// reach terminal selection, mouse reporting, borders, or chrome.
    pub fn handle_pointer_overlay_mouse_input(
        &mut self,
        owner: PointerOwner,
        button: MouseButton,
        state: ElementState,
        clipboard: &mut Clipboard,
    ) -> bool {
        self.update_mouse_button_state(button, state);
        if button != MouseButton::Left {
            return false;
        }

        match state {
            ElementState::Pressed => {
                self.consume_left_press();
                match owner {
                    PointerOwner::Assistant => self.handle_assistant_click(),
                    PointerOwner::CommandPalette => self.handle_palette_click(clipboard),
                    _ => false,
                }
            }
            ElementState::Released => {
                self.finish_hint_click(clipboard);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_owner_follows_the_render_stack() {
        let cases = [
            (
                PointerSnapshot {
                    route_is_terminal: true,
                    assistant_active: true,
                    command_palette_active: true,
                    confirm_quit_active: true,
                },
                PointerOwner::ConfirmQuit,
            ),
            (
                PointerSnapshot {
                    route_is_terminal: true,
                    assistant_active: true,
                    command_palette_active: true,
                    confirm_quit_active: false,
                },
                PointerOwner::CommandPalette,
            ),
            (
                PointerSnapshot {
                    route_is_terminal: true,
                    assistant_active: true,
                    ..PointerSnapshot::default()
                },
                PointerOwner::Assistant,
            ),
            (PointerSnapshot::default(), PointerOwner::OtherRoute),
            (
                PointerSnapshot {
                    route_is_terminal: true,
                    ..PointerSnapshot::default()
                },
                PointerOwner::Terminal,
            ),
        ];

        for (snapshot, expected) in cases {
            assert_eq!(select_pointer_owner(snapshot), expected);
        }
    }

    #[test]
    fn terminal_cursor_follows_hint_shift_and_mouse_mode() {
        let cases = [
            ((true, false, false), CursorIcon::Pointer),
            ((false, false, true), CursorIcon::Default),
            ((false, true, true), CursorIcon::Text),
            ((false, false, false), CursorIcon::Text),
        ];

        for ((hint, shift, mouse_mode), expected) in cases {
            assert_eq!(terminal_pointer_icon(hint, shift, mouse_mode), expected);
        }
    }
}

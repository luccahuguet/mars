use crate::router::{routes::RoutePath, Route};
use crate::screen::Screen;
use rio_backend::clipboard::{Clipboard, ClipboardType};
use rio_window::event::{ElementState, Ime, KeyEvent};
use rio_window::keyboard::{Key, ModifiersState, NamedKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyInputOwner {
    Assistant,
    CommandPalette,
    ConfirmQuit,
    Welcome,
    IslandRename,
    Hint,
    Search,
    Terminal,
}

impl KeyInputOwner {
    fn is_route_owned(self) -> bool {
        !matches!(self, Self::Hint | Self::Search | Self::Terminal)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct KeyInputSnapshot {
    confirm_quit_active: bool,
    command_palette_active: bool,
    assistant_active: bool,
    route_is_terminal: bool,
    island_rename_active: bool,
    hint_active: bool,
    search_active: bool,
}

/// Match the current Rio render/input stack without depending on renderer
/// types. Partial terminal UI (rename, hints, search) follows full overlays.
fn select_key_input_owner(snapshot: KeyInputSnapshot) -> KeyInputOwner {
    if snapshot.confirm_quit_active {
        KeyInputOwner::ConfirmQuit
    } else if snapshot.command_palette_active {
        KeyInputOwner::CommandPalette
    } else if snapshot.assistant_active {
        KeyInputOwner::Assistant
    } else if !snapshot.route_is_terminal {
        KeyInputOwner::Welcome
    } else if snapshot.island_rename_active {
        KeyInputOwner::IslandRename
    } else if snapshot.hint_active {
        KeyInputOwner::Hint
    } else if snapshot.search_active {
        KeyInputOwner::Search
    } else {
        KeyInputOwner::Terminal
    }
}

fn printable_text_input(text: Option<&str>) -> Option<&str> {
    text.filter(|text| !text.is_empty() && text.chars().all(|c| !c.is_control()))
}

fn accepted_text_input(text: Option<&str>, modifiers: ModifiersState) -> Option<&str> {
    printable_text_input(text).filter(|_| {
        !modifiers.super_key() && (!modifiers.control_key() || modifiers.alt_key())
    })
}

impl Screen<'_> {
    pub(crate) fn clear_ime_preedit(&mut self) -> bool {
        let ime = &mut self.context_manager.current_mut().ime;
        let changed = ime.preedit().is_some();
        if changed {
            ime.set_preedit(None);
        }
        changed
    }

    fn update_owned_ime_state(&mut self, event: &Ime) -> bool {
        let changed = self.clear_ime_preedit();
        let ime = &mut self.context_manager.current_mut().ime;
        match event {
            Ime::Enabled => ime.set_enabled(true),
            Ime::Disabled => ime.set_enabled(false),
            Ime::Preedit(..) | Ime::Commit(_) => {}
        }
        changed
    }
}

impl Route<'_> {
    pub(crate) fn key_input_owner(&self) -> KeyInputOwner {
        let screen = &self.window.screen;
        select_key_input_owner(KeyInputSnapshot {
            confirm_quit_active: screen.renderer.confirm_quit.is_active(),
            command_palette_active: screen.renderer.command_palette.is_enabled(),
            assistant_active: screen.renderer.assistant.is_active(),
            route_is_terminal: self.path == RoutePath::Terminal,
            island_rename_active: screen
                .renderer
                .island
                .as_ref()
                .is_some_and(|island| island.is_color_picker_open()),
            hint_active: screen.hint_state.is_active(),
            search_active: screen.search_active(),
        })
    }

    fn activate_command_palette(&mut self, clipboard: &mut Clipboard) {
        let selected_font = self
            .window
            .screen
            .renderer
            .command_palette
            .get_selected_font();
        let selected_action = self
            .window
            .screen
            .renderer
            .command_palette
            .get_selected_action();
        use crate::renderer::command_palette::PaletteAction;

        if let Some(font) = selected_font {
            clipboard.set(ClipboardType::Clipboard, font);
            self.window
                .screen
                .renderer
                .command_palette
                .set_enabled(false);
            self.request_overlay_redraw();
            return;
        }

        match selected_action {
            Some(PaletteAction::ListFonts) => {
                let fonts = self.window.screen.sugarloaf.font_family_names();
                self.window
                    .screen
                    .renderer
                    .command_palette
                    .enter_fonts_mode(fonts);
            }
            Some(action) => {
                self.window
                    .screen
                    .renderer
                    .command_palette
                    .set_enabled(false);
                self.window.screen.execute_palette_action(action, clipboard);
            }
            None => self
                .window
                .screen
                .renderer
                .command_palette
                .set_enabled(false),
        }
        self.request_overlay_redraw();
    }

    fn append_palette_query(&mut self, text: &str) {
        let palette = &mut self.window.screen.renderer.command_palette;
        let mut query = palette.query.clone();
        query.push_str(text);
        palette.set_query(query);
    }

    fn pop_palette_query(&mut self) -> bool {
        let palette = &mut self.window.screen.renderer.command_palette;
        let mut query = palette.query.clone();
        let changed = query.pop().is_some();
        if changed {
            palette.set_query(query);
        }
        changed
    }

    fn handle_command_palette_key(
        &mut self,
        key_event: &KeyEvent,
        clipboard: &mut Clipboard,
        text_input: Option<&str>,
    ) {
        use ElementState::{Pressed, Released};

        match (&key_event.logical_key, key_event.state) {
            (Key::Named(NamedKey::Escape), Released) => {
                self.window
                    .screen
                    .renderer
                    .command_palette
                    .set_enabled(false);
                self.request_overlay_redraw();
            }
            (Key::Named(NamedKey::Enter), Released) => {
                self.activate_command_palette(clipboard);
            }
            (Key::Named(NamedKey::ArrowUp), Pressed) => {
                self.window
                    .screen
                    .renderer
                    .command_palette
                    .move_selection_up();
                self.request_overlay_redraw();
            }
            (Key::Named(NamedKey::ArrowDown | NamedKey::Tab), Pressed) => {
                self.window
                    .screen
                    .renderer
                    .command_palette
                    .move_selection_down();
                self.request_overlay_redraw();
            }
            (Key::Named(NamedKey::Backspace), Pressed) => {
                if self.pop_palette_query() {
                    self.request_overlay_redraw();
                }
            }
            (_, Pressed) => {
                if let Some(text) = text_input {
                    self.append_palette_query(text);
                    self.request_overlay_redraw();
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_owned_key_event(
        &mut self,
        key_event: &KeyEvent,
        clipboard: &mut Clipboard,
    ) -> bool {
        let owner = self.key_input_owner();
        if owner.is_route_owned()
            && !self.window.screen.capture_route_key_event(key_event)
        {
            return false;
        }
        let text_input = accepted_text_input(
            key_event.text.as_deref(),
            self.window.screen.modifiers.state(),
        );
        if owner.is_route_owned() && self.window.screen.clear_ime_preedit() {
            self.request_overlay_redraw();
        }

        match owner {
            KeyInputOwner::Assistant => {
                if key_event.state == ElementState::Released
                    && key_event.logical_key == Key::Named(NamedKey::Enter)
                {
                    self.assistant.clear();
                    self.window.screen.renderer.assistant.clear();
                    self.request_overlay_redraw();
                }
                true
            }
            KeyInputOwner::CommandPalette => {
                self.handle_command_palette_key(key_event, clipboard, text_input);
                true
            }
            KeyInputOwner::ConfirmQuit => {
                if key_event.state == ElementState::Released {
                    match &key_event.logical_key {
                        Key::Character(c) if c.eq_ignore_ascii_case("n") => {
                            self.window.screen.renderer.confirm_quit.set_active(false);
                            self.request_overlay_redraw();
                        }
                        Key::Named(NamedKey::Escape) => {
                            self.window.screen.renderer.confirm_quit.set_active(false);
                            self.request_overlay_redraw();
                        }
                        Key::Character(c) if c.eq_ignore_ascii_case("y") => self.quit(),
                        _ => {}
                    }
                }
                true
            }
            KeyInputOwner::Welcome => {
                if key_event.state == ElementState::Released
                    && key_event.logical_key == Key::Named(NamedKey::Enter)
                {
                    rio_backend::config::create_config_file(None);
                    self.path = RoutePath::Terminal;
                    self.request_redraw();
                }
                true
            }
            KeyInputOwner::IslandRename => {
                self.window
                    .screen
                    .renderer
                    .island
                    .as_mut()
                    .expect("key owner requires an island")
                    .handle_rename_input(
                        key_event,
                        &mut self.window.screen.context_manager,
                        text_input,
                    );
                self.request_overlay_redraw();
                true
            }
            KeyInputOwner::Hint | KeyInputOwner::Search | KeyInputOwner::Terminal => {
                false
            }
        }
    }

    pub(crate) fn handle_owned_ime(
        &mut self,
        ime: &Ime,
        clipboard: &mut Clipboard,
    ) -> bool {
        let owner = self.key_input_owner();
        if owner == KeyInputOwner::Terminal {
            return false;
        }

        if self.window.screen.update_owned_ime_state(ime) {
            self.request_overlay_redraw();
        }

        let Ime::Commit(text) = ime else {
            return true;
        };
        let Some(text) = printable_text_input(Some(text)) else {
            return true;
        };

        match owner {
            KeyInputOwner::CommandPalette => {
                self.append_palette_query(text);
                self.request_overlay_redraw();
            }
            KeyInputOwner::IslandRename => {
                self.window
                    .screen
                    .renderer
                    .island
                    .as_mut()
                    .expect("key owner requires an island")
                    .append_rename_input(text);
                self.request_overlay_redraw();
            }
            KeyInputOwner::Hint | KeyInputOwner::Search => {
                self.window.screen.handle_local_ime_commit(text, clipboard);
                self.request_redraw();
            }
            KeyInputOwner::Assistant
            | KeyInputOwner::ConfirmQuit
            | KeyInputOwner::Welcome => {}
            KeyInputOwner::Terminal => unreachable!(),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_input_owner_follows_current_render_precedence() {
        let cases = [
            (
                KeyInputSnapshot {
                    confirm_quit_active: true,
                    command_palette_active: true,
                    assistant_active: true,
                    ..KeyInputSnapshot::default()
                },
                KeyInputOwner::ConfirmQuit,
            ),
            (
                KeyInputSnapshot {
                    command_palette_active: true,
                    assistant_active: true,
                    ..KeyInputSnapshot::default()
                },
                KeyInputOwner::CommandPalette,
            ),
            (
                KeyInputSnapshot {
                    assistant_active: true,
                    ..KeyInputSnapshot::default()
                },
                KeyInputOwner::Assistant,
            ),
            (KeyInputSnapshot::default(), KeyInputOwner::Welcome),
            (
                KeyInputSnapshot {
                    route_is_terminal: true,
                    island_rename_active: true,
                    hint_active: true,
                    search_active: true,
                    ..KeyInputSnapshot::default()
                },
                KeyInputOwner::IslandRename,
            ),
            (
                KeyInputSnapshot {
                    route_is_terminal: true,
                    hint_active: true,
                    search_active: true,
                    ..KeyInputSnapshot::default()
                },
                KeyInputOwner::Hint,
            ),
            (
                KeyInputSnapshot {
                    route_is_terminal: true,
                    search_active: true,
                    ..KeyInputSnapshot::default()
                },
                KeyInputOwner::Search,
            ),
            (
                KeyInputSnapshot {
                    route_is_terminal: true,
                    ..KeyInputSnapshot::default()
                },
                KeyInputOwner::Terminal,
            ),
        ];

        for (snapshot, expected) in cases {
            assert_eq!(select_key_input_owner(snapshot), expected);
        }
    }

    #[test]
    fn text_input_rejects_shortcuts_but_preserves_altgr() {
        for (text, modifiers, expected) in [
            ("a", ModifiersState::empty(), Some("a")),
            ("A", ModifiersState::SHIFT, Some("A")),
            (
                "á",
                ModifiersState::CONTROL | ModifiersState::ALT,
                Some("á"),
            ),
            ("a", ModifiersState::CONTROL, None),
            ("a", ModifiersState::SUPER, None),
            ("\n", ModifiersState::empty(), None),
            ("", ModifiersState::empty(), None),
        ] {
            assert_eq!(accepted_text_input(Some(text), modifiers), expected);
        }

        assert_eq!(printable_text_input(Some("日本語")), Some("日本語"));
        assert_eq!(printable_text_input(Some("\n")), None);
    }
}

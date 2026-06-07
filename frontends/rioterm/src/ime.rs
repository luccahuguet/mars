use unicode_width::UnicodeWidthChar;
#[derive(Debug, Default)]
pub struct Ime {
    /// Whether the IME is enabled.
    enabled: bool,

    /// Current IME preedit.
    preedit: Option<Preedit>,

    /// Whether a cleared preedit may still be followed by its commit.
    ///
    /// Some platforms clear the visible preedit before delivering
    /// `Ime::Commit`. A keyboard event for the composing key can arrive in that
    /// gap; it belongs to the IME transaction and must not be sent to the PTY or
    /// encoded by the Kitty keyboard path.
    suppress_key_after_preedit_clear: bool,
}

impl Ime {
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    pub fn set_enabled(&mut self, is_enabled: bool) {
        if is_enabled {
            self.enabled = is_enabled
        } else {
            // Clear state when disabling IME.
            *self = Default::default();
        }
    }

    #[inline]
    #[allow(unused)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[inline]
    pub fn set_preedit(&mut self, preedit: Option<Preedit>) {
        let had_preedit = self.preedit.is_some();
        self.suppress_key_after_preedit_clear = had_preedit && preedit.is_none();
        self.preedit = preedit;
    }

    #[inline]
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }

    #[inline]
    pub fn finish_commit(&mut self) {
        self.preedit = None;
        self.suppress_key_after_preedit_clear = false;
    }

    #[inline]
    pub fn should_suppress_key_after_preedit_clear(
        &mut self,
        key_can_be_composed_text: bool,
    ) -> bool {
        if !self.suppress_key_after_preedit_clear {
            return false;
        }

        self.suppress_key_after_preedit_clear = false;
        key_can_be_composed_text
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Preedit {
    /// The preedit text.
    pub text: String,

    /// Byte offset for cursor start into the preedit text.
    ///
    /// `None` means that the cursor is invisible.
    pub cursor_byte_offset: Option<usize>,

    /// The cursor offset from the end of the preedit in char width.
    pub cursor_end_offset: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::{Ime, Preedit};

    // Test lane: default

    fn preedit(text: &str) -> Preedit {
        Preedit::new(text.to_owned(), None)
    }

    #[test]
    fn cleared_preedit_suppresses_one_composed_text_key() {
        // Defends: a dead-key compose key must not leak as raw text or Kitty CSI-u before commit.
        let mut ime = Ime::new();

        ime.set_preedit(Some(preedit("´")));
        ime.set_preedit(None);

        assert!(ime.should_suppress_key_after_preedit_clear(true));
        assert!(!ime.should_suppress_key_after_preedit_clear(true));
    }

    #[test]
    fn non_text_key_clears_preedit_suppression_without_suppressing() {
        // Defends: canceled IME state cannot stale-suppress the next ordinary text key.
        let mut ime = Ime::new();

        ime.set_preedit(Some(preedit("´")));
        ime.set_preedit(None);

        assert!(!ime.should_suppress_key_after_preedit_clear(false));
        assert!(!ime.should_suppress_key_after_preedit_clear(true));
    }

    #[test]
    fn commit_clears_preedit_suppression() {
        // Defends: committed IME text is the only PTY input after a normal compose transaction.
        let mut ime = Ime::new();

        ime.set_preedit(Some(preedit("´")));
        ime.set_preedit(None);
        ime.finish_commit();

        assert!(!ime.should_suppress_key_after_preedit_clear(true));
    }

    #[test]
    fn disabled_ime_clears_preedit_suppression() {
        // Defends: canceled IME state cannot suppress later ordinary shortcuts or text.
        let mut ime = Ime::new();

        ime.set_enabled(true);
        ime.set_preedit(Some(preedit("´")));
        ime.set_preedit(None);
        ime.set_enabled(false);

        assert!(!ime.should_suppress_key_after_preedit_clear(true));
    }

    #[test]
    fn ordinary_keys_are_not_suppressed_without_preedit() {
        // Defends: normal Kitty keyboard chords such as Ctrl+Alt+H stay visible.
        let mut ime = Ime::new();

        assert!(!ime.should_suppress_key_after_preedit_clear(true));
    }
}

impl Preedit {
    pub fn new(text: String, cursor_byte_offset: Option<usize>) -> Self {
        let cursor_end_offset = if let Some(byte_offset) = cursor_byte_offset {
            // Convert byte offset into char offset.
            let cursor_end_offset = text[byte_offset..]
                .chars()
                .fold(0, |acc, ch| acc + ch.width().unwrap_or(1));

            Some(cursor_end_offset)
        } else {
            None
        };

        Self {
            text,
            cursor_byte_offset,
            cursor_end_offset,
        }
    }
}

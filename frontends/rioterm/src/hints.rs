use rio_backend::config::hints::Hint;
use rio_backend::crosswords::grid::Dimensions;
use rio_backend::crosswords::pos::{Column, Line, Pos};
use rio_backend::event::EventListener;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// State for hint selection mode
pub struct HintState {
    /// Currently active hint configuration
    active_hint: Option<Rc<Hint>>,

    /// Visible matches for the current hint
    matches: Vec<HintMatch>,

    /// Labels for each match (as Vec<char>)
    labels: Vec<Vec<char>>,

    /// Keys pressed so far for hint selection
    keys: Vec<char>,

    /// Alphabet for generating labels
    alphabet: String,
}

/// A match found by a hint
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintMatch {
    /// The text that was matched
    pub text: String,

    /// Start position of the match
    pub start: Pos,

    /// End position of the match
    pub end: Pos,

    /// The hint configuration that created this match
    pub hint: Rc<Hint>,
}

impl HintMatch {
    pub(crate) fn from_hyperlink(
        uri: &str,
        start: Pos,
        end: Pos,
        hint: Rc<Hint>,
    ) -> Option<Self> {
        Some(Self {
            text: process_hint_text(uri, hint.post_processing)?,
            start,
            end,
            hint,
        })
    }
}

impl HintState {
    pub fn new(alphabet: String) -> Self {
        Self {
            active_hint: None,
            matches: Vec::new(),
            labels: Vec::new(),
            keys: Vec::new(),
            alphabet,
        }
    }

    /// Check if hint mode is active
    pub fn is_active(&self) -> bool {
        self.active_hint.is_some()
    }

    /// Start hint mode with the given hint configuration
    pub fn start(&mut self, hint: Rc<Hint>) {
        self.active_hint = Some(hint);
        self.keys.clear();
        // matches and labels will be updated by update_matches
    }

    /// Stop hint mode
    pub fn stop(&mut self) {
        self.active_hint = None;
        self.matches.clear();
        self.labels.clear();
        self.keys.clear();
    }

    /// Update visible matches for the current hint
    pub fn update_matches<T: EventListener>(
        &mut self,
        term: &rio_backend::crosswords::Crosswords<T>,
    ) {
        self.matches.clear();

        let hint = match &self.active_hint {
            Some(hint) => hint.clone(),
            None => {
                return;
            }
        };

        // Find regex matches if regex is specified
        if let Some(regex_pattern) = &hint.regex {
            if let Ok(regex) = onig::Regex::new(regex_pattern) {
                self.find_regex_matches(term, &regex, hint.clone());
            }
        }

        // Find OSC 8 hyperlinks if enabled
        if hint.hyperlinks {
            self.find_hyperlink_matches(term, hint.clone());
        }

        // Cancel hint mode if no matches found
        if self.matches.is_empty() {
            self.stop();
            return;
        }

        // Sort and dedup matches
        self.matches.sort_by_key(|m| (m.start.row, m.start.col));
        self.matches.dedup_by_key(|m| m.start);

        // Generate labels for matches
        self.generate_labels();
    }

    /// Handle keyboard input during hint selection
    pub fn keyboard_input<T: EventListener>(
        &mut self,
        term: &rio_backend::crosswords::Crosswords<T>,
        c: char,
    ) -> Option<HintMatch> {
        match c {
            // Use backspace to remove the last character pressed
            '\x08' | '\x1f' => {
                self.keys.pop();
                // Only update matches after backspace to regenerate visible labels
                self.update_matches(term);
                return None;
            }
            // Cancel hint highlighting on ESC/Ctrl+c
            '\x1b' | '\x03' => {
                self.stop();
                return None;
            }
            _ => (),
        }

        let hint = self.active_hint.as_ref()?;

        // Get visible labels (labels filtered by keys pressed so far)
        let visible_labels = self.visible_labels();

        // Find the last label starting with the input character
        let mut matching_labels = visible_labels.iter().rev();
        let (index, remaining_label) = matching_labels
            .find(|(_, remaining)| !remaining.is_empty() && remaining[0] == c)?;

        // Check if this completes the label (only one character remaining)
        if remaining_label.len() == 1 {
            let hint_match = self.matches.get(*index)?.clone();
            let hint_config = hint.clone();

            // Exit hint mode unless it requires explicit dismissal
            if hint_config.persist {
                self.keys.clear();
            } else {
                self.stop();
            }

            Some(hint_match)
        } else {
            // Store character to preserve the selection
            self.keys.push(c);
            None
        }
    }

    /// Get current matches
    pub fn matches(&self) -> &[HintMatch] {
        &self.matches
    }

    /// Get keys pressed so far
    #[allow(dead_code)]
    pub fn keys_pressed(&self) -> &[char] {
        &self.keys
    }

    /// Get visible labels (filtered by current input)
    pub fn visible_labels(&self) -> Vec<(usize, Vec<char>)> {
        let keys_len = self.keys.len();
        self.labels
            .iter()
            .enumerate()
            .filter_map(|(i, label)| {
                if label.len() >= keys_len && label[..keys_len] == self.keys[..] {
                    let remaining: Vec<char> = label[keys_len..].to_vec();
                    Some((i, remaining))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Update the alphabet used for hint labels
    #[allow(dead_code)]
    pub fn update_alphabet(&mut self, alphabet: &str) {
        if self.alphabet != alphabet {
            self.alphabet = alphabet.to_string();
            self.keys.clear();
        }
    }

    // Private helper methods

    fn find_regex_matches<T: EventListener>(
        &mut self,
        term: &rio_backend::crosswords::Crosswords<T>,
        regex: &onig::Regex,
        hint: Rc<Hint>,
    ) {
        // Get the visible area of the terminal
        let grid = &term.grid;
        let display_offset = grid.display_offset();
        let visible_lines = grid.screen_lines();

        // Scan each visible line for matches
        for line_idx in 0..visible_lines {
            let line = Line(line_idx as i32 - display_offset as i32);
            if line < Line(0) || line.0 >= grid.total_lines() as i32 {
                continue;
            }

            // Extract text from the line
            let line_text = self.extract_line_text(term, line);

            // Find all matches in this line. Onig yields (byte_start, byte_end);
            for (start, end) in regex.find_iter(&line_text) {
                let start_col = Column(line_text[..start].chars().count());
                let Some(match_text) =
                    process_hint_text(&line_text[start..end], hint.post_processing)
                else {
                    continue;
                };

                let end_col =
                    Column(start_col.0 + match_text.chars().count().saturating_sub(1));

                let hint_match = HintMatch {
                    text: match_text,
                    start: Pos::new(line, start_col),
                    end: Pos::new(line, end_col),
                    hint: hint.clone(),
                };

                self.matches.push(hint_match);
            }
        }
    }

    fn find_hyperlink_matches<T: EventListener>(
        &mut self,
        term: &rio_backend::crosswords::Crosswords<T>,
        hint: Rc<Hint>,
    ) {
        // Walk the visible region looking for OSC 8 hyperlink spans.
        //
        // After the cell repack, hyperlinks live in the per-grid
        // `extras_table`. Each cell carries an `extras_id: u16`; cells
        // in the same hyperlink span share that id. We compare ids
        // (cheap u16 compare) to find the start and end of each span,
        // then look up the URI once via `Crosswords::cell_hyperlink`.
        let grid = &term.grid;
        let display_offset = grid.display_offset();
        let visible_lines = grid.screen_lines();

        for line_idx in 0..visible_lines {
            let line = Line(line_idx as i32 - display_offset as i32);
            if line < Line(0) || line.0 >= grid.total_lines() as i32 {
                continue;
            }

            let mut col = 0usize;
            let cols = grid.columns();
            while col < cols {
                let id = match term.cell_hyperlink_id(line, Column(col)) {
                    Some(id) => id,
                    None => {
                        col += 1;
                        continue;
                    }
                };

                // Found the start of a hyperlink span. Walk forward
                // until the extras_id changes.
                let start_col = col;
                let mut end_col = col;
                while end_col < cols
                    && term.cell_hyperlink_id(line, Column(end_col)) == Some(id)
                {
                    end_col += 1;
                }
                let end_pos = Column(end_col - 1);
                let Some(trimmed_end_col) = trim_trailing_link_blank_cells(
                    term,
                    line,
                    Column(start_col),
                    end_pos,
                ) else {
                    col = end_col;
                    continue;
                };

                // Look up the URI once for the whole span.
                if let Some(hyperlink) = term.cell_hyperlink(line, Column(start_col)) {
                    if let Some(hint_match) = HintMatch::from_hyperlink(
                        hyperlink.uri(),
                        Pos::new(line, Column(start_col)),
                        Pos::new(line, trimmed_end_col),
                        hint.clone(),
                    ) {
                        self.matches.push(hint_match);
                    }
                }

                col = end_col;
            }
        }
    }

    fn extract_line_text<T: EventListener>(
        &self,
        term: &rio_backend::crosswords::Crosswords<T>,
        line: Line,
    ) -> String {
        let grid = &term.grid;
        let mut text = String::new();

        for col in 0..grid.columns() {
            let cell = &grid[line][Column(col)];
            text.push(cell.c());
        }

        text.trim_end().to_string()
    }

    fn generate_labels(&mut self) {
        self.labels.clear();
        let mut generator = LabelGenerator::new(&self.alphabet);

        for _ in 0..self.matches.len() {
            self.labels.push(generator.next());
        }
    }
}

#[inline]
fn is_trailing_link_blank(c: char) -> bool {
    c == '\0' || c.is_whitespace()
}

pub(crate) fn trim_trailing_link_blank_cells<T: EventListener>(
    term: &rio_backend::crosswords::Crosswords<T>,
    line: Line,
    start_col: Column,
    end_col: Column,
) -> Option<Column> {
    if end_col < start_col {
        return None;
    }

    let mut col = end_col;
    loop {
        let c = term.grid[line][col].c();
        if !is_trailing_link_blank(c) {
            return Some(col);
        }
        if col == start_col {
            return None;
        }
        col -= 1;
    }
}

/// Generates hint labels using the specified alphabet
struct LabelGenerator {
    alphabet: Vec<char>,
    indices: Vec<usize>,
}

impl LabelGenerator {
    fn new(alphabet: &str) -> Self {
        Self {
            alphabet: alphabet.chars().collect(),
            indices: vec![0],
        }
    }

    fn next(&mut self) -> Vec<char> {
        let label = self.current_label();
        self.increment();
        label
    }

    fn current_label(&self) -> Vec<char> {
        self.indices
            .iter()
            .rev()
            .map(|&i| self.alphabet[i])
            .collect()
    }

    fn increment(&mut self) {
        let mut carry = true;
        let mut pos = 0;

        while carry && pos < self.indices.len() {
            self.indices[pos] += 1;
            if self.indices[pos] >= self.alphabet.len() {
                self.indices[pos] = 0;
                pos += 1;
            } else {
                carry = false;
            }
        }

        if carry {
            self.indices.push(0);
        }
    }
}

/// URI scheme prefixes that should never be resolved as file paths.
/// Matches the scheme branch of `DEFAULT_URL_REGEX`.
const URI_SCHEMES: &[&str] = &[
    "ipfs:",
    "ipns:",
    "magnet:",
    "mailto:",
    "gemini://",
    "gopher://",
    "https://",
    "http://",
    "news:",
    "file:",
    "git://",
    "ssh:",
    "ftp://",
    "tel:",
];

pub fn has_uri_scheme(text: &str) -> bool {
    let text = text.trim();
    URI_SCHEMES.iter().any(|scheme| {
        text.get(..scheme.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
    })
}

/// If `text` looks like a local filesystem path, resolve it against `cwd` and
/// return the absolute path when it exists on disk. Returns `None` for
/// URL-scheme strings, paths that don't exist, or anything we can't resolve
/// (e.g. relative path with no known `cwd`). On `None`, the caller should
/// fall back to the raw text and let the OS opener handle it.
///
/// Modelled on ghostty's `resolvePathForOpening` (`src/Surface.zig:2045`).
/// core only joins relative paths against the OSC 7 cwd; tilde
/// expansion lives in the macOS apprt's Swift `openURL`
/// (`.App.swift:715`, via `NSString.standardizingPath`), so `~/x`
/// works on macOS but isn't expanded on Linux/BSD where `xdg-open` gets the
/// literal `~`. Rio doesn't have a per-platform apprt layer, so we do the
/// expansion here to get consistent cross-platform behaviour:
///
/// 1. `~/x` and `~` expand via `dirs::home_dir()`.
/// 2. `$VAR/x` expands via `std::env::var` (ghostty doesn't do this on any
///    platform).
/// 3. Strings starting with a known URI scheme are rejected up front so the
///    OS opener routes them as URLs (saves one filesystem syscall vs
///    ghostty's "join cwd + stat → fail" path).
/// 4. Absolute paths are existence-checked too. short-circuits
///    absolute paths to `None` (caller passes raw); user-visible behaviour
///    is the same since the raw and resolved strings match.
pub fn resolve_path_for_opening(text: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Scheme URLs are not paths — let the OS opener route them.
    if has_uri_scheme(text) {
        return None;
    }

    // Expand a recognized path prefix. Anything falling through is treated as
    // a bare relative path (e.g. `src/main.rs`).
    let expanded: PathBuf = if let Some(rest) = text.strip_prefix("~/") {
        dirs::home_dir()?.join(rest)
    } else if text == "~" {
        dirs::home_dir()?
    } else if let Some(rest) = text.strip_prefix('$') {
        let (var_name, tail) = rest.split_once('/').unwrap_or((rest, ""));
        if var_name.is_empty() {
            return None;
        }
        let value = std::env::var(var_name).ok()?;
        let base = PathBuf::from(value);
        if tail.is_empty() {
            base
        } else {
            base.join(tail)
        }
    } else {
        PathBuf::from(text)
    };

    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        cwd?.join(expanded)
    };

    if absolute.exists() {
        Some(absolute)
    } else {
        None
    }
}

/// Remove trailing delimiters and truncate at unmatched closing brackets.
fn post_process_hint_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut end = chars.len();
    let mut open_parents = 0;
    let mut open_brackets = 0;

    // First pass: handle uneven brackets/parentheses
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '(' => open_parents += 1,
            '[' => open_brackets += 1,
            ')' => {
                if open_parents == 0 {
                    end = i;
                    break;
                }
                open_parents -= 1;
            }
            ']' => {
                if open_brackets == 0 {
                    end = i;
                    break;
                }
                open_brackets -= 1;
            }
            _ => (),
        }
    }

    while end > 0 && is_trailing_hint_delimiter(chars[end - 1]) {
        end -= 1;
    }

    chars.into_iter().take(end).collect()
}

#[inline]
pub(crate) fn is_trailing_hint_delimiter(c: char) -> bool {
    matches!(c, '.' | ',' | ':' | ';' | '?' | '!' | '(' | '[' | '\'')
}

pub(crate) fn process_hint_text(text: &str, post_processing: bool) -> Option<String> {
    let text = if post_processing {
        post_process_hint_text(text)
    } else {
        text.to_owned()
    };
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rio_backend::config::hints::{HintAction, HintInternalAction};

    #[test]
    fn test_label_generator() {
        let mut gen = LabelGenerator::new("abc");
        assert_eq!(gen.next(), vec!['a']);
        assert_eq!(gen.next(), vec!['b']);
        assert_eq!(gen.next(), vec!['c']);
        assert_eq!(gen.next(), vec!['a', 'a']);
        assert_eq!(gen.next(), vec!['a', 'b']);
        assert_eq!(gen.next(), vec!['a', 'c']);
        assert_eq!(gen.next(), vec!['b', 'a']);
    }

    #[test]
    fn test_hint_state_lifecycle() {
        let mut state = HintState::new("abc".to_string());
        assert!(!state.is_active());

        let hint = Rc::new(Hint {
            regex: Some("test".to_string()),
            hyperlinks: false,
            post_processing: true,
            persist: false,
            action: HintAction::Action {
                action: HintInternalAction::Copy,
            },
            mouse: Default::default(),
            binding: None,
        });

        state.start(hint);
        assert!(state.is_active());

        state.stop();
        assert!(!state.is_active());
    }

    #[test]
    fn test_visible_labels() {
        let mut state = HintState::new("abc".to_string());
        state.labels = vec![vec!['a'], vec!['b'], vec!['a', 'b'], vec!['a', 'c']];

        // No input - all labels visible
        let visible = state.visible_labels();
        assert_eq!(visible.len(), 4);

        // Input "a" - should show labels that start with "a"
        state.keys = vec!['a'];
        let visible = state.visible_labels();
        assert_eq!(visible.len(), 3); // "a", "ab", "ac"
        assert_eq!(visible[0].1, Vec::<char>::new()); // "a" with "a" removed = []
        assert_eq!(visible[1].1, vec!['b']); // "ab" with "a" removed = ['b']
        assert_eq!(visible[2].1, vec!['c']); // "ac" with "a" removed = ['c']
    }

    #[test]
    fn test_keyboard_input_logic() {
        let mut state = HintState::new("jfkdls".to_string());

        // Simulate having some labels
        state.labels = vec![
            vec!['j'], // index 0
            vec!['f'], // index 1
            vec!['k'], // index 2
            vec!['d'], // index 3
            vec!['l'], // index 4
            vec!['s'], // index 5
        ];

        // Simulate having matches (we'll use dummy matches)
        state.matches = vec![
            HintMatch {
                text: "match0".to_string(),
                start: rio_backend::crosswords::pos::Pos::new(
                    rio_backend::crosswords::pos::Line(0),
                    rio_backend::crosswords::pos::Column(0),
                ),
                end: rio_backend::crosswords::pos::Pos::new(
                    rio_backend::crosswords::pos::Line(0),
                    rio_backend::crosswords::pos::Column(5),
                ),
                hint: Rc::new(Hint {
                    regex: Some("test".to_string()),
                    hyperlinks: false,
                    post_processing: true,
                    persist: false,
                    action: HintAction::Action {
                        action: HintInternalAction::Copy,
                    },
                    mouse: Default::default(),
                    binding: None,
                }),
            },
            HintMatch {
                text: "match1".to_string(),
                start: rio_backend::crosswords::pos::Pos::new(
                    rio_backend::crosswords::pos::Line(0),
                    rio_backend::crosswords::pos::Column(10),
                ),
                end: rio_backend::crosswords::pos::Pos::new(
                    rio_backend::crosswords::pos::Line(0),
                    rio_backend::crosswords::pos::Column(15),
                ),
                hint: Rc::new(Hint {
                    regex: Some("test".to_string()),
                    hyperlinks: false,
                    post_processing: true,
                    persist: false,
                    action: HintAction::Action {
                        action: HintInternalAction::Copy,
                    },
                    mouse: Default::default(),
                    binding: None,
                }),
            },
        ];

        let hint = Rc::new(Hint {
            regex: Some("test".to_string()),
            hyperlinks: false,
            post_processing: true,
            persist: false,
            action: HintAction::Action {
                action: HintInternalAction::Copy,
            },
            mouse: Default::default(),
            binding: None,
        });

        state.active_hint = Some(hint);

        // Test keyboard input logic without needing a terminal
        // Test that 'j' should match the first label
        let mut test_keys = state.keys.clone();
        test_keys.push('j');

        let mut matching_indices = Vec::new();
        for (i, label) in state.labels.iter().enumerate() {
            if label.len() >= test_keys.len() && label[..test_keys.len()] == test_keys[..]
            {
                matching_indices.push(i);
            }
        }

        assert!(
            !matching_indices.is_empty(),
            "Should find matching labels for 'j'"
        );
        assert_eq!(matching_indices, vec![0], "Should match index 0 for 'j'");

        // Test that the label should be completed (single character)
        let index = *matching_indices.last().unwrap();
        let label = &state.labels[index];
        assert_eq!(
            label.len(),
            test_keys.len(),
            "Label should be completed with single character"
        );
    }

    #[test]
    fn test_resolve_path_skips_scheme_urls() {
        assert!(resolve_path_for_opening("https://example.com", None).is_none());
        assert!(resolve_path_for_opening("HTTP://localhost:4321/", None).is_none());
        assert!(resolve_path_for_opening("http://localhost:4321/", None).is_none());
        assert!(resolve_path_for_opening("mailto:a@b.c", None).is_none());
        assert!(resolve_path_for_opening("file:///tmp", None).is_none());
        assert!(resolve_path_for_opening("ssh://host/path", None).is_none());
    }

    #[test]
    fn test_has_uri_scheme() {
        assert!(has_uri_scheme("http://localhost:4321/"));
        assert!(has_uri_scheme("HTTP://localhost:4321/"));
        assert!(has_uri_scheme(" https://example.com/docs "));
        assert!(!has_uri_scheme("/tmp/file.txt"));
        assert!(!has_uri_scheme("src/main.rs"));
    }

    #[test]
    fn test_post_processing_rejects_delimiter_only_targets() {
        for input in ["", ".", ",", ":", ";", "?", "!", "(", "[", "'", ")", "]"] {
            assert_eq!(post_process_hint_text(input), "", "input: {input:?}");
        }

        for (input, expected) in [
            ("https://example.com)", "https://example.com"),
            ("https://example.com,", "https://example.com"),
            (
                "https://example.com/path(with)parens",
                "https://example.com/path(with)parens",
            ),
            ("https://example.com/path)", "https://example.com/path"),
            ("https://example.com.'),", "https://example.com"),
            ("https://example.com/path]", "https://example.com/path"),
            (
                "https://example.com/path[with]brackets",
                "https://example.com/path[with]brackets",
            ),
        ] {
            assert_eq!(post_process_hint_text(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn test_hyperlink_hint_trims_trailing_blank_cells() {
        use rio_backend::ansi::CursorShape;
        use rio_backend::crosswords::{Crosswords, CrosswordsSize};
        use rio_backend::event::{VoidListener, WindowId};
        use rio_backend::performer::handler::Processor;

        let url = "http://localhost:4321/";
        let label = format!("{url}   ");
        let bytes =
            format!("\x1b]8;;{url}\x07{label}\x1b]8;;\x07\x1b]8;;.\x07.\x1b]8;;\x07");
        let window_id = WindowId::from(0);
        let mut term = Crosswords::new(
            CrosswordsSize::new(40, 3),
            CursorShape::Block,
            VoidListener {},
            window_id,
            0,
            10_000,
        );
        let mut processor = Processor::default();
        processor.advance(&mut term, bytes.as_bytes());

        let hint = Rc::new(Hint {
            regex: None,
            hyperlinks: true,
            post_processing: true,
            persist: false,
            action: HintAction::Action {
                action: HintInternalAction::Copy,
            },
            mouse: Default::default(),
            binding: None,
        });
        let mut state = HintState::new("abc".to_string());

        state.find_hyperlink_matches(&term, hint.clone());

        assert_eq!(state.matches.len(), 1);
        let hint_match = &state.matches[0];
        assert_eq!(hint_match.text, url);
        assert!(Rc::ptr_eq(&hint_match.hint, &hint));
        assert_eq!(hint_match.start, Pos::new(Line(0), Column(0)));
        assert_eq!(
            hint_match.end,
            Pos::new(Line(0), Column(url.len().saturating_sub(1)))
        );
        assert!(term.cell_hyperlink_id(Line(0), Column(url.len())).is_some());
    }

    #[test]
    fn test_resolve_path_returns_none_when_nonexistent() {
        let cwd = std::env::temp_dir();
        assert!(resolve_path_for_opening(
            "rio-definitely-does-not-exist-xyz",
            Some(&cwd)
        )
        .is_none());
        assert!(resolve_path_for_opening(
            "./rio-definitely-does-not-exist-xyz",
            Some(&cwd)
        )
        .is_none());
    }

    #[test]
    fn test_resolve_path_absolute_existing_file() {
        let tmp = std::env::temp_dir();
        let file = tmp.join("rio-test-resolve-abs.txt");
        std::fs::write(&file, "hi").unwrap();

        let resolved = resolve_path_for_opening(&file.to_string_lossy(), None).unwrap();
        // PathBuf::exists() follows symlinks; on macOS /tmp is a symlink to
        // /private/tmp, so compare existence rather than exact paths.
        assert!(resolved.exists());

        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn test_resolve_path_relative_joined_with_cwd() {
        let tmp = std::env::temp_dir();
        let subdir = tmp.join("rio-test-resolve-dir");
        std::fs::create_dir_all(&subdir).unwrap();
        let file = subdir.join("child.txt");
        std::fs::write(&file, "hi").unwrap();

        let resolved = resolve_path_for_opening("child.txt", Some(&subdir)).unwrap();
        assert!(resolved.exists());
        assert!(resolved.ends_with("child.txt"));

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&subdir);
    }

    #[test]
    fn test_resolve_path_dot_relative_joined_with_cwd() {
        let tmp = std::env::temp_dir();
        let subdir = tmp.join("rio-test-resolve-dot-dir");
        std::fs::create_dir_all(&subdir).unwrap();
        let file = subdir.join("dot-child.txt");
        std::fs::write(&file, "hi").unwrap();

        let resolved =
            resolve_path_for_opening("./dot-child.txt", Some(&subdir)).unwrap();
        assert!(resolved.exists());

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&subdir);
    }

    #[test]
    fn test_resolve_path_requires_cwd_for_relative() {
        // With no cwd and a relative path, we can't resolve; return None.
        assert!(resolve_path_for_opening("foo/bar.txt", None).is_none());
    }

    #[test]
    fn test_resolve_path_expands_env_var() {
        let tmp = std::env::temp_dir();
        // Safety: setting an env var inside a process-local test. This is
        // unsafe in Rust 2024; rio-backend uses an earlier edition so it's
        // permitted here. If rio moves to 2024 this test needs adjustment.
        unsafe {
            std::env::set_var("RIO_TEST_PATH_VAR", tmp.to_string_lossy().to_string());
        }

        let file = tmp.join("rio-test-env-var.txt");
        std::fs::write(&file, "hi").unwrap();

        let resolved =
            resolve_path_for_opening("$RIO_TEST_PATH_VAR/rio-test-env-var.txt", None)
                .unwrap();
        assert!(resolved.exists());

        let _ = std::fs::remove_file(&file);
        unsafe {
            std::env::remove_var("RIO_TEST_PATH_VAR");
        }
    }
}

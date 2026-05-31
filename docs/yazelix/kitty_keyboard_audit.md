# Kitty Keyboard Protocol Audit

Reviewed on 2026-05-31 against:

- Official Kitty keyboard protocol: https://sw.kovidgoyal.net/kitty/keyboard-protocol/
- Frontend encoder: `frontends/rioterm/src/bindings/kitty_keyboard.rs`
- Frontend dispatch: `frontends/rioterm/src/screen/mod.rs`
- Backend mode state: `rio-backend/src/ansi/mod.rs`
- Backend handler state: `rio-backend/src/crosswords/mod.rs`
- Black-box comparison fixtures: `docs/yazelix/kitty_keyboard_blackbox.md`

The result is strong but not yet parity-complete. The mode bits, query path,
screen-local stacks, common CSI-u emission, repeat/release emission, associated
text, most private-use function keys, and most numpad keys are present. The
remaining risks are exact stack edge semantics, full modifier-bit reporting,
base-layout alternate keys, and a few private-use functional keys that winit can
surface but Rio does not encode yet.

## Spec Anchors

Kitty's event form is:

```text
CSI unicode-key-code:alternate-key-codes ; modifiers:event-type ; text-as-codepoints u
```

The five enhancement flags are:

```text
1  disambiguate escape codes
2  report event types
4  report alternate keys
8  report all keys as escape codes
16 report associated text
```

Mode control uses `CSI = flags ; mode u`, current-mode query uses `CSI ? u`,
push uses `CSI > flags u`, and pop uses `CSI < number u`. The spec requires
separate main and alternate screen stacks. It also says a pop that empties the
stack resets all flags, and a push into a full stack evicts the oldest entry.

## Implemented

- Enhancement bits match the official values in `KeyboardModes`
- `CSI = flags ; mode u` supports replace, union, and difference
- `CSI ? u` replies with `CSI ? flags u`
- `CSI > flags u` and `CSI < number u` are parsed
- Main and alternate screens swap independent keyboard stacks during alt-screen
  switching
- `DISAMBIGUATE_ESC_CODES` routes ambiguous keys to CSI-u when needed
- `REPORT_EVENT_TYPES` emits repeat and release subfields, while preserving
  Kitty's Enter/Tab/Backspace exception unless report-all is active
- `REPORT_ALL_KEYS_AS_ESC` sends text-producing keys as escape sequences and
  enables pure modifier key events
- `REPORT_ASSOCIATED_TEXT` appends generated text codepoints and rejects control
  characters
- `REPORT_ALTERNATE_KEYS` emits a shifted alternate codepoint when Rio can infer
  one
- F13-F35, ScrollLock, PrintScreen, Pause, ContextMenu, media playback keys,
  volume keys, left/right modifiers, CapsLock, and NumLock have private-use
  code mappings
- Common keypad numbers, operators, Enter, arrows, paging, Home/End, Insert, and
  Delete map to Kitty keypad codepoints when `KeyLocation::Numpad` is present

## Gaps

### Stack Edge Semantics

The backend stores a fixed array plus an active index. Popping more than the
stack depth clears the stack, but popping one entry while already at index zero
wraps to the last slot. The spec says a pop that empties the stack resets all
flags. The current `test_keyboard_mode_stack_underflow_protection` test codifies
the wrap behavior, so this should be fixed deliberately with a new regression
test.

The full-stack behavior is also circular overwrite rather than an explicit
"oldest entry evicted" stack model. It may be equivalent for some access
patterns, but the audit should not claim exact spec parity until push/pop order
is tested against overflow.

### Modifier Bits

`SequenceModifiers` reports shift, alt, control, super, hyper, and meta.
CapsLock and NumLock bits are defined but intentionally not inferred from key
presses, because Kitty wants enabled lock state and `rio-window::ModifiersState`
does not expose lock state today. CapsLock and NumLock key events still have
their private-use key codes; only the lock modifier bits remain platform-limited.

### Base-Layout Alternate Keys

The encoder reports shifted alternate keys and base-layout alternate keys for
PC-101 letters, digits, and punctuation exposed through `KeyEvent.physical_key`.
This matters for shortcuts on non-Latin layouts, where applications want
`ctrl+c` to match the physical `C` key even when the active layout emits another
character.

Remaining limits: niche keys outside the PC-101 letter/digit/punctuation set are
not assigned a guessed base-layout value.

### Functional And Keypad Coverage

The current mapping covers the important editor and shell keys, but it does not
cover every Kitty private-use key Rio can observe. Known holes from the local
source audit:

- `MEDIA_REVERSE` (`57431`) is not mapped because `rio-window` does not expose a
  distinct reverse-media key today
- ISO level 5 shift (`57454`) is not mapped because the local XKB adapter leaves
  ISO level 5 keysyms commented out instead of exposing a `NamedKey`

The first implementation follow-up after this audit filled the table entries
Rio does expose directly: `KP_SEPARATOR` (`57416`), `KP_BEGIN` (`57427~`), and
ISO level 3 shift through `NamedKey::AltGraph` (`57453`).

## Compatibility Impact

Helix, Yazi, Nushell, and Zellij mostly depend on the implemented part: mode
query, push/pop around alternate screen, disambiguated Esc/Ctrl/Alt keys,
release/repeat when requested, and keypad/function keys. The gaps affect harder
cases:

- non-Latin keyboard layout shortcut matching
- apps that rely on exact stack behavior after nested keyboard-mode users
- games or TUI input test tools that inspect Hyper/Meta/Caps/Num modifier bits
- keypad separator/begin keys on keyboards that expose them distinctly

## Follow-Up Work

- Fix keyboard mode stack underflow/overflow semantics and replace the wraparound
  underflow test with a spec test
- Add full modifier-bit support where platform events expose the needed state
- Implement base-layout alternate keys from `physical_key`
- Fill the remaining Kitty functional/keypad private-use mappings
- Run black-box key captures comparing Rio, Ghostty, and Kitty for the checked-in
  Helix/Yazi/Nushell/Zellij-oriented case matrix

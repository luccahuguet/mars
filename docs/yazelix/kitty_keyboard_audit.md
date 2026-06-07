# Kitty Keyboard Protocol Audit

Reviewed on 2026-05-31 against:

- Official Kitty keyboard protocol: https://sw.kovidgoyal.net/kitty/keyboard-protocol/
- Frontend encoder: `frontends/rioterm/src/bindings/kitty_keyboard.rs`
- Frontend dispatch: `frontends/rioterm/src/screen/mod.rs`
- Backend mode state: `rio-backend/src/ansi/mod.rs`
- Backend handler state: `rio-backend/src/crosswords/mod.rs`
- Black-box comparison fixtures: `docs/yazelix/kitty_keyboard_blackbox.md`

The result is strong. The mode bits, query path, screen-local stacks, common
CSI-u emission, repeat/release emission, associated text, base-layout alternate
keys, lock-state modifier bits where the platform exposes them, private-use
function keys, and numpad keys are present. The remaining limits are platform
lock-state availability and a few private-use functional keys that `rio-window`
does not expose as distinct events today.

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
- IME/dead-key commits are kept out of the Kitty keyboard encoder and written
  as committed text input
- `REPORT_EVENT_TYPES` emits repeat and release subfields, while preserving
  Kitty's Enter/Tab/Backspace exception unless report-all is active
- `REPORT_ALL_KEYS_AS_ESC` sends text-producing keys as escape sequences and
  enables pure modifier key events
- `REPORT_ASSOCIATED_TEXT` appends generated text codepoints and rejects control
  characters
- `REPORT_ALTERNATE_KEYS` emits a shifted alternate codepoint when Rio can infer
  one
- F13-F35, ScrollLock, PrintScreen, Pause, ContextMenu, media playback keys,
  volume keys, left/right modifiers, CapsLock, NumLock, and ISO level 3 shift
  have private-use code mappings
- Common keypad numbers, operators, Enter, arrows, paging, Home/End, Insert, and
  Delete map to Kitty keypad codepoints when `KeyLocation::Numpad` is present,
  including keypad separator and keypad begin/clear
- CapsLock and NumLock modifier bits are reported from platform lock state in
  enhanced Kitty keyboard mode when `rio-window` exposes that state. This is
  wired through Linux XKB, Windows, Web, and macOS CapsLock. macOS NumLock and
  other platforms that do not expose a reliable NumLock state leave the bit
  unset.
- Lock-state modifier bits are intentionally excluded from legacy xterm
  modifier sequences so CapsLock does not perturb normal arrow/function-key
  escapes outside Kitty keyboard mode

## Gaps

### Stack Edge Semantics

Stack edge semantics are covered by regressions:

- popping an empty/base stack resets all flags instead of wrapping
- popping nested entries restores the previous stack entry
- pushing into a full stack evicts the oldest entry

### Modifier Bits

`SequenceModifiers` reports shift, alt, control, super, hyper, meta, CapsLock,
and NumLock. CapsLock and NumLock key events also have their private-use key
codes. Lock bits are only reported from platform lock-state events, not inferred
from the lock keypress itself, because Kitty wants enabled lock state rather
than "this lock key is currently being pressed".

Remaining limit: NumLock is still platform-limited where the window/input layer
does not expose a reliable enabled state, such as macOS.

### Base-Layout Alternate Keys

The encoder reports shifted alternate keys and base-layout alternate keys for
PC-101 letters, digits, and punctuation exposed through `KeyEvent.physical_key`.
This matters for shortcuts on non-Latin layouts, where applications want
`ctrl+c` to match the physical `C` key even when the active layout emits another
character.

Remaining limits: niche keys outside the PC-101 letter/digit/punctuation set are
not assigned a guessed base-layout value.

### Functional And Keypad Coverage

The current mapping covers the important editor and shell keys and the exposed
private-use keys Rio can observe. Known holes from the local source audit:

- `MEDIA_REVERSE` (`57431`) is not mapped because `rio-window` does not expose a
  distinct reverse-media key today
- ISO level 5 shift (`57454`) is not mapped because the local XKB adapter leaves
  ISO level 5 keysyms commented out instead of exposing a `NamedKey`

Rio exposes and encodes `KP_SEPARATOR` (`57416`), `KP_BEGIN` (`57427~`), and ISO
level 3 shift through `NamedKey::AltGraph` (`57453`).

### IME And Dead Keys

Enhanced keyboard mode must not turn text composition into key reporting. When a
platform clears visible preedit before delivering `Ime::Commit`, Rio suppresses
the matching text-producing keyboard event and writes the committed text to the
PTY as typed input. That keeps dead-key composition aligned with Ghostty while
still allowing disambiguated chords such as `Ctrl+Alt+H` and `Ctrl+Alt+J` to use
the Kitty keyboard path.

The black-box matrix includes dead acute followed by `e` in disambiguate mode.
The expected payload is only UTF-8 `é`; a leaked `e`, bracketed paste wrapper, or
CSI-u encoded composing key is a failure.

## Compatibility Impact

Helix, Yazi, Nushell, and Zellij mostly depend on the implemented part: mode
query, push/pop around alternate screen, disambiguated Esc/Ctrl/Alt keys,
release/repeat when requested, and keypad/function keys. The gaps affect harder
cases:

- non-Latin keyboard layout shortcut matching
- apps that rely on exact stack behavior after nested keyboard-mode users
- games or TUI input test tools that inspect NumLock on platforms without a
  reliable NumLock state

## Follow-Up Work

- Keep tracking NumLock modifier-bit reporting on platforms where the
  window/input layer does not expose reliable lock state
- Decide whether niche private-use keys outside the exposed `rio-window` key set
  are implementable, or document them as platform-impossible instead of guessing
- Run black-box key captures comparing Rio, Ghostty, and Kitty for the checked-in
  Helix/Yazi/Nushell/Zellij-oriented case matrix

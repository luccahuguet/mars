# Kitty Keyboard Black-Box Fixtures

Source spec: https://sw.kovidgoyal.net/kitty/keyboard-protocol/

The automated Rio tests cover encoder helpers and the mode stack. The black-box
fixtures cover the terminal boundary: launch the capture tool inside Rio,
Ghostty, and Kitty, press the same keys, and compare the bytes delivered to the
application.

## Case Matrix

The case manifest lives at
`conformance/fixtures/kitty_keyboard_blackbox.json`.

It covers:

- `Esc` in disambiguate mode
- `Ctrl+C` in disambiguate mode
- `Ctrl+Shift+I` in disambiguate mode
- `Alt+[` in disambiguate mode
- dead acute then `e` in disambiguate mode
- keypad Left in disambiguate mode
- keypad `+` in disambiguate mode
- repeat events with associated text
- release events in report-all mode
- `Shift+A` associated text
- standalone Left Control press/release events

These are the combinations most likely to matter for Helix, Yazi, Nushell, and
Zellij because they exercise ambiguous legacy keys, dead-key composition, keypad
identity, event types, associated text, and standalone modifier reporting.

List the checked-in expectations:

```text
nix run .#yazelix-protocol-conformance -- keyboard-list
```

## Capture Workflow

Run the capture command inside the terminal being tested. For Rio from this
repository:

```text
nix develop -c target/debug/rio -e nix run .#yazelix-protocol-conformance -- keyboard-capture --terminal rio
```

For Kitty:

```text
kitty nix run .#yazelix-protocol-conformance -- keyboard-capture --terminal kitty
```

For Ghostty:

```text
ghostty -e nix run .#yazelix-protocol-conformance -- keyboard-capture --terminal ghostty
```

The command writes a capture report under
`artifacts/conformance/keyboard_captures/<terminal>.json`. It pushes the required
Kitty keyboard mode before each case and pops it afterward with `CSI < u`, so a
failed capture should not leave the terminal in an enhanced keyboard mode.

Verify a capture:

```text
nix run .#yazelix-protocol-conformance -- keyboard-verify-capture artifacts/conformance/keyboard_captures/rio.json --require-all
```

Exact cases must match the expected byte stream exactly. Repeat, release,
associated-text, and modifier-event cases use ordered containment because real
manual captures can include setup keystrokes such as a modifier press before the
event under test.

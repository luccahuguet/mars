# Kitty Rich Clipboard Provider Boundary

Reviewed on 2026-05-31.

## Evidence

- Kitty's OSC 5522 spec defines arbitrary MIME clipboard reads and writes,
  permission-denied replies, MIME-list reads, password/name trust metadata,
  paste-event private mode 5522, and multiplexer `id` echoing:
  https://sw.kovidgoyal.net/kitty/clipboard/
- Current Ghostty `main` reference is `c4eba3da3` from 2026-05-29. It has an
  OSC 5522 parser in
  `/home/lucca/pjs/open_source/yazelix_related/ghostty/src/terminal/osc/parsers/kitty_clipboard_protocol.zig`,
  but `/home/lucca/pjs/open_source/yazelix_related/ghostty/src/terminal/stream.zig`
  still routes `.kitty_clipboard_protocol` through the unimplemented OSC
  callback arm.
- Ghostty's app runtime has a richer MIME-shaped clipboard content boundary in
  `/home/lucca/pjs/open_source/yazelix_related/ghostty/src/apprt/structs.zig`
  and its GTK runtime can publish MIME content providers in
  `/home/lucca/pjs/open_source/yazelix_related/ghostty/src/apprt/gtk/class/surface.zig`.
  That is useful architecture evidence, not current OSC 5522 runtime behavior.
- Yazelix-terminal implements an OSC 5522 safe text slice in
  `rio-backend/src/crosswords/mod.rs`: parser dispatch, text/plain and
  text/plain;charset=utf-8 reads/writes, MIME-list replies, chunk limits,
  transaction failure state, and focus-policy frontend replies.
- Yazelix-terminal's actual platform clipboard boundary is still
  `rio-backend/src/clipboard.rs`, which is backed by `copypasta` and exposes
  only `String` get/set.

## Decision

Do not pretend arbitrary MIME OSC 5522 is a parser-only feature. The current
safe text implementation is enough for Ghostty parity because current Ghostty
does not implement runtime OSC 5522 behavior beyond parsing. Full Kitty rich
clipboard remains frontier work.

The rejected shortcut is accepting `image/png`, `text/html`, or arbitrary MIME
packets in the parser and then dropping, lossy text-coercing, or storing them in
terminal-local memory. That would advertise capabilities the platform clipboard
cannot actually provide and would create security confusion around reads,
writes, and paste-event preauthorization.

## Future Provider Shape

If Yazelix-terminal later chases full Kitty OSC 5522 parity, implement the
clipboard boundary first:

- `available_mimes(ClipboardType) -> Vec<String>`
- `read_mime(ClipboardType, mime) -> Vec<u8>`
- `write_mimes(ClipboardType, Vec<{ mime, bytes, aliases }>)`
- frontend permission policy for read/write, including clear EPERM replies
- password/name trust metadata scoped to the originating TTY and clipboard
  location
- DECSET/DECRST/DECRQM private mode 5522 for paste-event MIME notifications
- multiplexer `id` parsing, sanitization, and echoing on every OSC 5522 reply

Platform work should be explicit:

- macOS: `NSPasteboard` types and byte data, not just strings
- Windows: registered clipboard formats and byte handles for non-text payloads
- X11: TARGETS/selection ownership and incremental transfer handling
- Wayland: data-control or compositor-supported data offers, with clear
  unavailable behavior when the protocol is absent

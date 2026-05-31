# Frontier Kitty Protocols

Reviewed on 2026-05-31 against:

- Kitty official protocol index: https://sw.kovidgoyal.net/kitty/protocol-extensions/
- Kitty keyboard protocol: https://sw.kovidgoyal.net/kitty/keyboard-protocol/
- Kitty graphics protocol: https://sw.kovidgoyal.net/kitty/graphics-protocol/
- Kitty text sizing: https://sw.kovidgoyal.net/kitty/text-sizing-protocol/
- Kitty multiple cursors: https://sw.kovidgoyal.net/kitty/multiple-cursors-protocol/
- Kitty file transfer: https://sw.kovidgoyal.net/kitty/file-transfer-protocol/
- Kitty drag and drop: https://sw.kovidgoyal.net/kitty/dnd-protocol/
- Kitty pointer shapes: https://sw.kovidgoyal.net/kitty/pointer-shapes/
- Kitty color control: https://sw.kovidgoyal.net/kitty/color-stack/
- Kitty arbitrary-region styling: https://sw.kovidgoyal.net/kitty/deccara/
- Kitty clipboard: https://sw.kovidgoyal.net/kitty/clipboard/
- Kitty unscrolling: https://sw.kovidgoyal.net/kitty/unscroll/
- Ghostty local reference: `/home/lucca/pjs/open_source/yazelix_related/ghostty`

The implementation rule is the same as the parity contract: use Kitty's public
specs and black-box behavior for Kitty-specific protocols, and use Ghostty's MIT
source as a reference only where Ghostty already implements the same behavior.

## Current State

Already implemented or partially validated in Yazelix-terminal:

- Kitty graphics, including Unicode-placeholder graphics through the Yazelix
  stack
- Kitty keyboard protocol mode stack and CSI-u emission paths
- Sixel and iTerm2 inline image paths through the renderer
- OSC 66 parser and cell-width behavior
- OSC 99 first slice for notification parsing and display
- OSC 133 first slice for semantic prompt regions
- OSC 52 hardened clipboard policy
- OSC 21 first slice for foreground, background, cursor, and ANSI palette
  set/query/reset
- OSC 22 pointer shape set/reset, push/pop, current/support query, and
  frontend cursor selection

Important gaps found during this audit:

- OSC 21 unsupported special color keys such as cursor text, selection colors,
  visual bell, and transparent background slots still need representable terminal
  storage before they can be more than query-visible.
- OSC 5522 Kitty rich clipboard is absent. Ghostty has a dedicated parser and
  fuzz coverage for it.
- Kitty multiple cursors are absent in both Yazelix-terminal and the checked
  Ghostty source. This remains a modern Kitty-frontier feature rather than
  strict Ghostty parity.
- Kitty file transfer and OSC 72 drag/drop are absent. Both cross a security and
  OS-integration boundary and should not be treated as parser-only work.

## Must

These are needed to honestly call the protocol surface Ghostty-parity quality.

### OSC 21 Kitty Color Control

Why: Ghostty implements Kitty OSC 21. It is a cleaner modern surface for setting
and querying foreground, background, cursor, cursor text, selection colors,
visual bell, transparent background color slots, and ANSI palette entries. Rio
currently handles legacy dynamic color OSCs such as 10/11/12 and palette reset
paths, but not the Kitty keyed protocol.

Scope:

- Parse OSC 21 key/value packets
- Support set, query, and reset for the fields Yazelix-terminal can represent
- Return `key=?` for unknown query fields
- Preserve existing legacy color behavior
- Add conformance fixtures and query reply tests

### OSC 5522 Kitty Rich Clipboard

Why: Ghostty implements Kitty OSC 5522, and Kitty positions it as the modern
clipboard protocol for arbitrary MIME data and permission-aware reads/writes.
Yazelix-terminal only has OSC 52 text clipboard behavior today.

Scope:

- Parse OSC 5522 metadata and payload packets
- Support a safe first slice for `type=read`, `type=write`, `type=wdata`, and
  `type=walias`
- Gate reads and writes through the same visible policy mindset as OSC 52
- Start with `text/plain` support before image/rich-data writes
- Fail closed on unsupported MIME, invalid base64, oversized chunks, and missing
  policy decisions

### OSC 22 Pointer Shape End-To-End

Why: pointer shapes are in the Ghostty parity contract and Ghostty wires OSC 22
into terminal mouse shape state. Yazelix-terminal currently parses simple shape
names but does not preserve the Kitty stack/query semantics or route the result
through the front-end cursor decision.

Scope:

- Maintain pointer-shape stacks for normal and alternate screens
- Implement set, reset, push, pop, and support/current queries
- Map Kitty/CSS shape names to winit cursor icons where supported
- Let URL hover, selection dragging, and resize interactions override the app
  requested pointer only while those UI states are active
- Add parser and frontend behavior tests

### Existing Must-Follow Work

These already have beads and should stay ahead of new frontier features:

- Complete OSC 133 prompt navigation and selection actions
- Render OSC 66 scale and fractional text visually
- Complete OSC 99 lifecycle replies and activation behavior
- Fix the WGPU Vulkan surface issue on this COSMIC/NVIDIA setup, or clearly
  scope GL as the supported validation renderer for now

## Should

These are valuable modern-terminal features, but they do not block the first
release-quality Ghostty-parity claim.

### Kitty Multiple Cursors

Why: this is the protocol the Yazelix discussion called out directly. It lets
editors render real terminal cursors instead of fake glyphs, so it fits the
cursor-shader investment. Kitty added it in 0.43.0. The checked Ghostty source
does not appear to implement it yet, so this is frontier parity with Kitty, not
Ghostty parity.

Scope:

- Parse CSI `> ... q` extra-cursor commands
- Track extra cursors as screen-local ephemeral state
- Clear them on ED 2/3/22, reset, and normal/alternate screen switches
- Render them through the same cursor renderer path where possible, including
  cursor shader inputs
- Implement support and state queries

### Kitty Keyboard Completeness Audit

Why: Rio is listed by Kitty as a keyboard-protocol implementer, and the code has
the mode stack and CSI-u emission machinery. Before declaring parity, Yazelix
should run a spec-level audit for associated text, alternate keys, event types,
numpad/function-key mappings, mode query replies, and terminal reset behavior.

Scope:

- Compare implementation against the current official Kitty keyboard spec
- Add fixtures for the gaps that matter to Helix, Yazi, Nushell, and Zellij
- Keep winit limitations documented instead of hiding them

### Kitty Unscrolling

Why: CSI `n + T` is small and helps modern shells restore screen content after
temporary completion blocks. It is not a renderer feature, but it makes shell
UX feel current.

Scope:

- Parse `CSI <n> + T`
- Pull lines from scrollback when available
- Fall back to blank lines on alternate screen or empty scrollback
- Add focused scrollback regression tests

### Kitty DECCARA Extension

Why: arbitrary-region SGR styling is a useful modern screen mutation primitive
and likely maps well to Rio's grid cell attributes. It is not widely essential,
but the implementation shape is bounded.

Scope:

- Parse DECCARA with Kitty's all-SGR extension
- Apply SGR attributes to rectangular regions without moving the cursor
- Respect clipping, default parameters, and damage tracking
- Preserve existing BCE/erase behavior

## Cool / Frontier

These need design work or upstream-spec maturity before implementation.

### Kitty File Transfer

Why: the feature is genuinely useful over nested SSH or serial links, but it
lets untrusted terminal applications request filesystem reads/writes. Kitty's
spec requires explicit user authorization unless a shared secret is configured.
This must be designed as a product/security feature, not a parser task.

Recommended first step:

- Write a policy decision covering session approval UI, destination defaults,
  remote receive behavior, password bypass, path validation, symlink handling,
  size limits, cancel behavior, and audit/logging

### OSC 72 Drag And Drop

Why: this is very modern and potentially excellent for TUI apps, but Kitty marks
the protocol as still under development. It also requires OS drag/drop event
integration, MIME negotiation, chunking, remote-machine behavior, and security
rules for same-window drags.

Recommended first step:

- Defer full implementation until the spec settles; optionally implement only
  support-query behavior once apps start depending on it

### Color Stack Push/Pop

Why: Kitty documents OSC 30001/30101 color stack push/pop next to OSC 21, and
xterm has related push/pop/report color controls. This is useful for full-screen
apps, but it is lower priority than OSC 21 set/query because it does not unlock
new app behavior by itself.

Recommended first step:

- Add after OSC 21 if implementation is small and can reuse the same color
  snapshot model

## Out Of Scope For This Track

- Kitty remote control: it controls Kitty's own app model. Yazelix-terminal is
  deliberately not trying to replace Zellij's workspace ownership.
- Terminal-native multiplexing parity: the Yazelix mode direction is to avoid
  competing with Zellij.
- Kitty private escape codes without a stable public application use case.

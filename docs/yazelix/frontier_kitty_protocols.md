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
- Kitty keyboard audit: `docs/yazelix/kitty_keyboard_audit.md`

The implementation rule is the same as the parity contract: use Kitty's public
specs and black-box behavior for Kitty-specific protocols, and use Ghostty's MIT
source as a reference only where Ghostty already implements the same behavior.

## Current State

Already implemented or partially validated in Yazelix-terminal:

- Kitty graphics, including Unicode-placeholder graphics through the Yazelix
  stack
- Kitty keyboard protocol mode stack and CSI-u emission paths
- Kitty keyboard all-flags and stack fixture streams in the conformance harness
- Sixel and iTerm2 inline image paths through the renderer
- OSC 66 parser and cell-width behavior
- OSC 99 notification parsing/display, support query replies, alive query
  replies, and untracked close replies
- OSC 133 semantic prompt regions, prompt navigation, and output selection
- OSC 52 hardened clipboard policy
- OSC 5522 Kitty rich clipboard text/plain read/write/wdata/walias first slice
- OSC 21 keyed color set/query/reset for foreground, background, cursor,
  cursor text, selection colors, visual bell color, transparent background
  color, and ANSI palette slots
- OSC 22 pointer shape set/reset, push/pop, current/support query, and
  frontend cursor selection
- Kitty multiple cursors support/state/color queries, coordinate mutation,
  ED/reset/alternate-screen clearing, conformance fixture stream, and sprite
  rendering through the normal cursor atlas slots

Important gaps found during this audit:

- OSC 21 unsupported special color keys such as cursor text, selection colors,
  visual bell, and transparent background slots still need representable terminal
  storage before they can be more than query-visible.
- OSC 5522 Kitty rich clipboard still needs arbitrary MIME, OS-backed rich
  clipboard integration, password trust prompts, and chunk/session hardening
  beyond the current text/plain first slice.
- Kitty multiple cursors still need deeper visual parity work for exact
  reverse-video text color handling and multi-cursor Ghostty shader uniforms.
  The checked Ghostty source does not appear to implement the protocol, so this
  remains modern Kitty-frontier work rather than strict Ghostty parity.
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

Result:

- Implemented metadata/payload parser for `type=read`, `type=write`,
  `type=wdata`, and `type=walias`
- Implemented `text/plain` reads with OK/DATA/DONE replies and MIME-list reads
  without touching the system clipboard
- Implemented `text/plain` writes with transaction state, chunk append, no-op
  text/plain alias handling, and DONE/EPERM frontend replies
- Rejected unsupported MIME types, malformed base64, oversized chunks, missing
  sessions, and invalid locations with protocol error replies
- Routed actual clipboard access through frontend clipboard events so focus
  policy remains outside the parser
- Remaining limitation: non-text MIME data, platform rich clipboard APIs, and
  password-based trust prompts are intentionally deferred

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

- Render OSC 66 scale and fractional text visually
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

Result:

- Implemented parser dispatch for CSI `> ... SP q`
- Implemented shape operations for current cursor, point lists, full-screen
  rectangles, and clipped rectangles
- Implemented support, state, and color queries
- Implemented extra cursor color state for unset, special, sRGB, and indexed
  color spaces
- Implemented ED 2/3/22, reset, and alternate-screen clearing
- Rendered extra cursors through the existing block/non-block cursor atlas slots
- Remaining limitation: the grid shader still has a single block-cursor
  reverse-video position and Ghostty shader frame state still has one cursor, so
  extra cursor text-color inversion and multi-cursor shader effects need a
  follow-up renderer/shader expansion

### Kitty Keyboard Completeness Audit

Why: Rio is listed by Kitty as a keyboard-protocol implementer, and the code has
the mode stack and CSI-u emission machinery. Before declaring parity, Yazelix
should run a spec-level audit for associated text, alternate keys, event types,
numpad/function-key mappings, mode query replies, and terminal reset behavior.

Result:

- The audit lives in `docs/yazelix/kitty_keyboard_audit.md`
- The conformance harness has all-flags and stack fixture streams
- Follow-up implementation beads should cover stack edge semantics, full
  modifier-bit reporting, base-layout alternate keys, and the remaining
  functional/keypad private-use mappings

### Kitty Unscrolling

Why: CSI `n + T` is small and helps modern shells restore screen content after
temporary completion blocks. It is not a renderer feature, but it makes shell
UX feel current.

Scope:

- Parse `CSI <n> + T`
- Pull lines from scrollback when available
- Fall back to blank lines on alternate screen or empty scrollback
- Add focused scrollback regression tests

Result:

- Implemented in the experiment branch for primary-screen full-screen regions
- Non-full-screen regions and alternate screen intentionally fall back to ordinary
  blank `SD` behavior
- The conformance harness includes `kitty_unscroll_three_lines`

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

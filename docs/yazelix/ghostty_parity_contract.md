# Ghostty Parity Contract

Status: experimental target for the `yazelix-terminal` Rio fork.

This contract defines when `yazelix-terminal` is good enough to replace
Ghostty as the preferred terminal surface for Yazelix. The goal is not to
clone Ghostty's application shell. The goal is to make Rio's Rust/WebGPU
terminal core reach the visual, protocol, and reliability level Yazelix needs,
while keeping Zellij/Yazi/Helix as the workspace stack.

## Source Baseline

- Rio base: `7e18dde1c90182a5170a7cca7779544967d7291c`
- Ghostty reference clone: `/home/lucca/pjs/open_source/yazelix_related/ghostty`
- WezTerm reference clone: `/home/lucca/pjs/open_source/yazelix_related/wezterm`
- Kitty is a protocol/spec reference unless a separate licensing decision says otherwise

Ghostty and WezTerm are MIT-licensed references. Kitty's implementation is GPL;
use Kitty's public protocol documentation, black-box behavior, and tests/specs,
not implementation copying.

## Success Definition

`yazelix-terminal` reaches Ghostty parity when:

- Yazelix can launch it as the terminal host without changing Yazelix's main
  workspace ownership model
- Zellij remains the owner of panes, tabs, sessions, layouts, and focus policy
- Yazi image previews, Helix editing, shell prompts, clipboard, links, keyboard
  input, and modern terminal protocols behave at least as well as they do in
  Ghostty for normal Yazelix workflows
- all Yazelix Ghostty cursor shader presets render with equivalent shader
  inputs and visual behavior
- regressions are covered by automated parser/protocol checks and screenshot or
  framebuffer evidence for visual behavior

## Must-Have Parity

### Cursor Shader Parity

This is the first hard gate. If Rio's renderer cannot support this without a
large rewrite, the experiment must document the reason and consider a different
base.

The compatible surface is Ghostty's `custom-shader` behavior:

- GLSL/Shadertoy-style `mainImage` shader source
- post-process input texture containing the current terminal frame
- support for one or more shaders in a stable order
- actionable errors for unreadable or invalid shader files
- animation that repaints while shader time or cursor state changes
- no black-window failure mode that leaves the user without an obvious recovery

Required core uniforms:

- `iChannel0`
- `iResolution`
- `iTime`
- `iTimeDelta`
- `iFrame`
- `iChannelResolution[0]`

Required Ghostty cursor/state extensions:

- `iCurrentCursor`
- `iPreviousCursor`
- `iCurrentCursorColor`
- `iPreviousCursorColor`
- `iCurrentCursorStyle`
- `iPreviousCursorStyle`
- `iCursorVisible`
- `iTimeCursorChange`
- `iTimeFocus`
- `iFocus`
- `iPalette[256]`
- `iBackgroundColor`
- `iForegroundColor`
- `iCursorColor`
- `iCursorText`
- `iSelectionForegroundColor`
- `iSelectionBackgroundColor`

Ghostty currently exposes some Shadertoy names that may be unsupported or
placeholder-backed, such as mouse/date/audio/framerate fields. For compatibility,
their names should still compile even when values are zeroed or static.

### Core Graphics And Input Protocols

These must work through the actual Yazelix stack, not only in isolated Rio tests:

- Kitty graphics protocol, including chunking, deletion, placement, Unicode
  placeholders, virtual placements, z-index ordering, and screen/alt-screen state
- Sixel images
- iTerm2 inline images through OSC 1337 `File=...`
- Kitty keyboard protocol / CSI-u mode stack
- bracketed paste
- focus events
- SGR mouse reporting
- synchronized output using DEC private mode 2026

### Shell, Prompt, And Identity Protocols

Yazelix needs shell-aware terminal behavior because prompt boundaries, current
directory, and command output regions are where terminal UX is moving.

Required:

- OSC 7 current working directory
- OSC 133 semantic prompt / command regions
- Kitty/Ghostty semantic prompt options that affect line redraw or prompt spans
- XTVERSION response
- XTGETTCAP responses for capabilities that Yazelix, shells, Zellij, Yazi, and
  Helix may query
- terminfo that truthfully advertises the implemented capabilities

### Clipboard, Hyperlink, And Notification Basics

Required:

- OSC 8 hyperlinks with correct span lifetime, line wrapping, and reset behavior
- OSC 52 clipboard read/write policy with visible, secure failure behavior
- dynamic colors for foreground, background, cursor, palette set/query/reset
- OSC 9/777 notifications and OSC 9;4 progress reporting, gated by sane user
  policy
- OSC 22 mouse pointer shape where supported by the platform

### Yazelix Mode

`yazelix-terminal` must provide a Yazelix-owned launch/config mode that is small
and explicit:

- one command surface for launching Yazelix's runtime command
- generated or documented config under a Yazelix-owned path
- no hidden terminal-native mux, tabs, split panes, or session persistence that
  competes with Zellij
- terminal-native split/mux code may remain upstream code only if disabled or
  irrelevant to Yazelix mode
- clear app id/window class behavior so desktop entries and launchers are not
  duplicated or confusing

## Should-Have Parity

These are expected before calling the fork release-quality, but they should not
block the first cursor-shader proof:

- Kitty OSC 66 text sizing
- parser fixtures copied from observed Ghostty/WezTerm/Kitty behavior where
  licensing allows
- fuzzing or property coverage for escape sequence parsers
- visual screenshot harness for graphics, hyperlinks, colors, and cursor shaders
- performance benchmarks against Ghostty on large scrollback, image-heavy
  panes, and animated shaders
- config migration/documentation for users moving from Ghostty-style shader
  config to Rio/Yazelix config
- theme/light/dark notification behavior where the platform supports it

## Frontier Protocols

These are modern or exploratory features worth tracking, but they are not
required for the first usable Yazelix terminal:

- Kitty multicursor protocol, if the public spec and real application behavior
  prove useful
- Kitty file transfer protocol
- advanced terminal annotations or command-output navigation surfaces
- richer prompt/command region actions once OSC 133 state is stable
- image animation beyond what current Kitty graphics support needs for Yazelix
- terminal-side integrations that can replace brittle `zellij write-chars`
  flows without replacing Zellij as workspace owner

## Absorption Rules

Use source reading aggressively, but with ownership discipline:

- Prefer Rio's existing architecture when adding behavior
- Use Ghostty for shader semantics, OSC 133/66 behavior, renderer state, and
  protocol tests where the MIT license allows it
- Use WezTerm for alternate Rust-oriented implementation ideas and behavior
  cross-checks
- Use Kitty's public docs/specs and black-box behavior for Kitty protocols
- Preserve attribution and license notices whenever source-derived code is used
- If the clean implementation requires large copied subsystems, document the
  ownership boundary before coding

## Validation Evidence

Each parity claim needs at least one durable artifact:

- parser/unit tests for escape sequence behavior
- protocol smoke tests that run a real PTY stream through Rio
- screenshots or framebuffer captures for visual output
- manual Yazelix session evidence through Zellij, Yazi, Helix, and shell prompt
- benchmark notes for performance-sensitive renderer changes
- source references naming the upstream file/commit used for behavior

The exact acceptance standard is behavior, not API shape. If Rio implements a
feature differently from Ghostty but the user-visible Yazelix behavior is equal
or better, that satisfies parity.

## Pivot Criteria

Stop the current implementation path, document the hard problem, and move to
another bead if any of these happen:

- cursor shader uniforms require replacing the renderer instead of extending it
- shaders can only work on one backend while Yazelix needs a different backend
  as the default target
- protocol parser ownership makes modern OSC/APC/DCS additions fragile without
  first refactoring the parser
- the terminal-native split/session model cannot be cleanly disabled for
  Yazelix mode
- a license boundary would force copying GPL implementation code into the fork

Pivots are not failures. They are evidence that the next bead should either
shrink the scope, change the implementation order, or reassess Rio as the base.

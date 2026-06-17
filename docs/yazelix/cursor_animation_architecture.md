# Cursor Animation Architecture

Status: active policy for Mars Terminal dogfooding.

Date: 2026-06-02.

## Decision

Rio's native `trail-cursor` is the primary Mars Terminal cursor animation.
It stays enabled in the packaged `full`/`default` profile.

Ghostty-compatible `custom-shader` cursor presets remain supported, packaged,
and tested, but they are opt-in through the `shaders` profile.

When the shader profile is used with `trail-cursor = true`, Rio's trail cursor
is still the cursor motion owner. The Ghostty shader uniform path consumes the
Rio trail's animation state through additive Yazelix uniforms and marks cursor
motion as externally animated, so shader cursor movement does not open a
separate redraw window or compute an independent cursor transition.

## Evidence

- Rio documents `trail-cursor` as a built-in smooth cursor trail using spring
  physics: <https://rioterm.com/docs/config#effects>
- Ghostty documents `custom-shader` as a postprocess shader chain over the
  current terminal texture, with cursor uniforms and an optional animation loop:
  <https://ghostty.org/docs/config/reference#custom-shader>
- Local dogfooding on 2026-06-02 first showed the focus-regain lag and fast
  catch-up rendering bug improve when `custom-shader` was removed from the
  generated `mars` config while `trail-cursor = true` stayed enabled. The
  bug later reproduced without custom shaders, so shader stacking is not the
  proven root cause.
- Before `yzt-unify-rio-trail-shader-cursor-cho`, local code had two
  independent animation paths:
  - `frontends/rioterm/src/renderer/trail_cursor.rs` owns Rio's spring trail.
  - `sugarloaf/src/components/ghostty_shaders/mod.rs` owns shader time,
    previous/current cursor uniforms, and shader animation invalidation.
- Runtime diagnostics on 2026-06-03 show that the WebGPU `custom-shader`
  pipeline and generated `mars` configuration load in fresh windows: a
  full-screen diagnostic shader tints the terminal, config reloads swap the
  diagnostic color, and fresh packaged windows can see
  `MARS_RIO_TRAIL`.
- Follow-up probes on 2026-06-03 isolated the shader visibility failure to
  uniform placement, not config or frame-state collection: a macro probe
  rendered cyan, a base `iCursorVisible` probe rendered green, and an
  `iYazelixRioTrailActive` probe rendered blue while
  `MARS_SHADER_STATE_LOG` simultaneously reported
  `rio_trail_active=true`. The Rio trail extension fields were therefore moved
  before the large extra-cursor arrays in the std140 block and covered with
  layout/order tests.

That combination was useful for compatibility testing, but it was not an
elegant cursor architecture. The current path keeps one cursor motion owner:
Rio trail drives motion, while the shader runtime remains responsible for
postprocess time, colors, palette, focus, extra cursors, and shader-only cursor
motion when `trail-cursor` is disabled.

## Profiles

| Profile | Renderer | Rio Trail | Ghostty Shaders | Cursor Blink | Purpose |
| --- | --- | --- | --- | --- | --- |
| `full`, `default`, `effects` | WebGPU | enabled | disabled | enabled, 650 ms | Dogfooding profile with Rio's native trail |
| `baseline`, `no-effects`, `none` | WebGPU | disabled | disabled | enabled, 650 ms | No-effects comparison profile |
| `shaders`, `cursor-shaders`, `ghostty-shaders` | WebGPU | enabled | enabled | enabled, 650 ms | Compatibility and visual-effect diagnostics |

`MARS_RENDER_STRATEGY=game` remains a renderer scheduling
diagnostic. It composes with each profile, but it does not imply shader use.

## Diagnostics

Set `MARS_SHADER_STATE_LOG=/path/to/shader_state.jsonl` before
launching the terminal to record active-panel shader frame-state changes. The
logger is disabled when the variable is unset. It records only state changes,
including:

- the active cursor render style and blink visibility
- the cursor visual extent width/height
- whether Rio trail cursor state was available
- the Rio trail snapshot stage and sampled cell size
- the Rio trail gate reason: `active`, `no_rio_trail_snapshot`,
  `cursor_extent`, or `cursor_not_rendered`
- whether the shader cursor is externally animated by Rio

## Integration Policy

Shader work should build on top of Rio trail instead of replacing it.
The implemented integration is:

- `TrailCursor::shader_state()` exposes the terminal destination rectangle, the
  animated trail bounding rectangle, the four animated trail corners, and the
  animation-active bit.
- `screen::render` keeps standard Ghostty cursor uniforms on the terminal
  cursor destination, then feeds Rio trail geometry into
  `GhosttyShaderFrameState.rio_trail` for the active one-cell cursor when
  `trail-cursor` is enabled.
- `GhosttyShaderFrameState.cursor_externally_animated` tells the shader runtime
  to ignore cursor-rect motion for redraw rearming and to keep Ghostty
  `iPreviousCursor == iCurrentCursor` for externally animated cursor motion.
- Wider OSC 66 cursor extents and shader-only configurations keep the existing
  Ghostty previous/current cursor transition behavior.
- `MARS_RIO_TRAIL` is defined only by the Mars Terminal shader
  wrapper. Yazelix cursor shaders must guard all Rio-specific uniform reads with
  `#if defined(MARS_RIO_TRAIL)` so the same generated shader files
  remain valid in Ghostty.

## Rio Trail Shader ABI

The canonical mars shader ABI is documented in
[`shader_abi.md`](shader_abi.md). This section summarizes the Rio trail pieces
that drive cursor animation.

The Ghostty-compatible uniforms keep their original meaning. The Yazelix
extension is declared in the same std140 uniform block before the large
extra-cursor arrays:

| Uniform | Meaning |
| --- | --- |
| `iYazelixRioTrailActive` | non-zero when the active cursor is a one-cell cursor using Rio trail state |
| `iYazelixRioTrailAnimating` | non-zero while Rio's trail spring is still visibly moving |
| `iYazelixRioTrailDestinationCursor` | destination cursor rectangle as `x, bottom_y, width, height`, matching Ghostty cursor-uniform coordinates |
| `iYazelixRioTrailAnimatedCursor` | bounding rectangle of Rio's animated trail as `x, bottom_y, width, height` |
| `iYazelixRioTrailCorners[4]` | animated Rio trail corners as `x, y, 0, 0` in drawable pixels, top-left coordinate space, ordered top-left, top-right, bottom-right, bottom-left |

Yazelix-owned cursor shaders use this extension to apply spread, glow, edge,
and core masks over Rio's actual trail geometry. Third-party Ghostty shaders
continue to read only the standard Ghostty uniforms unless they explicitly opt
into the Yazelix extension.

Acceptable future designs:

- non-cursor postprocess shaders that treat the already-rendered Rio trail in
  `iChannel0` as part of the terminal frame
- an explicit compatibility mode that intentionally stacks Ghostty cursor
  shaders over Rio trail for parity investigations

The default profile must not enable `custom-shader`, and the shader profile must
not compute an independent Ghostty cursor trail while Rio's trail is active.

## Validation Matrix

- package config: `share/mars/config.toml` has
  `trail-cursor = true` and no `custom-shader`
- baseline config: `share/mars/baseline/config.toml` has neither
  `trail-cursor` nor `custom-shader`
- shader profile:
  `share/mars/profiles/shaders/config.toml` has both
  `trail-cursor = true` and the packaged `custom-shader` chain
- wrapper smoke: `tools/yazelix_event_mode_smoke.sh` verifies all profile
  contents and starts the default, baseline, and shader profiles
- benchmark harness: `yzt-default` means Rio trail only; `yzt-shaders` means
  the opt-in shader stack on top of Rio trail

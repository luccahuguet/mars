# Cursor Shader Parity Dossier

## Feature

- Feature: Ghostty-compatible cursor-aware custom shaders
- Bead: `yzt-7p3.6`
- Parity tier: must-have
- Status: complete research, ready for implementation

## Source Inventory

| Project | Commit | License posture | Files/specs/probes |
| --- | --- | --- | --- |
| Rio | `a941221c87a69d62f906774ffb96937dc5207c60` | fork base | `sugarloaf/src/sugarloaf.rs`, `sugarloaf/src/components/filters/mod.rs`, `sugarloaf/src/grid/cell.rs`, `frontends/rioterm/src/screen/mod.rs`, `frontends/rioterm/src/grid_emit.rs`, `frontends/rioterm/src/renderer/trail_cursor.rs`, `rio-backend/src/config/renderer.rs` |
| Ghostty | `c4eba3da38c629dbd4b8f770da3e0c605f2a7f53` | MIT | `src/renderer/shadertoy.zig`, `src/renderer/shaders/shadertoy_prefix.glsl`, `src/renderer/generic.zig`, `src/renderer/metal/shaders.zig`, `src/renderer/opengl/shaders.zig`, `src/config/Config.zig` |
| WezTerm | not used | MIT | not needed for this proof |
| Kitty docs/specs | not used | spec/black-box | not needed for this proof |
| Other | `librashader 0.11.0`, `wgpu 29.0.3`, `glslang 0.8.1`, `spirv-cross2 0.7.1` | existing Rio dependency path when `wgpu` is enabled | `sugarloaf/Cargo.toml`, workspace `Cargo.toml` |

## Behavior Target

Yazelix-terminal must accept Ghostty-style custom shader source and make it useful for cursor effects. The user-facing surface should be close enough that existing Ghostty cursor shader files can run with little or no editing:

- Shadertoy-style fragment shader with `mainImage(out vec4 fragColor, in vec2 fragCoord)`
- terminal framebuffer exposed as `iChannel0`
- per-frame values: `iResolution`, `iTime`, `iTimeDelta`, `iFrameRate`, `iFrame`, channel times/resolutions, mouse/date/sample-rate placeholders
- cursor values: `iCurrentCursor`, `iPreviousCursor`, `iCurrentCursorColor`, `iPreviousCursorColor`, `iCurrentCursorStyle`, `iPreviousCursorStyle`, `iCursorVisible`, `iTimeCursorChange`
- focus values: `iFocus`, `iTimeFocus`
- terminal colors: `iPalette[256]`, `iBackgroundColor`, `iForegroundColor`, `iCursorColor`, `iCursorText`, `iSelectionBackgroundColor`, `iSelectionForegroundColor`
- repeated shader paths run in order as postprocess passes
- shader compile/load failures must be visible in logs and must not make the terminal unusable

## Current Rio State

Rio already has the pieces needed to host this without a renderer rewrite:

- `Sugarloaf::render_wgpu` renders grid/text/UI into the surface texture, then runs an optional postprocess `FiltersBrush`
- `FiltersBrush` copies the surface texture into a source texture and applies WGPU filter chains
- WGPU feature builds already pull `wgpu`, `glslang`, `spirv-cross2`, and `librashader`
- `GridUniforms` already carries active cursor position, cursor foreground swap color, cursor block fill color, cell size, grid size, padding, and colorspace flags
- `frontends/rioterm/src/screen/mod.rs` already computes panel origin, cell dimensions, cursor shape, cursor visibility, cursor blink visibility, cursor color, active/focused panel state, and terminal colors
- `grid_emit::emit_cursor_sprite` already has the glyph rectangle data needed for bar, underline, block, and hollow cursor geometry
- `TrailCursor` already tracks animated cursor destination and previous destination style data for Rio's trail effect

What Rio does not have yet:

- a Ghostty/Shadertoy shader loader that accepts Ghostty's fragment-shader shape
- a `GhosttyShaderUniforms` block matching Ghostty's uniform ABI/names
- a postprocess brush that binds `iChannel0` plus that uniform block
- current/previous cursor rectangle tracking in framebuffer pixels
- theme/palette/selection color export into shader uniforms
- focus/time/frame animation invalidation tied to custom shader presence
- config surface for `custom-shader`/animation mode

The existing `FiltersBrush` is not enough by itself. RetroArch/librashader filters do not expose Ghostty's cursor-specific uniforms, and overloading that path would make cursor parity depend on a filter API that was built for CRT/scanline chains. The cleaner path is a sibling `GhosttyShaderBrush` that shares the WGPU postprocess phase and source-texture copy pattern.

## Candidate Implementation

Smallest Rio-native path:

1. Add a renderer config surface for Ghostty-compatible shaders, initially WGPU-only:
   - `renderer.custom-shader = ["path/to/shader.glsl"]` or a top-level `custom-shader` alias if matching Ghostty exactly is preferred
   - `renderer.custom-shader-animation = true | false | "always"` once animation policy is needed
2. Add `sugarloaf/src/components/ghostty_shaders/`:
   - Shadertoy prefix matching Ghostty uniform names and cursor style macros
   - WGPU fullscreen triangle pipeline
   - uniform buffer mirroring Ghostty's `shadertoy.Uniforms`
   - source texture sampler bound as `iChannel0`
3. Keep `FiltersBrush` separate:
   - render order should be grid/text/UI, then Ghostty shaders, then optional RetroArch filters only if the user enables both
   - if combining both is too ambiguous, make it explicit in docs/config and fail clearly for unsupported combinations
4. Lift shader-state snapshots from `frontends/rioterm/src/screen/mod.rs` into Sugarloaf:
   - active panel cursor rect in drawable pixels
   - previous cursor rect/color/style retained by the shader brush
   - terminal palette/theme/selection colors
   - focus/time/frame counters
5. Make the implementation degrade explicitly:
   - if a custom shader is configured without WGPU support, fail config validation or log a clear unsupported-backend error
   - do not silently fall back to CPU or native Vulkan without shaders

Expected owner files:

- `rio-backend/src/config/renderer.rs` for config fields
- `frontends/rioterm/src/screen/mod.rs` for extracting shader frame state from panels
- `sugarloaf/src/sugarloaf.rs` for brush ownership and render order
- `sugarloaf/src/components/ghostty_shaders/*` for WGPU pipeline, uniforms, source copy, and shader compilation
- `conformance/shaders/ghostty_cursor_probe.glsl` as the first visual/compile probe

## License And Attribution Decision

This can be implemented as in-house Rio/Yazelix code using Ghostty's MIT source as behavioral reference with attribution. Do not copy Kitty implementation code for this feature; Kitty is not needed for the renderer proof.

The Shadertoy uniform names, cursor-style macro names, and config semantics should intentionally match Ghostty. If code is source-derived from Ghostty's `shadertoy.zig`, `generic.zig`, or shader prefixes, keep attribution in the new module header and commit message.

## Validation Plan

- unit/fixture tests:
  - config parses shader paths and animation mode
  - Shadertoy prefix exposes all Ghostty uniform names used by `conformance/shaders/ghostty_cursor_probe.glsl`
  - uniform packing size/alignment is stable and 16-byte aligned
  - cursor state transitions update previous/current cursor and `iTimeCursorChange`
- PTY/conformance smoke:
  - `python3 tools/yazelix_conformance.py verify`
  - emit a cursor-moving shell probe and keep the frame loop alive while shader animation is enabled
- screenshot/framebuffer evidence:
  - capture a WGPU window with `conformance/shaders/ghostty_cursor_probe.glsl`
  - verify visually that the shader color follows cursor movement and changes when focus/cursor visibility changes
- manual Yazelix session:
  - launch Yazelix using the WGPU build, open Helix/Yazi, move cursor rapidly, and confirm no frame stalls or stale cursor rectangles
- benchmark:
  - compare idle focused, idle unfocused, and rapid cursor movement with shaders disabled/enabled

## Risks

- The local Nix/host graphics stack currently blocks WGPU screenshot evidence.
- WGPU-only support is probably acceptable for the first implementation because Rio's existing filter chain is already WGPU-only, but it should be explicit.
- Native Vulkan/Metal parity would require separate postprocess implementations if WGPU is not made the shader-required backend.
- Cursor rectangle semantics must match Ghostty closely. Block, hollow block, bar, underline, preedit, blink-hidden, and unfocused states need separate cases.
- Multi-panel Rio layouts need a policy: expose only the active panel cursor at first, or expose per-panel cursor data later.
- Shader animation can raise idle CPU/GPU cost; animation policy must be configurable.
- Shader compile failures must not become silent visual no-ops.

## Pivot Criteria

Pivot away from the WGPU brush path only if:

- WGPU cannot be made reliable on the target Linux/macOS systems where Yazelix-terminal must run
- WGPU source texture postprocess cannot sample and render back to the surface safely
- Ghostty-compatible GLSL cannot be translated to WGSL/SPIR-V with the existing dependency stack
- Cursor/palette state cannot be captured before `Sugarloaf::render_wgpu` without duplicating terminal render-state ownership

None of those pivot criteria were proven by source inspection. The current blocker is local graphics validation, not renderer architecture.

## Outcome

- Implemented: no runtime shader code in this bead; this bead is a renderer feasibility proof
- Evidence:
  - `nix develop -c cargo build -p rioterm --features wgpu` passed
  - WGPU config parsing reaches `sugarloaf/src/context/webgpu.rs`, proving `backend = "Webgpu"` is honored
  - source audit shows cursor, palette, focus, cell, and postprocess seams already exist
  - `python3 tools/yazelix_conformance.py verify` passed
- Local launch blockers:
  - native Vulkan: `vkCreateInstance failed ... ERROR_INCOMPATIBLE_DRIVER`
  - WGPU/GL on Wayland/X11: `Request adapter: NotFound ... incompatible_surface_backends: Backends(GL)`
  - WGPU/Vulkan on Wayland/X11: `CreateSurfaceError { inner: Hal(FailedToCreateSurfaceForAnyBackend({})) }`
  - host launch outside Nix misses dynamic Wayland/X11 helper libraries
- Remaining gaps:
  - implement `GhosttyShaderBrush`
  - add shader config surface
  - add uniform packing and cursor state tests
  - fix or bypass local GPU launch so visual shader screenshots can be captured
- Follow-up beads:
  - `yzt-7p3.7` for the Ghostty-compatible cursor shader runtime
  - create a dedicated graphics-environment bead if WGPU visual validation remains blocked after runtime code exists

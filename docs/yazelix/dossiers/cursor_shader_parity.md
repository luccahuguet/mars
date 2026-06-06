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
- `grid_emit::cursor_sprite_cell` already has the glyph rectangle data needed for bar, underline, block, and hollow cursor geometry
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

## Runtime Implementation Update

`yzt-7p3.7` added the first WGPU implementation slice:

- `[renderer] custom-shader = ["path/to/shader.glsl"]`
- WGPU-only `GhosttyShaderBrush` beside `FiltersBrush`
- Shadertoy prefix exposing Ghostty cursor/color/focus/palette uniform names
- std140-compatible `GhosttyShaderUniforms`
- active-panel cursor rectangle, cursor style, palette, terminal colors, and window focus state flowing into Sugarloaf before present
- Naga GLSL pre-parse before creating the WGPU shader module, so invalid shader files produce a logged load error instead of being blindly handed to the render path
- render order: Rio grid/text/UI, Ghostty shaders, then existing librashader filters

The implementation is source-compatible with the checked-in
`conformance/shaders/ghostty_cursor_probe.glsl` probe.

`yzt-7p3.26.1` added a Yazelix-specific multi-cursor extension on top of the
Ghostty-compatible surface for Kitty's multiple-cursor protocol:

- `iYazelixExtraCursorCount`
- `iYazelixExtraCursors[256]`, as cursor rectangles in drawable pixels
- `iYazelixExtraCursorColors[256]`
- `iYazelixExtraCursorStyles[256]`, with `.x` using the same cursor-style enum

The standard Ghostty `iCurrentCursor`/`iPreviousCursor` fields remain unchanged
for source compatibility. The extension is intentionally additive because
Ghostty does not currently expose a multi-cursor shader ABI.

## Visual Validation Update

`yzt-7p3.17` fixed the WGPU surface path enough for local screenshot validation:

- Sugarloaf now passes its raw display handle into `wgpu::InstanceDescriptor`
  instead of constructing WGPU without a display handle
- `python3 tools/yazelix_conformance.py launch-wgpu-shader-screenshot` launches
  the checked-in shader probe
- `artifacts/shader_probe/screenshots/wgpu_shader_probe_gl.png` captures the
  WGPU shader-probe window and shows the magenta cursor block produced through
  the Ghostty cursor uniform path

`yzt-7p3.22` re-tested the same probe with `WGPU_BACKEND=vulkan` on the local
COSMIC Wayland/NVIDIA host. WGPU/Vulkan no longer reproduces the earlier
`CreateSurfaceError`, and the harness default is Vulkan again.

- Visual evidence:
  `artifacts/shader_probe/screenshots/wgpu_shader_probe_vulkan.png`
- GL remains useful as an explicit fallback probe:
  `python3 tools/yazelix_conformance.py launch-wgpu-shader-screenshot --wgpu-backend gl`

## Yazelix Preset Validation

`yzt-7p3.8` validated the generated Yazelix Ghostty cursor preset set from
`~/.local/share/yazelix/configs/terminal_emulators/ghostty/shaders`:

- generated top-level trail shaders and `generated_effects/*.glsl` validate
  through Naga's GLSL frontend when `YAZELIX_GHOSTTY_SHADER_DIR` is set
- the default Ghostty stack from the current Yazelix config
  (`cursor_trail_dusk.glsl`, `generated_effects/sweep.glsl`,
  `generated_effects/rectangle_boom.glsl`) launches through WGPU/GL
- visual evidence is captured at
  `artifacts/shader_probe/screenshots/yazelix_default_cursor_stack_gl.png`

Compatibility shim: the Shadertoy wrapper now appends Rio's generated `main()`
after the user shader body. WGPU/Naga accepts simple forward declarations, but
the generated Yazelix shaders exposed validator failures when `main()` appeared
before the full user function graph. Appending the wrapper preserves Ghostty's
`mainImage` authoring surface while satisfying WGPU's stricter GLSL path.

## Cursor Animation Ownership Update

Dogfooding on 2026-06-02 first suggested the focus-regain lag and fast catch-up
rendering bug improved when the generated `yzxterm` config stopped loading
`custom-shader` while keeping Rio `trail-cursor = true`. The bug later
reproduced without custom shaders, so this is no longer root-cause evidence.

The architecture policy is therefore:

- Rio `TrailCursor` owns default cursor motion.
- Ghostty-compatible cursor shaders remain supported for parity work through an
  explicit shader profile.
- Future shader effects should build on the already-rendered Rio trail in
  `iChannel0`, or consume a Yazelix extension exposing Rio trail state, instead
  of independently computing another cursor trail in the default profile.
- The remaining focus catch-up bug should be investigated separately from this
  profile/default architecture cleanup.

## 2026-06-06 Dogfooding State

Fresh Yazelix `yzxterm` windows are launching through the yzxterm-specific
runtime and launch-scoped config, even when Home Manager's active Yazelix
runtime is `ratty`. Runtime selection is therefore not the current shader
visibility blocker. A representative fresh yzxterm launch used:

- `YAZELIX_TERMINAL=yzxterm`
- `YAZELIX_TERMINAL_PROFILE=shaders`
- `RIO_CONFIG_HOME=$YAZELIX_STATE_DIR/terminal_launches/.../configs/terminal_emulators/yzxterm`
- `[renderer] backend = "Webgpu"`
- `[renderer] custom-shader = [".../cursor_trail_magma.glsl"]`
- `[effects] trail-cursor = true`

Old running yzxterm windows can keep showing older glow behavior after a
runtime update because every desktop launch copies a shader snapshot into its
own `terminal_launches/<id>/...` directory. Those windows are useful evidence
that the terminal shader engine still works, but they do not reflect the newly
generated shader policy.

The current pushed state is:

- `yazelix-cursors` `99569c7`: generated shaders use
  `iYazelixRioTrailAnimating` to decide when to replace the normal
  Ghostty-style trail SDF with Rio's exported animated trail geometry.
- `yazelix` `153cd551`: main runtime pins that cursor package and enables
  Rio's native trail cursor in the packaged Rio runtime config.
- Fresh yzxterm windows show Rio's native cursor animation and the shader's
  split cursor fill. This confirms the shader is loading and the split cursor
  path is no longer completely hidden by Rio's native single-color cursor.

The remaining visible gap is movement glow spread. During movement, the native
Rio cursor animation looks good, but the shader's wider glow/aura contribution
is not yet visibly spreading around the moving cursor. The next tuning should
start in `yazelix-cursors`, by making the movement-only aura/glow layer stronger
while keeping idle glow small. The terminal fork should only be changed if
evidence shows Rio's native cursor pass or shader composition order is
overwriting the shader output.

Vanilla Rio is now the intended comparison baseline. The Home Manager Rio
config was missing native trail animation, so
`~/.config/home-manager/modules/rio.nix` now includes:

```toml
[effects]
trail-cursor = true
```

Testing vanilla Rio without that setting does not compare against yzxterm's
native Rio trail behavior.

Follow-up observation from comparing fresh windows on the same host: plain Rio
and packaged Yazelix Rio resolve to the same `rioterm 0.4.6` binary, so the
visible animation difference is not explained by a Rio version mismatch. The
important config difference is that packaged Yazelix Rio and yzxterm both
force:

```toml
[renderer]
backend = "Webgpu"
```

while the plain Rio Home Manager config does not set an explicit renderer
backend. This was validated by launching plain Rio with an isolated temp config
that added only `backend = "Webgpu"` to the otherwise working host Rio config.
That window switched from the default edge-style trail to the same filled
block/rectangle trail seen in packaged Yazelix Rio and yzxterm.

Conclusion: Rio's native trail cursor has different visual behavior between the
default renderer path and the wgpu renderer path selected by
`backend = "Webgpu"`. Plain Rio with the default renderer looks more like an
edge trail. Plain Rio with `backend = "Webgpu"`, packaged Yazelix Rio, and
yzxterm look more like a filled rectangle stretched from source to destination
and then retracted toward the final cursor position. Do not treat that
block/retract animation as yzxterm-specific unless a future probe finds a
separate fork-local difference.

Source inspection of the installed Rio `0.4.6` package shows this is not two
separate cursor algorithms. `TrailCursor::draw` always emits one convex quad as
two `sugarloaf.triangle(...)` calls. On Linux, Rio's implicit default is native
Vulkan, while `backend = "Webgpu"` selects Sugarloaf's wgpu renderer. Both
renderers have a triangle-list geometry path, but the same animated trail
geometry is perceived differently through those backend paths. yzxterm's fork
adds large-jump snapping, short vertical-move handling, and shader-state export;
those changes affect when the trail is drawn and how shaders can follow it, but
they do not explain the packaged Rio vs plain Rio animation-shape difference.

# Yzxterm Shader ABI

This document describes the shader ABI owned by `yazelix-terminal`. It is the
contract between packaged yzxterm profiles, the Rio-derived renderer, and main
Yazelix cursor settings.

## Boundary

Main Yazelix is terminal-agnostic. It may select a yzxterm package/profile and
pass stable cursor settings that the yzxterm package advertises as supported.
It must not generate Rio-specific shader code, depend on yzxterm shader file
names, or reuse yzxterm extension uniforms for Ghostty, Kitty, WezTerm, Ratty,
or host terminal modes.

The corresponding main boundary bead is
`yazelix:yazelix-yzxterm-boundary-cleanup-1l7rd`. Terminal-side shader profile
ownership is documented in
[`shader_profile_ownership.md`](shader_profile_ownership.md).

## Standard Ghostty-Compatible Surface

Yzxterm custom shaders use Shadertoy-style GLSL source with
`mainImage(out vec4 fragColor, in vec2 fragCoord)`. The shader wrapper provides
the current terminal frame through `iChannel0` and composes one or more shaders
in the order listed by `[renderer].custom-shader`.

Core Shadertoy names that should compile:

| Uniform or define | Meaning |
| --- | --- |
| `iChannel0` | Postprocess input texture containing the current terminal frame |
| `iResolution` | Drawable resolution as `width, height, scale_or_1` |
| `iTime` | Runtime seconds since the shader brush first rendered |
| `iTimeDelta` | Seconds since the previous shader frame |
| `iFrameRate` | Placeholder/static-compatible frame-rate field |
| `iFrame` | Monotonic shader frame counter |
| `iChannelTime[4]` | Placeholder-compatible channel time array |
| `iChannelResolution[4]` | Channel dimensions; channel 0 tracks the drawable resolution |
| `iMouse`, `iDate`, `iSampleRate` | Compatibility fields that may be zeroed or static |

Ghostty-compatible cursor and terminal-state names:

| Uniform or define | Meaning |
| --- | --- |
| `iCurrentCursor` | Current cursor rectangle as `x, bottom_y, width, height` in drawable pixels |
| `iPreviousCursor` | Previous cursor rectangle in the same coordinate system |
| `iCurrentCursorColor` | Current cursor color as RGBA floats |
| `iPreviousCursorColor` | Previous cursor color as RGBA floats |
| `iCurrentCursorStyle` | Current cursor style integer |
| `iPreviousCursorStyle` | Previous cursor style integer |
| `iCursorVisible` | Non-zero when the cursor is visible |
| `iTimeCursorChange` | Runtime seconds when cursor shader state last changed |
| `iTimeFocus` | Runtime seconds when focus state last changed |
| `iFocus` | Non-zero when the terminal is focused |
| `iPalette[256]` | Terminal palette as RGB float vectors |
| `iBackgroundColor` | Effective background color as RGB floats |
| `iForegroundColor` | Effective foreground color as RGB floats |
| `iCursorColor` | Effective cursor color as RGB floats |
| `iCursorText` | Effective cursor text color as RGB floats |
| `iSelectionForegroundColor` | Effective selection foreground color as RGB floats |
| `iSelectionBackgroundColor` | Effective selection background color as RGB floats |
| `CURSORSTYLE_BLOCK` | Cursor style value `0` |
| `CURSORSTYLE_BLOCK_HOLLOW` | Cursor style value `1` |
| `CURSORSTYLE_BAR` | Cursor style value `2` |
| `CURSORSTYLE_UNDERLINE` | Cursor style value `3` |
| `CURSORSTYLE_LOCK` | Cursor style value `4` |

Yzxterm keeps these standard names available so Ghostty-style shader source can
compile and render without knowing about Rio trail internals.

`iCursorVisible` follows the renderer's effective cursor visibility, not just
the terminal's logical cursor-visible state. Blink-off frames export no normal
cursor rectangle, set `iCursorVisible` to zero, and suppress Rio trail extension
state so shader glow and native cursor blinking do not fight each other.

## Yzxterm Extension Surface

The yzxterm shader wrapper defines:

```glsl
#define YAZELIX_TERMINAL_RIO_TRAIL 1
```

Every shader that reads Rio-specific uniforms must guard those reads with
`#if defined(YAZELIX_TERMINAL_RIO_TRAIL)` and provide a valid fallback path.
This is required so generated shader files remain usable as Ghostty-compatible
source when the yzxterm extension is absent.

Rio trail extension uniforms:

| Uniform | Meaning |
| --- | --- |
| `iYazelixRioTrailActive` | Non-zero when the active cursor is a one-cell cursor using Rio trail state |
| `iYazelixRioTrailAnimating` | Non-zero while Rio's trail spring is visibly moving |
| `iYazelixRioTrailDestinationCursor` | Terminal cursor destination rectangle as `x, bottom_y, width, height`, matching Ghostty cursor-uniform coordinates |
| `iYazelixRioTrailAnimatedCursor` | Bounding rectangle of Rio's animated trail as `x, bottom_y, width, height` |
| `iYazelixRioTrailCorners[4]` | Animated Rio trail corners as `x, y, 0, 0` in drawable pixels, top-left coordinate space, ordered top-left, top-right, bottom-right, bottom-left |

Extra cursor extension uniforms:

| Uniform | Meaning |
| --- | --- |
| `iYazelixExtraCursorCount` | Number of visible extra cursor cells exported to the shader ABI |
| `iYazelixExtraCursors[256]` | Extra cursor rectangles as `x, bottom_y, width, height` |
| `iYazelixExtraCursorColors[256]` | Extra cursor colors as RGBA floats |
| `iYazelixExtraCursorStyles[256]` | Extra cursor style values in `.x`; remaining components are padding |

The std140 uniform block declares Rio trail extension fields before the large
extra-cursor arrays. This order is intentional and covered by tests because
driver/Naga paths observed during dogfooding failed when the Rio trail fields
lived after the tail arrays.

## Main Yazelix Cursor Inputs

Main Yazelix may provide stable settings such as a supported profile name and a
named cursor glow level when yzxterm package metadata advertises those settings.
The current packaged shader assets use a medium glow profile.

Main Yazelix may assume:

- `full`, `baseline`, and `shaders` are stable profile names when advertised
- `shaders` means the yzxterm-owned opt-in Ghostty-compatible shader chain
- `full` means Rio native `trail-cursor` without a custom shader chain
- `baseline` means no cursor effects

Main Yazelix may not assume:

- which GLSL files implement `shaders`
- that Rio extension uniforms exist outside yzxterm
- that yzxterm shader glow/spread constants map to Ghostty or other terminals
- that shader profile internals are stable without package metadata saying so

## Validation

Cheap validation:

- `python3 tools/yazelix_conformance.py verify` checks the fixture manifest,
  Ghostty cursor probe shader, keyboard manifest, and yzxterm packaged shader
  assets.
- `git diff --check` catches formatting damage in docs and generated assets.

Focused Rust tests, when the compile-heavy gate permits them:

- `sugarloaf` `ghostty_uniform_layout_matches_std140_offsets` checks the
  uniform block size and key std140 offsets.
- `sugarloaf` `shadertoy_prefix_exposes_ghostty_cursor_names` checks standard
  and extension names exposed by the wrapper.
- `sugarloaf` `rio_trail_extension_macro_selects_yazelix_branch` checks guarded
  yzxterm extension reads.
- `sugarloaf` `rio_trail_extension_uniforms_validate_as_user_shader_reads`
  checks direct user shader reads of the Rio trail extension.
- `sugarloaf` `rio_trail_extension_uniforms_precede_extra_cursor_arrays` checks
  the layout-order invariant.

Visual validation:

- Shader aura/spread behavior requires screenshot or framebuffer evidence before
  a visual correctness claim.
- `cursor_trail_dusk.glsl` must retain Rio trail SDF, glow, edge, and core masks
  so the shader profile decorates Rio's actual trail geometry instead of
  computing an unrelated Ghostty-only cursor trail.

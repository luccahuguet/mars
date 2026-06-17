# Mars Terminal Shader Profile Ownership

This document records which cursor shader and profile assets are owned by
Mars Terminal's current `mars` package surface and which inputs may
come from main Yazelix.

## Ownership Boundary

Mars Terminal owns every Rio-aware shader behavior detail:

- the packaged Rio config profiles under `share/mars`
- the packaged emoji fallback profile roots under `share/mars/emoji`
- the WebGPU `custom-shader` profile shape
- the generated GLSL files installed under `share/mars/shaders`
- the guarded Rio trail extension uniform reads
- the choice to keep the default profile on Rio's native `trail-cursor`

Main Yazelix must not infer Rio shader behavior from config paths, shader file
names, or generated GLSL internals. It may only select stable user-facing
profiles and cursor settings that Mars Terminal exposes as supported.

## Packaged Profiles

| Profile | Config path | Terminal-owned behavior |
| --- | --- | --- |
| `full` / `default` | `share/mars/config.toml` | WebGPU backend, Rio native `trail-cursor`, no custom shader chain |
| `baseline` / `none` | `share/mars/baseline/config.toml` | WebGPU backend, no cursor effects, performance comparison baseline |
| `shaders` | `share/mars/profiles/shaders/config.toml` | WebGPU backend, Rio native `trail-cursor`, opt-in Ghostty-compatible custom shader chain |

The desktop wrapper selects these profiles through `MARS_PROFILE`
or the compatibility alias `MARS_EFFECTS=none`. The diagnostic
`MARS_RENDER_STRATEGY=game` knob creates a runtime config copy but
does not change ownership of the profile assets.

`MARS_EMOJI_FONT` selects the packaged emoji fallback preset before
profile selection. `noto` uses the default profile roots above, while `twitter`
and `serenityos` use matching profile roots under
`share/mars/emoji/<preset>/`.

## Packaged Shader Assets

| Asset | Installed path | Ownership decision |
| --- | --- | --- |
| `cursor_trail_dusk.glsl` | `share/mars/shaders/cursor_trail_dusk.glsl` | Terminal-owned Rio-aware cursor trail shader. It can use `MARS_RIO_TRAIL` only behind preprocessor guards so the source stays Ghostty-compatible. |
| `generated_effects/sweep.glsl` | `share/mars/shaders/generated_effects/sweep.glsl` | Terminal-owned generated copy of a vendored Ghostty cursor effect template. It reads only standard Ghostty-compatible uniforms. |
| `generated_effects/rectangle_boom.glsl` | `share/mars/shaders/generated_effects/rectangle_boom.glsl` | Terminal-owned generated copy of a vendored Ghostty cursor effect template. It reads only standard Ghostty-compatible uniforms. |

The effect templates are not the long-term profile source of truth. The
packaged generated files are the terminal-owned release artifacts and must be
reviewed when the shader ABI changes.

## Main Yazelix Inputs

Main Yazelix may choose among supported mars profiles and emoji fallback
presets, request a release or fast package, and pass stable cursor settings
such as a named glow level when a future package metadata surface advertises
that capability.

Main Yazelix must not:

- generate Rio extension uniform reads
- splice shader files into the mars package config
- assume `profiles/shaders` contains a specific file list
- reuse mars shader settings for Ghostty, Kitty, WezTerm, Ratty, or host
  terminal modes

This keeps Ghostty-compatible shader behavior available for mars visual
diagnostics without moving Rio-specific assumptions into the terminal-agnostic
main Yazelix runtime.

## Compatibility Split

The custom shader runtime remains Ghostty-compatible for standard cursor
uniforms. Mars-specific reads are guarded by
`#if defined(MARS_RIO_TRAIL)` and must have a valid fallback path.
The full mars shader uniform contract is documented in
[`shader_abi.md`](shader_abi.md).

The default profile intentionally does not enable `custom-shader`. The shader
profile is opt-in because it is a compatibility and visual-diagnostic surface,
not the default Mars Terminal cursor animation path.

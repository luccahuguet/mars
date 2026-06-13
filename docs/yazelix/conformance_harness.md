# Protocol Conformance Harness

The local protocol harness lives at `tools/yazelix_protocol_conformance`. The
flake app is the stable command surface, and the isolated Cargo package is the
direct development entrypoint.

The harness has three explicit scopes:

- protocol conformance fixtures for stable byte streams and manifest checks
- visual/rendering probes for shader, CPU renderer, and screenshot evidence
- behavior comparison targets for black-box checks against Ghostty, WezTerm,
  Kitty, or other terminals

Protocol authority and comparison targets are separate. Kitty owns Kitty
protocol specs. Ghostty is the primary behavior and quality target for yzxterm.
WezTerm is the mature independent terminal-engine comparison target.

## Commands

List checked-in fixture streams:

```text
nix run .#yazelix-protocol-conformance -- list
```

Validate protocol fixture manifest bytes, fixture metadata, keyboard manifest
metadata, and the cursor shader ABI probe:

```text
nix run .#yazelix-protocol-conformance -- verify
```

List Kitty keyboard black-box capture cases:

```text
nix run .#yazelix-protocol-conformance -- keyboard-list
```

Capture and verify terminal keyboard bytes for Rio, Kitty, Ghostty, or WezTerm:

```text
nix run .#yazelix-protocol-conformance -- keyboard-capture --terminal rio
nix run .#yazelix-protocol-conformance -- keyboard-verify-capture artifacts/conformance/keyboard_captures/rio.json --require-all
```

Emit one fixture byte stream to a terminal or file:

```text
nix run .#yazelix-protocol-conformance -- emit osc133_semantic_prompt
```

Record local source/version evidence:

```text
nix run .#yazelix-protocol-conformance -- record-env
```

Launch the built Rio binary with CPU renderer and capture a COSMIC screenshot:

```text
nix run .#yazelix-protocol-conformance -- launch-cpu-screenshot
```

Launch the built WGPU Rio binary with the Ghostty-compatible cursor shader
probe and capture a COSMIC screenshot:

```text
nix run .#yazelix-protocol-conformance -- launch-wgpu-shader-screenshot
```

Pass `--shader` more than once to validate a Ghostty-style shader chain:

```text
nix run .#yazelix-protocol-conformance -- launch-wgpu-shader-screenshot \
  --shader /path/to/cursor_trail_dusk.glsl \
  --shader /path/to/generated_effects/sweep.glsl \
  --shader /path/to/generated_effects/rectangle_boom.glsl
```

The WGPU renderer probe config lives at
`artifacts/shader_probe/rio_wgpu_config/config.toml`. It sets the WGPU backend
and loads the checked-in Ghostty cursor probe:

```toml
[renderer]
backend = "Webgpu"
custom-shader = ["conformance/shaders/ghostty_cursor_probe.glsl"]
```

Use it when validating shader work so failures clearly belong to WGPU or the
host graphics stack, not to Rio's default native Vulkan backend. On a working
GPU stack, that probe should compile through Naga's GLSL frontend and tint the
cursor area from `iCurrentCursor`.

The default screenshot command uses `WGPU_BACKEND=vulkan`. The local
COSMIC/Wayland/NVIDIA stack now creates a WGPU/Vulkan surface for the shader
probe, so Vulkan is the primary validation path for cursor-shader work. Pass
`--wgpu-backend gl` only when investigating a host-specific Vulkan regression.

## Protocol Fixture Scope

The first manifest covers:

- OSC 8 hyperlinks
- OSC 52 clipboard query
- Kitty OSC 5522 rich clipboard text/plain and UTF-8 text MIME read/write stream
- Kitty OSC 21 keyed color control
- Kitty OSC 22 pointer shapes
- OSC 133 semantic prompt regions
- Kitty OSC 66 text sizing
- Kitty OSC 99 desktop notification
- Kitty keyboard mode query
- Kitty keyboard all-flags query
- Kitty keyboard stack push/union/pop/query stream
- Kitty keyboard black-box key-event case matrix
- Kitty unscrolling CSI plus T
- Kitty multiple cursors support/set/state/color/clear stream
- Kitty graphics 1x1 RGBA transmit/place
- minimal Sixel DCS path
- synchronized output DECSET 2026
- XTVERSION
- XTGETTCAP RGB query

Each fixture declares:

| Field | Meaning |
| --- | --- |
| `kind` | Fixture bucket: `protocol`, `visual-probe`, or `comparison` |
| `source` | Short source category: `kitty-spec`, `kitty-behavior`, `ghostty-behavior`, `wezterm-behavior`, `xterm`, `iterm2`, `de-facto`, or `rio-implementation` |
| `comparison_targets` | External terminals to black-box compare against, currently `kitty`, `ghostty`, and `wezterm` |
| `reference` | Human-readable note explaining the concrete source or comparison behavior |

The fixtures are not proof that Rio supports every protocol correctly. They are
stable byte streams that future beads can feed into Rio, Ghostty, WezTerm, or
black-box probes and compare against expected behavior.

The Kitty keyboard black-box matrix is separate from `manifest.json` because it
is a manual capture protocol, not a byte stream sent to the terminal. It is
documented in `docs/yazelix/kitty_keyboard_blackbox.md`.

## Visual And Rendering Probes

`conformance/shaders/ghostty_cursor_probe.glsl` is a minimal shader that reads
the standard Ghostty-compatible cursor uniforms. It is not a visual parity target
by itself. Its job is to fail early when a renderer path cannot compile or
populate the names Yazelix cursor presets depend on.

CPU renderer screenshots prove process launch and window rendering only. They do
not prove shader parity.

## Comparison Targets

Ghostty remains the primary behavior and quality comparison target for yzxterm,
especially for shader ABI behavior and terminal-app compatibility. WezTerm is a
mature terminal-engine comparison target. Kitty is the normative source for
Kitty-owned protocol specs, not an implementation source to copy from.

When a fixture records Ghostty behavior, it uses `source = "ghostty-behavior"`
so that the harness does not imply that every fixture is a Ghostty conformance
test.

When a fixture records a Kitty-owned protocol, it uses `source = "kitty-spec"`
and still lists Ghostty and WezTerm in `comparison_targets` when black-box runs
against those terminals are useful.

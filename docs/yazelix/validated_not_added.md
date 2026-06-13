# Validated, Not Added

This file is intentionally separate from
`docs/yazelix/fork_feature_verification.md`.

Rows here must not be counted as Yazelix Terminal features. They are evidence
that the fork checked inherited Rio behavior, confirmed an environment property,
measured a baseline, audited a source boundary, or explicitly deferred support.

## Rules

- If Yazelix Terminal implements, packages, or changes behavior, record it in
  `fork_feature_verification.md`
- If Yazelix Terminal only verifies, measures, audits, or defers behavior,
  record it here
- Parser-only behavior is not runtime support
- A benchmark result is not a feature
- A source audit is not a feature

## Baseline And Feasibility Evidence

| Surface | What was validated, measured, or audited | Evidence |
| --- | --- | --- |
| Upstream Rio build and launch baseline | Established the upstream Rio build/launch baseline before fork features; recorded native Vulkan launch failure and CPU renderer launch success on the local host | `~/.cargo/bin/cargo build -p rioterm`; `nix develop -c cargo build -p rioterm`; CPU renderer screenshot evidence |
| Cursor shader feasibility | Proved Rio had cursor, palette, focus, and WGPU postprocess seams that made Ghostty-style shaders feasible; this proof did not itself add shader runtime support | `nix develop -c cargo build -p rioterm --features wgpu`; source audit in `docs/yazelix/dossiers/cursor_shader_parity.md`; `nix run .#yazelix-protocol-conformance -- verify` |
| WGPU/Vulkan screenshot validation | Validated shader screenshots with `WGPU_BACKEND=vulkan` on the local COSMIC Wayland/NVIDIA host; this was environment evidence for the screenshot path | `nix run .#yazelix-protocol-conformance -- launch-wgpu-shader-screenshot`; `cargo check --manifest-path tools/yazelix_protocol_conformance/Cargo.toml`; `nix run .#yazelix-protocol-conformance -- verify`; screenshot `artifacts/shader_probe/screenshots/wgpu_shader_probe_vulkan.png` |
| Stack validation commands | Recorded reproducible focused stack checks; these commands are evidence for other fixes, not features by themselves | `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' graphics_namespace -- --nocapture`; `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' yazelix_mode -- --nocapture`; `nix develop -c cargo test -p rio-backend --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_virtual -- --nocapture`; `nix develop -c cargo build -p rioterm --features wgpu`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |

## Inherited Rio And Terminal Behavior

These rows are coverage evidence for behavior inherited from Rio or from
standard terminal semantics. The fork keeps them covered because nearby fork
work can regress them, but they are not Yazelix-added features.

| Surface | Inherited or pre-existing behavior validated | Evidence |
| --- | --- | --- |
| Kitty graphics base path | Validated the existing renderer path for a 1x1 RGBA Kitty image transmit/place fixture and stack image previews; fork-owned Kitty graphics fixes are recorded separately in `fork_feature_verification.md` | `nix run .#yazelix-protocol-conformance -- verify`; direct Kitty Unicode-placeholder screenshot; Yazi Kitty preview through Zellij/Yazelix |
| OSC 8 hyperlinks | Kept the existing hyperlink parser/renderer behavior in the conformance fixture set | `nix run .#yazelix-protocol-conformance -- verify` fixture `osc8_hyperlink` |
| Synchronized output | Kept existing DECSET 2026 behavior in the conformance fixture set | `nix run .#yazelix-protocol-conformance -- verify` fixture `synchronized_output` |
| XTVERSION and XTGETTCAP | Kept existing terminal identity/capability replies in the conformance fixture set | `nix run .#yazelix-protocol-conformance -- verify` fixtures `xtversion_query` and `xtgettcap_rgb` |

## Benchmark Results

Benchmark results prove local behavior on a measured host. They are not
features. The harness itself is a fork-owned tooling addition and is recorded in
`fork_feature_verification.md`.

| Surface | What was measured | Evidence |
| --- | --- | --- |
| Startup and scroll smoke benchmark | Recorded local release-build startup/exit timing, scrollback stress smoke timing, raw CSV artifacts, screenshots, methodology, and limitations against Ghostty 1.3.1 | `nix develop -c cargo build -p rioterm --release`; benchmark commands in `docs/yazelix/performance_and_graphics_benchmark.md`; Python stats readback; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| Comparable Rio/Ghostty frame runs | Recorded one local release sample per case for Rio WGPU scroll, Ghostty scroll, Rio shader idle, throttled Rio shader idle, and Ghostty shader idle | `python3 -m py_compile tools/yazelix_benchmark.py`; `python3 tools/yazelix_benchmark.py self-test`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; artifacts under `artifacts/benchmarks/frame_time/` |

## Parser-Only Or Boundary Evidence

| Surface | What was audited or bounded | Evidence |
| --- | --- | --- |
| Full arbitrary-MIME OSC 5522 | Current Ghostty `main` parses OSC 5522 but leaves runtime handling unimplemented; Yazelix Terminal implements only the text/plain-compatible OSC 5522 runtime slice. Full Kitty arbitrary-MIME support still requires a MIME-tagged platform clipboard provider beyond the current `copypasta` `String` boundary | Source audit of Ghostty `c4eba3da3`; `docs/yazelix/kitty_rich_clipboard_provider.md`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |

## Explicitly Deferred Runtime Support

These entries are not partial support. They are explicit non-support until the
named boundary changes.

| Surface | Deferred boundary | Evidence |
| --- | --- | --- |
| Kitty OSC 72 drag and drop | Runtime support is deferred because the current window event boundary exposes path-level drops but not MIME offers, per-move cell/pixel coordinates, data request handles, final operation reports, same-window source/drop denial, or Wayland coverage | Re-checked official Kitty DnD spec; audited `rio-window` drop boundaries; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |

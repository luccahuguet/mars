# Yazelix Terminal Fork Feature Verification

Status: evidence ledger for fork work completed through commit `420e8565be`.

The fork starts from Rio `7e18dde1c90182a5170a7cca7779544967d7291c` and targets
the Ghostty-level terminal behavior Yazelix needs while keeping Zellij, Yazi,
and Helix as the workspace stack. This document summarizes the implemented fork
delta and the verification evidence recorded for each feature or protocol
surface.

Related docs:

- `docs/yazelix/ghostty_parity_contract.md`
- `docs/yazelix/frontier_kitty_protocols.md`
- `docs/yazelix/conformance_harness.md`
- `docs/yazelix/dossiers/cursor_shader_parity.md`
- `docs/yazelix/performance_and_graphics_benchmark.md`
- `docs/yazelix/stack_validation.md`

## Common Gates

These checks were used repeatedly as the fork-wide regression floor:

```bash
nix develop -c cargo fmt --check
nix develop -c cargo check -p rioterm
nix develop -c cargo build -p rioterm
nix develop -c cargo build -p rioterm --features wgpu
python3 tools/yazelix_conformance.py verify
git diff --check
```

The final Ghostty-parity close used:

```bash
nix develop -c cargo fmt --check
nix develop -c cargo check -p rioterm
python3 tools/yazelix_conformance.py verify
git diff --check
```

`cargo check -p rioterm` still reports the pre-existing
`rio-window/src/platform_impl/linux/x11/ime/context.rs` function-item cast
warning. It is not introduced by the Yazelix fork work.

## Fork And Harness Foundation

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Lineage and guardrails | Records Rio base commit, reference clones, source-use rules, main Yazelix integration boundary, and hard-problem pivot rules | Documentation review and `git diff --check` |
| Baseline build and launch | Establishes the upstream Rio build/launch baseline before fork features; records native Vulkan launch failure and CPU renderer launch success on the local host | `~/.cargo/bin/cargo build -p rioterm`; `nix develop -c cargo build -p rioterm`; CPU renderer screenshot evidence |
| Source absorption workflow | Documents how Ghostty, WezTerm, and Kitty evidence can be used without crossing license boundaries | Documentation review and `git diff --check` |
| Conformance harness | Provides checked-in byte fixtures, environment capture, shader probe validation, keyboard black-box capture helpers, and screenshot launch helpers | `python3 tools/yazelix_conformance.py verify`; `list`; `emit`; `record-env`; `launch-cpu-screenshot`; `python3 -m py_compile tools/yazelix_conformance.py` |
| Parser robustness | Provides deterministic parser noise smoke coverage and documents when to escalate to cargo-fuzz/libFuzzer | `cargo fmt`; focused `rio-backend` parser test; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `cargo check -p rioterm` |

## Cursor Shaders And Rendering

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Cursor shader feasibility | Proves Rio has the cursor, palette, focus, and WGPU postprocess seams needed for Ghostty-style shaders | `nix develop -c cargo build -p rioterm --features wgpu`; source audit in `docs/yazelix/dossiers/cursor_shader_parity.md`; `python3 tools/yazelix_conformance.py verify` |
| Ghostty-compatible shader runtime | Implements `[renderer] custom-shader`, WGPU `GhosttyShaderBrush`, Shadertoy `mainImage` wrapper, Ghostty cursor/color/focus/palette uniforms, Naga GLSL parse gate, and render hook before existing filters | `cargo check -p rioterm --features wgpu`; `sugarloaf` Ghostty shader tests; `rio-backend` custom shader config test; `python3 tools/yazelix_conformance.py verify`; dev build |
| WGPU screenshot path | Validates local WGPU surface creation for shader screenshot capture by passing the raw display handle into WGPU instance creation | `python3 tools/yazelix_conformance.py launch-wgpu-shader-screenshot`; `python3 -m py_compile tools/yazelix_conformance.py`; `cargo check -p rioterm --features wgpu`; `sugarloaf` Ghostty shader tests with Linux window features; `python3 tools/yazelix_conformance.py verify`; screenshot `artifacts/shader_probe/screenshots/wgpu_shader_probe_gl.png` |
| WGPU/Vulkan validation | Validates shader screenshots with `WGPU_BACKEND=vulkan` on the local COSMIC Wayland/NVIDIA host | `python3 tools/yazelix_conformance.py launch-wgpu-shader-screenshot`; `python3 -m py_compile tools/yazelix_conformance.py`; `python3 tools/yazelix_conformance.py verify`; screenshot `artifacts/shader_probe/screenshots/wgpu_shader_probe_vulkan.png` |
| Yazelix Ghostty shader presets | Validates the generated Yazelix Ghostty cursor shader stack and moves the Shadertoy wrapper after user shader source for Naga compatibility | `cargo fmt`; `sugarloaf` Ghostty shader tests with `YAZELIX_GHOSTTY_SHADER_DIR`; `cargo check -p rioterm --features wgpu`; `python3 -m py_compile tools/yazelix_conformance.py`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; manual screenshot inspection; screenshot `artifacts/shader_probe/screenshots/yazelix_default_cursor_stack_gl.png` |
| Shader redraw throttling | Replaces immediate game-mode redraw requests with a vblank-interval scheduler tick | `cargo fmt --check`; `cargo check -p rioterm`; `cargo build -p rioterm --release`; throttled frame-run benchmark; `python3 tools/yazelix_benchmark.py self-test`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; benchmark dropped shader idle from 69,688 presented frames and about 96.9% sampled CPU to 237 presented frames and about 2.5% sampled CPU over the same 4s workload |

## Yazelix Host Mode And Stack Validation

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Yazelix terminal host mode | Provides `--yazelix`, requires `-e/--command`, defaults app id to `yazelix-terminal`, sets `TERM_PROGRAM=yazelix-terminal`, and disables Rio split/config-editor ownership in Yazelix mode | `rioterm` `yazelix_mode` tests; `cargo fmt --check`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| Desktop Wayland event wakeup | Calls `pre_present_notify()` only for frames that will actually present, keeps the packaged desktop config on Rio's default event renderer strategy, and preserves an explicit `YAZELIX_TERMINAL_RENDER_STRATEGY=game` diagnostic overlay | `sh -n misc/yazelix_terminal_desktop.sh`; `git diff --check`; `cargo +1.96 fmt -- --check`; `cargo +1.96 check -p rioterm --features wgpu`; manual COSMIC/Wayland desktop launcher typing, `yzx enter`, and snappiness test |
| Stack graphics validation | Validates Yazelix under `target/debug/rio --yazelix` with WGPU/GL; fixes atlas Sixel/iTerm rendering, Kitty `U=1` virtual placement dimensions, virtual source rectangles, and child terminal identity | Focused `rioterm` and `rio-backend` tests; WGPU build; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; direct Sixel screenshot; direct Kitty Unicode-placeholder screenshot; Yazi Kitty preview through Zellij/Yazelix; Helix-in-stack screenshot evidence |
| Stack validation commands | Records reproducible focused stack checks | `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' graphics_namespace -- --nocapture`; `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' yazelix_mode -- --nocapture`; `nix develop -c cargo test -p rio-backend --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_virtual -- --nocapture`; `nix develop -c cargo build -p rioterm --features wgpu`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |

## Core Graphics And Terminal Fixtures

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Kitty graphics | Validates the renderer path for a 1x1 RGBA Kitty image transmit/place fixture and stack image previews | `python3 tools/yazelix_conformance.py verify`; direct Kitty Unicode-placeholder screenshot; Yazi Kitty preview through Zellij/Yazelix |
| Sixel | Bridges atlas Sixel graphics into the renderer image-overlay path | Focused stack tests; `python3 tools/yazelix_conformance.py verify`; direct Sixel screenshot |
| iTerm2 OSC 1337 images | Bridges atlas iTerm image graphics into the renderer image-overlay path | Focused stack tests; WGPU build; `python3 tools/yazelix_conformance.py verify`; stack screenshot evidence |
| OSC 8 hyperlinks | Keeps OSC 8 in the conformance fixture set | `python3 tools/yazelix_conformance.py verify` fixture `osc8_hyperlink` |
| Synchronized output | Keeps DECSET 2026 in the conformance fixture set | `python3 tools/yazelix_conformance.py verify` fixture `synchronized_output` |
| XTVERSION and XTGETTCAP | Keeps terminal identity/capability replies in the conformance fixture set | `python3 tools/yazelix_conformance.py verify` fixtures `xtversion_query` and `xtgettcap_rgb` |

## Shell And Prompt Protocols

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| OSC 133 semantic zones | Implements typed parser for `A/B/C/D/I/L/N/P`, Ghostty/Kitty options, production dispatch, and row-level prompt/input/output state | `cargo fmt --check`; `rio-backend` `semantic_prompt` tests with platform features; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| OSC 133 prompt actions | Implements prompt navigation and command-output selection actions on top of semantic zone state | `cargo fmt --check`; `rio-backend` `semantic_` tests; `rioterm` `semantic_prompt_actions_parse_from_config_strings`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `nix develop -c cargo check -p rioterm` |

## Clipboard, Colors, Pointer, And Notifications

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| OSC 52 clipboard policy | Rejects unsupported clipboard designators, invalid base64, non-UTF-8 payloads, and oversized encoded/decoded payloads with visible warnings | `osc52` backend test; `cargo fmt`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| OSC 99 notification parser | Parses Kitty OSC 99 metadata/title/body/base64 payloads, accumulates chunked title/body by id, and reuses the existing desktop notification event | `kitty_notification` tests; `cargo fmt --check`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| OSC 99 lifecycle replies | Sanitizes notification ids, parses close-report metadata, tracks live ids for alive replies, answers support queries, and handles backend close requests | `cargo fmt --check`; `rio-backend` `kitty_notification` tests; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `nix develop -c cargo check -p rioterm` |
| OSC 99 OS-backed handles | Preserves report/buttons/close metadata, routes Linux D-Bus replace/close/action events back to the originating PTY, and documents macOS/Windows as explicitly untracked until delegate layers exist | `cargo fmt --check`; `cargo test -p rio-notifier`; `cargo test -p rio-backend ... kitty_notification`; `cargo check -p rioterm`; `cargo build -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; live Rio OSC 99 smoke |
| OSC 21 color control | Implements keyed set/query/reset for foreground, background, cursor, palette, unsupported query replies, and conformance fixture | Focused tests recorded in Beads; `python3 tools/yazelix_conformance.py verify` fixture `osc21_kitty_color_control` |
| OSC 21 special colors | Stores and queries `cursor_text`, selection colors, `visual_bell`, `transparent_background`, and Ghostty-compatible `second_transparent_background`; wires cursor text and selection colors into rendering and shader uniforms | `cargo fmt --check`; `rio-backend` `kitty_color` tests; `nix develop -c cargo check -p rioterm`; `nix develop -c cargo check -p rioterm --features wgpu`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| OSC 21 visual effects | Renders `visual_bell` as a fading BEL overlay and transparent-background color as transparent-window composition when no background image is configured | `cargo fmt --check`; `rioterm` `visual_bell` test; `rioterm` `transparent_background` test; `rio-backend` `second_transparent` test; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `cargo check -p rioterm`; `cargo build -p rioterm` |
| OSC 22 pointer shapes | Implements set/reset, push/pop, current/support queries, alternate-screen stacks, reset clearing, frontend cursor selection, and conformance fixture | Focused parser/frontend tests recorded in Beads; `python3 tools/yazelix_conformance.py verify` fixture `osc22_pointer_shapes` |

## Kitty Keyboard

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Spec audit and fixtures | Audits Kitty keyboard parity, adds all-flags and stack fixture streams, and creates concrete gap beads | `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Mode stack semantics | Implements compact stack behavior, oldest-entry eviction on full push, pop-to-empty reset, and a shared mode-sync helper | `cargo fmt --check`; `nix develop -c cargo test -p rio-backend --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' keyboard_mode -- --nocapture`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `nix develop -c cargo check -p rioterm` |
| Modifier bits | Implements Hyper and Meta modifier bits and keeps CapsLock/NumLock explicit instead of fabricated before platform lock-state support | `cargo fmt --check`; `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_ -- --nocapture`; `nix develop -c cargo check -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Base-layout alternate keys | Reports PC-101 physical letters, digits, and punctuation through Kitty base-layout alternate-key fields | `cargo fmt --check`; `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_ -- --nocapture`; `nix develop -c cargo check -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Functional/keypad mappings | Fills exposable table holes including numpad comma, numpad clear/begin, and ISO level 3 shift through AltGraph | `cargo fmt --check`; `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_ -- --nocapture`; `nix develop -c cargo check -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Black-box comparison harness | Provides capture/verify commands and a case manifest for Rio/Ghostty/Kitty keyboard comparison | `python3 tools/yazelix_conformance.py verify`; `python3 tools/yazelix_conformance.py keyboard-list`; synthetic `keyboard-verify-capture`; `python3 -m py_compile tools/yazelix_conformance.py`; `git diff --check` |
| Lock-state modifiers | Threads CapsLock/NumLock through Linux XKB, Windows, Web, and macOS CapsLock; reports lock bits only in enhanced Kitty keyboard mode | `cargo fmt --check`; `rioterm` Kitty keyboard tests; `rio-backend` `keyboard_mode` tests; `cargo check -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |

Known limits:

- macOS NumLock and backends without reliable NumLock remain unavailable
- private-use keys that `rio-window` does not expose distinctly remain
  unavailable

## Kitty OSC 66 Text Sizing

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Protocol parser and width support | Parses scale, declared width, fractional scale, alignment, and text payload; dispatches to `input_sized_text`; advances cursor by client-declared width/scale | `text_sizing` and `osc66` `rio-backend` tests with platform features; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| Visual scaled/fractional rendering | Stores sizing metadata in the Extras side table and renders scaled/fractional runs without widening `Square` | `cargo fmt --check`; `git diff --check`; `python3 tools/yazelix_conformance.py verify`; focused `rio-backend` `text_sizing` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; local CPU-renderer OSC 66 screenshot probe |
| Multicell editing semantics | Handles top-row overwrite erasure, lower-row skip, width-fit wrapping, ECH/EL/ED erasure, ICH/DCH edit erasure, and IL/DL line edit erasure | `cargo fmt --check`; `git diff --check`; `python3 tools/yazelix_conformance.py verify`; `rio-backend` `text_sizing` tests; `cargo check -p rioterm`; `cargo build -p rioterm` |
| Cursor visual extents | Expands block/hollow-block/bar/underline cursor sprites and Ghostty shader cursor rectangles over sized-text visual extents | `cargo fmt --check`; `git diff --check`; `python3 tools/yazelix_conformance.py verify`; `rioterm` `text_sizing_cursor` tests; `cargo check -p rioterm`; `cargo build -p rioterm` |

## Kitty Multiple Cursors

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Parser/state/rendering | Implements CSI `> ... SP q`, support/state/color queries, point/rectangle/current-coordinate mutation, ED/reset/alternate-screen clearing, conformance fixture, backend tests, and frontend sprite rendering | Backend tests recorded in Beads; `python3 tools/yazelix_conformance.py verify` fixture `kitty_multiple_cursors_query_set_clear` |
| Shader and reverse-video rendering | Carries up to 256 extra block cursor cells with independent background/text-swap colors through CPU/WGPU/Metal/Vulkan shader paths and adds Yazelix shader ABI extension uniforms | `cargo fmt --check`; `git diff --check`; `python3 tools/yazelix_conformance.py verify`; `sugarloaf` `ghostty_` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; local CPU-rendered Kitty multiple-cursor screenshot probe |

Known limit: the shader/uniform path exports up to 256 visible extra cursor
cells. State can track larger requests, but exact shader/reverse-video parity is
bounded.

## Other Kitty Protocols

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Kitty unscrolling | Implements primary-screen full-screen `CSI <n> + T` line restore from scrollback, with non-full-screen and alternate-screen regions falling back to ordinary blank scroll-down behavior | `python3 tools/yazelix_conformance.py verify` fixture `kitty_unscroll_three_lines`; focused scrollback regression tests recorded in Beads |
| Kitty DECCARA | Implements all-SGR rectangular styling, common DECSACE wrapper handling, and RGB/indexed SGR tails | Parser/handler/grid tests; `python3 tools/yazelix_conformance.py verify` fixture `kitty_deccara_all_sgr` |
| Kitty OSC 72 drag and drop | Explicitly defers runtime support because the current window event boundary exposes path-level drops but not MIME offers, per-move cell/pixel coordinates, data request handles, final operation reports, same-window source/drop denial, or Wayland coverage | Re-checked official Kitty DnD spec; audited `rio-window` drop boundaries; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |

## Kitty OSC 5522 Rich Clipboard

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Text-compatible OSC 5522 slice | Implements metadata/payload parser, `type=read`, `type=write`, `type=wdata`, `type=walias`, focus-policy frontend replies, MIME-list replies, transaction state, chunk limits, and text/plain storage | Focused OSC 5522 tests recorded in Beads; `python3 tools/yazelix_conformance.py verify` fixture `osc5522_rich_clipboard_text_plain` |
| UTF-8 text MIME aliases | Advertises and accepts `text/plain` and `text/plain;charset=utf-8`; rejects non-text aliases instead of pretending arbitrary-MIME support | `cargo fmt --check`; focused `osc5522` tests; `cargo check -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Arbitrary-MIME boundary | Documents that current Ghostty `main` parses OSC 5522 but leaves runtime handling unimplemented; full Kitty arbitrary-MIME support requires a MIME-tagged platform clipboard provider beyond the current `copypasta` `String` boundary | Source audit of Ghostty `c4eba3da3`; `docs/yazelix/kitty_rich_clipboard_provider.md`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |

## Kitty File Transfer

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Security policy | Documents the OSC 5113 trust model, approval boundaries, safe roots, staged writes, local-read limits, and rejected advanced features | `git diff --check` |
| Parser and deny-by-default skeleton | Parses file-transfer commands, denies send/receive starts by default, rejects out-of-order commands, and performs no filesystem access before approval | `cargo fmt --check`; `rio-backend` file-transfer tests; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; `cargo check -p rioterm`; `cargo build -p rioterm` |
| Approved remote-to-local writes | Routes approval through frontend notification, stages incoming files under an explicit transfer directory, normalizes paths, supports regular file/directory writes, and handles size/cancel/finish cleanup | `cargo fmt --check`; `rio-backend` `osc5113` and `kitty_file_transfer` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| File-transfer state extraction | Moves OSC 5113 send/write state out of `crosswords/mod.rs` into `crosswords/kitty_file_transfer.rs` | `cargo fmt --check`; `rio-backend` `osc5113` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Approved local-to-remote reads | Collects paths before approval, shows preview body, bounds path count/traversal depth, rejects symlink parents, lists regular file/directory metadata, streams one regular file at a time, and handles cancel/finish | `cargo fmt --check`; `rio-backend` `osc5113` and `kitty_file_transfer` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Zlib compression | Supports zlib compression for regular-file send and receive data | Focused file-transfer tests; `python3 tools/yazelix_conformance.py verify` fixture `kitty_file_transfer_zlib_file` |
| Explicit advanced-feature rejections | Proves `pw`/shared-secret metadata does not bypass approval, symlink/hardlink file types are rejected without staged files, and rsync/delta transmission rejects before sessions/prompts | `cargo fmt --check`; focused `osc5113` tests; `kitty_file_transfer` parser tests; `cargo check -p rioterm`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |

Current policy rejections:

- destination chooser UX
- symlink and hardlink creation/traversal
- metadata preservation beyond bytes/path/kind/size
- generic shared-secret or password bypass
- rsync/delta transmission

## Benchmark And Performance Evidence

| Surface | Implemented or documented behavior | Verification evidence |
| --- | --- | --- |
| Startup and scroll smoke benchmark | Records local release-build startup/exit timing, scrollback stress smoke timing, raw CSV artifacts, screenshots, methodology, and limitations against Ghostty 1.3.1 | `nix develop -c cargo build -p rioterm --release`; benchmark commands in `docs/yazelix/performance_and_graphics_benchmark.md`; Python stats readback; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Frame-time harness | Provides env-gated Rio frame JSONL logging, stdlib frame benchmark runner/summarizer, `/proc` CPU/RSS sampling, and built-in scroll/idle/Kitty graphics/Sixel/shader workloads | `nix develop -c cargo fmt --check`; `nix develop -c cargo check -p rioterm`; `nix develop -c cargo build -p rioterm`; `python3 -m py_compile tools/yazelix_benchmark.py`; `python3 tools/yazelix_benchmark.py self-test`; real frame-run smoke under `nix develop`; `python3 tools/yazelix_conformance.py verify`; `git diff --check` |
| Comparable Rio/Ghostty frame runs | Records one local release sample per case for Rio WGPU scroll, Ghostty scroll, Rio shader idle, throttled Rio shader idle, and Ghostty shader idle | `python3 -m py_compile tools/yazelix_benchmark.py`; `python3 tools/yazelix_benchmark.py self-test`; `python3 tools/yazelix_conformance.py verify`; `git diff --check`; artifacts under `artifacts/benchmarks/frame_time/` |

## Final State

- All Beads for the Ghostty-parity experiment are closed
- `br list --status open` returns no open issues
- `docs/yazelix/ghostty_parity_contract.md` has no current Ghostty-parity
  blockers
- Full Kitty arbitrary-MIME OSC 5522 support remains documented frontier work,
  not a Ghostty blocker
- Full Kitty OSC 72 drag/drop runtime remains deferred until the window/event
  boundary can represent the spec safely

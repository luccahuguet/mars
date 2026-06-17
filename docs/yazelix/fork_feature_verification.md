# Mars Terminal Fork-Owned Feature Verification

Status: fork-owned additions are tracked by row with focused verification
evidence.

This file is only for behavior, tooling, policy, or documentation that the
Mars Terminal's current `yazelix-terminal` fork adds on top of the Rio base commit
`7e18dde1c90182a5170a7cca7779544967d7291c`.

If a row describes inherited Rio behavior, unchanged terminal behavior, a
baseline launch check, a benchmark run, a source audit, or a deferred feature,
it does not belong here. Put it in
`docs/yazelix/validated_not_added.md` instead.

## Classification Rule

Rows in this file are fork-owned additions. They may be one of:

- `runtime`: user-visible terminal behavior added by the fork
- `tooling`: conformance, benchmark, screenshot, packaging, or smoke-test tooling
- `policy`: documented security, source-use, or support boundary owned by the fork
- `docs`: durable documentation authored by the fork

Rows in this file must not be described as merely "verified". Verification
evidence belongs in the right column, but the middle column must name the
fork-owned addition. Each row also records the practical implication today and
the future possibility the addition opens, so the ledger stays useful for
prioritization instead of becoming only a completion checklist.

Related docs:

- `docs/yazelix/validated_not_added.md`
- `docs/yazelix/ghostty_parity_contract.md`
- `docs/yazelix/frontier_kitty_protocols.md`
- `docs/yazelix/conformance_harness.md`
- `docs/yazelix/dossiers/cursor_shader_parity.md`
- `docs/yazelix/performance_and_graphics_benchmark.md`
- `docs/yazelix/stack_validation.md`
- `docs/yazelix/release_closeout_2026_06.md`
- `docs/yazelix/upstream_sync_workflow.md`

## Upstream Comparison Snapshot

The fork base remains Rio commit
`7e18dde1c90182a5170a7cca7779544967d7291c` (`flake.lock: Update
(#1636)`, 2026-05-31). The current upstream comparison point is Rio
`fdf5a7b9602bbeb19bfe5d3dae6ebe39894b1e39`
(`v0.4.7-1-gfdf5a7b960`, 2026-06-10).

Upstream-compatible fixes absorbed from the v0.4.7 comparison pass:

| Upstream commit | Surface | Fork state | Verification evidence |
| --- | --- | --- | --- |
| `330ed70c9c` | Kitty graphics `X=`/`Y=` sub-cell placement offsets | Parser, placement occupancy, offset clamping, and renderer overlay positioning are aligned with upstream while preserving Yazelix image-overlay behavior | `cargo test -p rio-backend --features rio-window/x11 subcell_offset`; `cargo fmt --check`; `git diff --check` |
| `9f0b59733c` | Route-aware cursor trail reset between tabs/panels | `TrailCursor` tracks route ids and snaps first frame after route changes, while Yazelix redraw-jump and Helix one-row trail policy stays covered | `cargo test -p rioterm trail_cursor`; `cargo fmt --check`; `git diff --check` |
| `e96137a5a4` | Font fallback discovery restore | Sugarloaf UI text uses `FontLibrary::resolve_font_for_char` on non-macOS, matching the already-aligned grid emitter path | `cargo test -p sugarloaf --features rio-window/x11 font::symbol_map_tests::symbol_map_match_preserves_emoji_flag`; `cargo fmt --check`; `git diff --check` |
| `fdf5a7b960` | Teletypewriter winsize fixes | Unix PTY creation receives initial pixel width/height, winsize fields use `ws_xpixel`/`ws_ypixel`, and Yazelix child-env sanitization remains intact | `cargo test -p teletypewriter`; `cargo fmt --check`; `git diff --check` |

Resolved upstream comparison decisions:

- `yzt-5hl.5`: Rio tab drag/reorder and island refactor are not
  cherry-picked in this pass. They should be adapted later only after tab
  metadata moves out of index-keyed title/island maps. See
  `docs/yazelix/upstream_sync_workflow.md`.
- `yzt-5hl.6`: future Rio sync work defaults to selective cherry-pick/manual
  adaptation, not blind rebase or merge. The workflow, delta classes,
  high-conflict files, and validation ladder are recorded in
  `docs/yazelix/upstream_sync_workflow.md`.

## Common Gates

These checks were used repeatedly as the fork-wide regression floor:

```bash
nix develop -c cargo fmt --check
nix develop -c cargo check -p rioterm
nix develop -c cargo build -p rioterm
nix develop -c cargo build -p rioterm --features wgpu
nix run .#yazelix-protocol-conformance -- verify
git diff --check
```

The current focused package checks also include:

```bash
cargo fmt -- --check
cargo check -p rioterm --no-default-features --features wgpu,x11,wayland
cargo test -p rio-window
cargo test --features wgpu ghostty -- --nocapture
cargo test -p rio-backend --features rio-window/x11 subcell_offset
cargo test -p rioterm trail_cursor
cargo test -p sugarloaf --features rio-window/x11 font::symbol_map_tests::symbol_map_match_preserves_emoji_flag
cargo test -p teletypewriter
nix build .#mars-fast
tools/yazelix_event_mode_smoke.sh ./result_mars_fast_package
```

The X11 IME callback cast warning is fixed in `0880df924a`; the focused
`rioterm` check above is warning-free.

## Fork-Owned Foundation

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| docs | Lineage and guardrails | Records Rio base commit, reference clones, source-use rules, main Yazelix integration boundary, and hard-problem pivot rules | Keeps the experiment bounded and reviewable, with a clear upstream comparison point. | Makes rebasing, upstream-delta audits, and abandoned-approach writeups cheaper. | Documentation review and `git diff --check` |
| docs | Source absorption workflow | Defines how Ghostty, WezTerm, and Kitty evidence can be used without crossing license boundaries | Lets the fork use Ghostty and WezTerm as implementation references while keeping Kitty to specs and black-box behavior. | Provides a repeatable review path if future features need deeper source borrowing or license decisions. | Documentation review and `git diff --check` |
| tooling | Conformance harness | Adds checked-in byte fixtures, environment capture, shader probe validation, keyboard black-box capture helpers, and screenshot launch helpers | Gives protocol work a quick regression floor before heavier Rust, Nix, or visual checks. | Can become the shared Rio/Ghostty/Kitty comparison harness for parity decisions and CI gates. | `nix run .#yazelix-protocol-conformance -- verify`; `list`; `emit`; `record-env`; `launch-cpu-screenshot`; `cargo check --manifest-path tools/yazelix_protocol_conformance/Cargo.toml` |
| tooling | Parser robustness smoke | Adds deterministic parser-noise smoke coverage and records when to escalate to cargo-fuzz/libFuzzer | Catches obvious parser regressions from malformed or noisy escape streams. | Establishes the boundary for adding fuzzing/property tests when parser risk grows. | `cargo fmt`; focused `rio-backend` parser test; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `cargo check -p rioterm` |

## Cursor Shaders And Rendering

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | Ghostty-compatible shader runtime | Adds `[renderer] custom-shader`, WGPU `GhosttyShaderBrush`, Shadertoy `mainImage` wrapping, Ghostty cursor/color/focus/palette uniforms, Naga GLSL parse gate, and render hook before existing filters | Runs Ghostty-style cursor shaders in the Rio-derived renderer, making visual parity a real runtime capability. | Enables richer shader presets, shader compatibility testing against Ghostty, and possible user-supplied visual effects. | `cargo check -p rioterm --features wgpu`; `sugarloaf` Ghostty shader tests; `rio-backend` custom shader config test; `nix run .#yazelix-protocol-conformance -- verify`; dev build |
| tooling | WGPU screenshot path | Adds WGPU shader screenshot capture by passing the raw display handle into WGPU instance creation | Produces visual evidence for shader behavior instead of relying only on logs or successful compilation. | Can grow into automated screenshot/framebuffer parity checks for renderer changes. | `nix run .#yazelix-protocol-conformance -- launch-wgpu-shader-screenshot`; `cargo check --manifest-path tools/yazelix_protocol_conformance/Cargo.toml`; `cargo check -p rioterm --features wgpu`; `sugarloaf` Ghostty shader tests with Linux window features; `nix run .#yazelix-protocol-conformance -- verify`; screenshot `artifacts/shader_probe/screenshots/wgpu_shader_probe_gl.png` |
| runtime | Yazelix Ghostty shader presets | Adds packaged support for the generated Yazelix Ghostty cursor shader stack and moves the Shadertoy wrapper after user shader source for Naga compatibility | Makes the packaged Yazelix terminal visibly match the intended Ghostty cursor effect stack. | Gives Yazelix a stable preset surface for new terminal effects without per-user config surgery. | `cargo fmt`; `sugarloaf` Ghostty shader tests with `YAZELIX_GHOSTTY_SHADER_DIR`; `cargo check -p rioterm --features wgpu`; `cargo check --manifest-path tools/yazelix_protocol_conformance/Cargo.toml`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; manual screenshot inspection |
| runtime | Shader redraw throttling | Replaces immediate game-mode redraw requests with a vblank-interval scheduler tick | Keeps animated shaders responsive without pointlessly hammering the event loop. | Provides a performance control point for more animation-heavy terminal effects. | `cargo fmt --check`; `cargo check -p rioterm`; `cargo build -p rioterm --release`; throttled frame-run benchmark; `python3 tools/yazelix_benchmark.py self-test`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| runtime | Packaged Rio trail default and shader profile | Builds the packaged terminal with WGPU, installs Yazelix Ghostty-style cursor shader assets under `share/yazelix-terminal/shaders`, keeps `backend = "Webgpu"` and `trail-cursor = true` in the default profile, and moves the packaged `custom-shader` chain to `profiles/shaders` | Users launching the Nix package get Rio's native trail cursor by default without stacking a second cursor animation; shader parity remains opt-in. | Makes future package-level visual defaults and compatibility migrations explicit and testable. | `cargo fmt -- --check`; `cargo test --features wgpu ghostty -- --nocapture`; `cargo check -p rioterm --no-default-features --features wgpu,x11,wayland`; `nix build .#mars-fast`; `tools/yazelix_event_mode_smoke.sh ./result_mars_fast_package`; manual packaged-window typing confirmation |
| runtime | Event-mode cursor animation | Treats Ghostty cursor shader frame-state changes as an explicit redraw source so cursor shader/trail animation works without global `renderer.strategy = "game"` | Keeps cursor animation working in the normal event renderer strategy. | Reduces pressure to use game mode and leaves room for event-driven animation of other stateful effects. | `cargo test --features wgpu ghostty -- --nocapture`; `cargo check -p rioterm --no-default-features --features wgpu,x11,wayland`; `tools/yazelix_event_mode_smoke.sh ./result_mars_fast_package`; manual confirmation that shader appears while typing under event mode |

## Yazelix Host Mode And Stack Fixes

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | Yazelix terminal host mode | Adds `--yazelix`, requires `-e/--command`, defaults app id to `yazelix-terminal`, sets `TERM_PROGRAM=mars` and `MARS_TERMINAL_HOST=mars`, keeps Rio-compatible terminfo aliases, and disables Rio split/config-editor ownership in Yazelix mode | Lets Yazelix run in Mars without competing with Zellij for panes, tabs, sessions, or focus policy. | Provides the launch contract for making Mars the default Yazelix terminal host. | `rioterm` `yazelix_mode` tests; `cargo fmt --check`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| runtime | Desktop Wayland event wakeup | Calls `pre_present_notify()` only for frames that will actually present, keeps the packaged desktop config on Rio's default event renderer strategy, and preserves an explicit `YAZELIX_TERMINAL_RENDER_STRATEGY=game` diagnostic overlay | Improves packaged desktop snappiness on Wayland while keeping a diagnostic escape hatch. | Gives future desktop launcher work a safe place to tune render strategy per environment. | `sh -n misc/yazelix_terminal_desktop.sh`; `git diff --check`; `cargo fmt -- --check`; `cargo check -p rioterm --no-default-features --features wgpu,x11,wayland`; `tools/yazelix_event_mode_smoke.sh ./result_mars_fast_package`; manual COSMIC/Wayland desktop launcher typing, `yzx enter`, and snappiness test |
| runtime | Stack graphics fixes | Fixes atlas Sixel/iTerm rendering, Kitty `U=1` virtual placement dimensions, virtual source rectangles, and child terminal identity for Yazelix/Zellij/Yazi stack use | Makes direct Sixel/Kitty image paths and Yazi previews work through the Yazelix/Zellij stack. | Opens the door to treating image/PDF preview quality as a terminal-host requirement instead of a best-effort add-on. | Focused `rioterm` and `rio-backend` tests; WGPU build; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; direct Sixel screenshot; direct Kitty Unicode-placeholder screenshot; Yazi Kitty preview through Zellij/Yazelix; Helix-in-stack screenshot evidence |
| runtime | Emoji and icon glyph fallback | Preserves emoji metadata for symbol-map font hits, maps private-use icon glyphs to `Symbols Nerd Font Mono`, maps selected text-style status symbols to packaged `Noto Sans Symbols 2`, and maps common emoji/status ranges to the selected packaged emoji fallback | Prompt emoji and common status glyphs use a packaged color emoji face instead of rough text fallback, terminal status text symbols use a deterministic monochrome symbol face, and prompt glyphs outside emoji coverage keep normal font fallback instead of being forced to emoji tofu. `noto` remains the default; `twitter` and `serenityos` are free visual dogfooding presets. | Gives yzxterm a clean place to tune emoji/icon fallback quality without moving terminal-specific font logic into main Yazelix. | `git diff --check`; `nix-instantiate --parse pkgRio.nix`; package layout check; manual fresh-window glyph screenshot evidence |

## Fork-Owned Core Graphics Delta

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | Sixel | Bridges atlas Sixel graphics into the renderer image-overlay path | Terminals apps that emit Sixel can show images instead of silently parsing and dropping them. | Creates a shared image-overlay path for future visual comparisons across graphics protocols. | Focused stack tests; `nix run .#yazelix-protocol-conformance -- verify`; direct Sixel screenshot |
| runtime | iTerm2 OSC 1337 images | Bridges atlas iTerm image graphics into the renderer image-overlay path | Tools using iTerm-style inline images have a working renderer path. | Gives the stack another compatibility path for preview tools that do not emit Kitty graphics. | Focused stack tests; WGPU build; `nix run .#yazelix-protocol-conformance -- verify`; stack screenshot evidence |

## Shell And Prompt Protocols

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | OSC 133 semantic zones | Adds typed parser for `A/B/C/D/I/L/N/P`, Ghostty/Kitty options, production dispatch, and row-level prompt/input/output state | The terminal can distinguish prompt, input, and output regions instead of treating the scrollback as undifferentiated text. | Enables smarter command navigation, output selection, shell integration, and agent-readable terminal state. | `cargo fmt --check`; `rio-backend` `semantic_prompt` tests with platform features; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| runtime | OSC 133 prompt actions | Adds prompt navigation and command-output selection actions on top of semantic-zone state | Users can navigate prompt/output regions with terminal actions. | Makes room for command-aware copy, replay, diagnostics, and structured history features. | `cargo fmt --check`; `rio-backend` `semantic_` tests; `rioterm` `semantic_prompt_actions_parse_from_config_strings`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `nix develop -c cargo check -p rioterm` |

## Clipboard, Colors, Pointer, And Notifications

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | OSC 52 clipboard policy | Rejects unsupported clipboard designators, invalid base64, non-UTF-8 payloads, and oversized encoded/decoded payloads with visible warnings | Makes terminal clipboard escape handling fail closed instead of accepting malformed or oversized writes. | Provides a policy base for richer clipboard permissions and user-visible denial UX. | `osc52` backend test; `cargo fmt`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| runtime | OSC 99 notification parser | Parses Kitty OSC 99 metadata/title/body/base64 payloads, accumulates chunked title/body by id, and reuses the existing desktop notification event | Terminal applications can emit Kitty-style notifications with useful title/body metadata. | Enables richer app-to-terminal notification contracts, including action buttons and close tracking. | `kitty_notification` tests; `cargo fmt --check`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| runtime | OSC 99 lifecycle replies | Sanitizes notification ids, parses close-report metadata, tracks live ids for alive replies, answers support queries, and handles backend close requests | Applications can ask what notification support exists and receive lifecycle replies. | Makes it possible to build terminal apps that respond to notification activation/close events. | `cargo fmt --check`; `rio-backend` `kitty_notification` tests; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `nix develop -c cargo check -p rioterm` |
| runtime | OSC 99 OS-backed handles | Preserves report/buttons/close metadata, routes Linux D-Bus replace/close/action events back to the originating PTY, and documents macOS/Windows as explicitly untracked until delegate layers exist | On Linux, OS notification callbacks can reach the terminal app that requested them. | Gives macOS/Windows delegate work a precise target and keeps cross-platform gaps explicit. | `cargo fmt --check`; `cargo test -p rio-notifier`; `cargo test -p rio-backend ... kitty_notification`; `cargo check -p rioterm`; `cargo build -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; live Rio OSC 99 smoke |
| runtime | OSC 21 color control | Implements keyed set/query/reset for foreground, background, cursor, palette, unsupported query replies, and conformance fixture | Apps can dynamically inspect and adjust terminal colors with Kitty/Ghostty-compatible replies. | Supports theme-aware TUIs, better shader uniforms, and future visual conformance around palette changes. | Focused tests recorded in Beads; `nix run .#yazelix-protocol-conformance -- verify` fixture `osc21_kitty_color_control` |
| runtime | OSC 21 special colors | Stores and queries `cursor_text`, selection colors, `visual_bell`, `transparent_background`, and Ghostty-compatible `second_transparent_background`; wires cursor text and selection colors into rendering and shader uniforms | Selection, cursor, visual-bell, and transparency colors affect actual rendering and shader inputs. | Enables full Ghostty-style visual state parity and richer app-controlled terminal theming. | `cargo fmt --check`; `rio-backend` `kitty_color` tests; `nix develop -c cargo check -p rioterm`; `nix develop -c cargo check -p rioterm --features wgpu`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| runtime | OSC 21 visual effects | Renders `visual_bell` as a fading BEL overlay and transparent-background color as transparent-window composition when no background image is configured | BEL and transparent-background color changes become visible behavior, not parser-only state. | Leaves a path for visual effects to be controlled by terminal protocols instead of static config only. | `cargo fmt --check`; `rioterm` `visual_bell` test; `rioterm` `transparent_background` test; `rio-backend` `second_transparent` test; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `cargo check -p rioterm`; `cargo build -p rioterm` |
| runtime | OSC 22 pointer shapes | Implements set/reset, push/pop, current/support queries, alternate-screen stacks, reset clearing, frontend cursor selection, and conformance fixture | Mouse-heavy terminal UIs can request more appropriate pointer shapes. | Gives Yazelix room for richer pointer-aware workflows in panes, pickers, previews, and editors. | Focused parser/frontend tests recorded in Beads; `nix run .#yazelix-protocol-conformance -- verify` fixture `osc22_pointer_shapes` |

## Kitty Keyboard

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| tooling | Spec audit and fixtures | Adds all-flags and stack fixture streams and records concrete gap beads from the Kitty keyboard audit | Makes keyboard-protocol gaps concrete instead of anecdotal. | Can guide black-box parity work against Ghostty, Kitty, and WezTerm without copying implementation code. | `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| runtime | Mode stack semantics | Implements compact stack behavior, oldest-entry eviction on full push, pop-to-empty reset, and a shared mode-sync helper | Apps can push/pop enhanced keyboard modes without corrupting terminal state. | Enables more editor/TUI workflows to rely on Kitty keyboard semantics across nested applications. | `cargo fmt --check`; `nix develop -c cargo test -p rio-backend --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' keyboard_mode -- --nocapture`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `nix develop -c cargo check -p rioterm` |
| runtime | Modifier bits | Implements Hyper and Meta modifier bits and keeps CapsLock/NumLock explicit instead of fabricated before platform lock-state support | Reduces ambiguous key reports for power-user bindings and enhanced keyboard mode. | Lets Yazelix support richer cross-platform keybinding schemes as platform lock-state coverage improves. | `cargo fmt --check`; `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_ -- --nocapture`; `nix develop -c cargo check -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| runtime | Base-layout alternate keys | Reports PC-101 physical letters, digits, and punctuation through Kitty base-layout alternate-key fields | Applications can distinguish layout-produced text from physical/base keys where the protocol exposes it. | Improves prospects for keyboard-layout-stable shortcuts in Helix/Zellij/Yazi workflows. | `cargo fmt --check`; `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_ -- --nocapture`; `nix develop -c cargo check -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| runtime | Functional/keypad mappings | Fills exposable table holes including numpad comma, numpad clear/begin, and ISO level 3 shift through AltGraph | More special keys reach apps as specific events instead of falling through as ambiguous input. | Supports deeper parity for international layouts, keypad-heavy workflows, and advanced editor bindings. | `cargo fmt --check`; `nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_ -- --nocapture`; `nix develop -c cargo check -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| tooling | Black-box comparison harness | Adds capture/verify commands and a case manifest for Rio/Ghostty/Kitty keyboard comparison | Lets maintainers capture actual terminal key bytes and compare them to checked-in expectations. | Can become a cross-terminal compatibility matrix for deciding when keyboard parity is good enough. | `nix run .#yazelix-protocol-conformance -- verify`; `nix run .#yazelix-protocol-conformance -- keyboard-list`; synthetic `keyboard-verify-capture`; `cargo check --manifest-path tools/yazelix_protocol_conformance/Cargo.toml`; `git diff --check` |
| runtime | Lock-state modifiers | Threads CapsLock/NumLock through Linux XKB, Windows, Web, and macOS CapsLock; reports lock bits only in enhanced Kitty keyboard mode | Reports lock-state modifiers where the platform reliably exposes them without inventing fake state. | Leaves a clean extension point for macOS NumLock or backend-specific lock-state improvements. | `cargo fmt --check`; `rioterm` Kitty keyboard tests; `rio-backend` `keyboard_mode` tests; `cargo check -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |

Known runtime limits:

- macOS NumLock and backends without reliable NumLock remain unavailable
- private-use keys that `rio-window` does not expose distinctly remain
  unavailable

## Kitty OSC 66 Text Sizing

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | Protocol parser and width support | Parses scale, declared width, fractional scale, alignment, and text payload; dispatches to `input_sized_text`; advances cursor by client-declared width/scale | Apps can emit sized text without breaking cursor advancement or line layout. | Enables richer terminal UI typography and compatibility with Kitty/Ghostty text-sizing users. | `text_sizing` and `osc66` `rio-backend` tests with platform features; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `cargo check -p rioterm` with platform features |
| runtime | Visual scaled/fractional rendering | Stores sizing metadata in the Extras side table and renders scaled/fractional runs without widening `Square` | Sized text appears visually scaled while preserving the underlying grid cell model. | Makes it possible to add visual parity tests and higher-level UI patterns using sized runs. | `cargo fmt --check`; `git diff --check`; `nix run .#yazelix-protocol-conformance -- verify`; focused `rio-backend` `text_sizing` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; local CPU-renderer OSC 66 screenshot probe |
| runtime | Multicell editing semantics | Handles top-row overwrite erasure, lower-row skip, width-fit wrapping, ECH/EL/ED erasure, ICH/DCH edit erasure, and IL/DL line edit erasure | Editing operations no longer leave stale fragments when they intersect sized text. | Provides the correctness base for real applications to use sized text repeatedly, not only in demos. | `cargo fmt --check`; `git diff --check`; `nix run .#yazelix-protocol-conformance -- verify`; `rio-backend` `text_sizing` tests; `cargo check -p rioterm`; `cargo build -p rioterm` |
| runtime | Cursor visual extents | Expands block/hollow-block/bar/underline cursor sprites and Ghostty shader cursor rectangles over sized-text visual extents | Cursor rendering matches the visible sized text span instead of only the anchor cell. | Enables shader and cursor-effect parity for terminals that mix normal and sized text. | `cargo fmt --check`; `git diff --check`; `nix run .#yazelix-protocol-conformance -- verify`; `rioterm` `text_sizing_cursor` tests; `cargo check -p rioterm`; `cargo build -p rioterm` |

## Kitty Multiple Cursors

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | Parser/state/rendering | Implements CSI `> ... SP q`, support/state/color queries, point/rectangle/current-coordinate mutation, ED/reset/alternate-screen clearing, conformance fixture, backend tests, and frontend sprite rendering | Advanced clients can render extra cursors with state/query behavior instead of being ignored. | Opens a path for editor-like multicursor UI in terminal apps and deeper Kitty frontier parity. | Backend tests recorded in Beads; `nix run .#yazelix-protocol-conformance -- verify` fixture `kitty_multiple_cursors_query_set_clear` |
| runtime | Shader and reverse-video rendering | Carries up to 256 extra block cursor cells with independent background/text-swap colors through CPU/WGPU/Metal/Vulkan shader paths and adds Yazelix shader ABI extension uniforms | Extra cursors can participate in reverse-video and shader rendering rather than becoming parser-only state. | Gives shader presets access to multicursor state for future editor-grade visual effects. | `cargo fmt --check`; `git diff --check`; `nix run .#yazelix-protocol-conformance -- verify`; `sugarloaf` `ghostty_` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; local CPU-rendered Kitty multiple-cursor screenshot probe |

Known runtime limit: the shader/uniform path exports up to 256 visible extra
cursor cells. State can track larger requests, but exact shader/reverse-video
parity is bounded.

## Other Kitty Protocols

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | Kitty unscrolling | Implements primary-screen full-screen `CSI <n> + T` line restore from scrollback, with non-full-screen and alternate-screen regions falling back to ordinary blank scroll-down behavior | Apps using Kitty unscrolling can reveal scrollback content correctly in the primary screen. | Improves compatibility with modern terminal navigation patterns that blend viewport and scrollback state. | `nix run .#yazelix-protocol-conformance -- verify` fixture `kitty_unscroll_three_lines`; focused scrollback regression tests recorded in Beads |
| runtime | Kitty DECCARA | Implements all-SGR rectangular styling, common DECSACE wrapper handling, and RGB/indexed SGR tails | Apps can apply rectangular style changes without moving the cursor or redrawing whole regions manually. | Supports more efficient and expressive TUI rendering primitives. | Parser/handler/grid tests; `nix run .#yazelix-protocol-conformance -- verify` fixture `kitty_deccara_all_sgr` |

## Kitty OSC 5522 Rich Clipboard

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| runtime | Text-compatible OSC 5522 slice | Implements metadata/payload parser, `type=read`, `type=write`, `type=wdata`, `type=walias`, focus-policy frontend replies, MIME-list replies, transaction state, chunk limits, and text/plain storage | Modern clipboard transactions work for text while keeping unsupported MIME out of scope. | Provides a bridge toward richer MIME-aware clipboard support if the platform clipboard layer grows. | Focused OSC 5522 tests recorded in Beads; `nix run .#yazelix-protocol-conformance -- verify` fixture `osc5522_rich_clipboard_text_plain` |
| runtime | UTF-8 text MIME aliases | Advertises and accepts `text/plain` and `text/plain;charset=utf-8`; rejects non-text aliases instead of pretending arbitrary-MIME support | Apps get predictable text clipboard behavior and clear rejection for unsupported MIME aliases. | Keeps the compatibility surface honest while leaving a specific path for future arbitrary-MIME support. | `cargo fmt --check`; focused `osc5522` tests; `cargo check -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |

## Kitty File Transfer

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| policy | Security policy | Documents the OSC 5113 trust model, approval boundaries, safe roots, staged writes, local-read limits, and rejected advanced features | Keeps file transfer from becoming an implicit filesystem backdoor. | Gives future receive/send expansions a security checklist before implementation. | `git diff --check` |
| runtime | Parser and deny-by-default skeleton | Parses file-transfer commands, denies send/receive starts by default, rejects out-of-order commands, and performs no filesystem access before approval | File-transfer requests fail safely before user approval and before touching disk. | Allows incremental support for more OSC 5113 actions without weakening the trust boundary. | `cargo fmt --check`; `rio-backend` file-transfer tests; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check`; `cargo check -p rioterm`; `cargo build -p rioterm` |
| runtime | Approved remote-to-local writes | Routes approval through frontend notification, stages incoming files under an explicit transfer directory, normalizes paths, supports regular file/directory writes, and handles size/cancel/finish cleanup | A remote process can send approved regular files/directories without writing arbitrary paths directly. | Could become a convenient Yazelix workflow for moving artifacts out of remote shells or containers. | `cargo fmt --check`; `rio-backend` `osc5113` and `kitty_file_transfer` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| runtime | File-transfer state extraction | Moves OSC 5113 send/write state out of `crosswords/mod.rs` into `crosswords/kitty_file_transfer.rs` | Reduces pressure on the already large terminal grid module and localizes transfer policy/state. | Makes future file-transfer features easier to review and test independently. | `cargo fmt --check`; `rio-backend` `osc5113` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| runtime | Approved local-to-remote reads | Collects paths before approval, shows preview body, bounds path count/traversal depth, rejects symlink parents, lists regular file/directory metadata, streams one regular file at a time, and handles cancel/finish | A terminal app can request local files only after explicit approval and bounded path validation. | Could support safer local-to-remote project handoff flows from inside Yazelix. | `cargo fmt --check`; `rio-backend` `osc5113` and `kitty_file_transfer` tests; `cargo check -p rioterm`; `cargo build -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |
| runtime | Zlib compression | Supports zlib compression for regular-file send and receive data | Large regular-file transfers can use the common compressed path instead of only raw bytes. | Creates room for more efficient artifact movement once broader transfer UX exists. | Focused file-transfer tests; `nix run .#yazelix-protocol-conformance -- verify` fixture `kitty_file_transfer_zlib_file` |
| runtime | Explicit advanced-feature rejections | Proves `pw`/shared-secret metadata does not bypass approval, symlink/hardlink file types are rejected without staged files, and rsync/delta transmission rejects before sessions/prompts | Unsupported dangerous features fail closed and are covered by tests. | Defines the exact acceptance bar before adding destination chooser, metadata, links, secrets, or rsync support. | `cargo fmt --check`; focused `osc5113` tests; `kitty_file_transfer` parser tests; `cargo check -p rioterm`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |

Current policy rejections:

- destination chooser UX
- symlink and hardlink creation/traversal
- metadata preservation beyond bytes/path/kind/size
- generic shared-secret or password bypass
- rsync/delta transmission

## Benchmark Tooling

| Kind | Surface | Fork-owned addition | Practical implication today | Future possibility | Verification evidence |
| --- | --- | --- | --- | --- | --- |
| tooling | Frame-time harness | Adds env-gated Rio frame JSONL logging, stdlib frame benchmark runner/summarizer, `/proc` CPU/RSS sampling, and built-in scroll/idle/Kitty graphics/Sixel/shader workloads | Makes performance claims about scrolling, image workloads, and shaders measurable from local artifacts. | Can become a repeatable regression gate for Ghostty comparisons and renderer changes. | `nix develop -c cargo fmt --check`; `nix develop -c cargo check -p rioterm`; `nix develop -c cargo build -p rioterm`; `python3 -m py_compile tools/yazelix_benchmark.py`; `python3 tools/yazelix_benchmark.py self-test`; real frame-run smoke under `nix develop`; `nix run .#yazelix-protocol-conformance -- verify`; `git diff --check` |

## Document Boundary

- This file is the fork-owned addition ledger
- `docs/yazelix/validated_not_added.md` is the validation-only ledger for
  inherited behavior, baseline probes, benchmark results, source audits,
  parser-only evidence, and deferred runtime support
- Deferred/frontier protocol detail belongs in
  `docs/yazelix/validated_not_added.md` or
  `docs/yazelix/frontier_kitty_protocols.md`, not in this file

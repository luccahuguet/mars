# Mars Change Scorecard

Every Mars-owned code or runtime-behavior commit gets a row here. Pure documentation-only commits do not need scorecard rows. The goal is to make the fork delta easy to audit before pulling upstream Rio.

## Rules

- Add or update a row before closing the Bead for a code or runtime-behavior commit
- Keep the reason concrete: what broke, what user workflow it enables, or what gate it supports
- Record touched upstream Rio-owned files explicitly
- Record verification, even when verification is only static review
- For Rio-owned source edits, name the Ghostty and WezTerm equivalent paths or docs, the Rio GitHub issue search, and the newer upstream Rio commit range checked before editing
- Mark upstream merge cost as `low`, `medium`, or `high`
- Confirm the change is the smallest, simplest, surgical, non-invasive path; if not, explain the exception in that cell

## Scorecard

| Commit | Bead | Why It Was Needed | Main Change | Rio-Owned Files | Verification | Merge Cost | Surgical? |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `f4894de6ee` | `yzt-clean-rio-rebuild-0kz` | Restart Mars from a clean Rio baseline after the old fork became unusable. | Added the initial clean Mars wrapper identity on top of upstream Rio. | None expected for runtime behavior. | `git log` shows one Mars commit on top of `rio-upstream/main`; dogfooding gate still required. | low | yes |
| `a1165abd23` | `yzt-clean-rio-rebuild-0kz.11` | Mars and Rio need separate config roots without Home Manager pretending to be Rio. | Make Mars default to `~/.config/mars`, support `MARS_CONFIG_HOME`, and keep Yazelix startup in TOML config. | `rio-backend/src/config/mod.rs` | `git diff --check`; `cargo fmt --check`; `python3 -m py_compile tools/mars_private_yazelix.py`. | low | yes |
| pending | `yzt-emel` | Raw Mars launches can inherit terminal-session and Vulkan environment, and TOML-owned `yzx enter` exits cleanly without opening a durable dogfood window. | Keep Mars config terminal-only, route Mars through a small Vulkan-safe wrapper, and add a separate `mars-yazelix` launch-command wrapper. | None. | `git diff --check`; `python3 -m py_compile tools/mars_private_yazelix.py`; Home Manager switch; `mars --version`; `timeout 10 mars -e true`; `timeout 10 mars-yazelix -e true`; manual desktop launch. | low | yes |
| pending | `yzt-clean-rio-rebuild-0kz.12` | Desktop launchers need stable Mars identity and visible icons during dogfooding. | Install Mars icons at standard hicolor sizes and point the desktop file at Mars metadata. | None. | Home Manager switch and visual launcher check. | low | yes |
| pending | `yzt-clean-rio-rebuild-0kz.5` | Mars work needs repeatable resource measurements before feature changes. | Add reproducible perf gate orchestration with saved logs and delegated `pidstat`/`perf` sampling. | None. | `python -m py_compile tools/mars_perf_gate.py`; suite run still required after runtime is stable. | low | yes |
| `f2d1ff45a8` | `yzt-t6w7.3` | Yazi KGP virtual previews omit `c=`/`r=`, which made Mars/Rio render the image as a single cell. | Derive omitted virtual-placement columns/rows from image size and cell metrics. | `rio-backend/src/ansi/kitty_virtual.rs` | Reference gate: Ghostty `src/terminal/kitty/graphics_unicode.zig` and `graphics_storage.zig`; WezTerm `term/src/terminalstate/kitty.rs`, `image.rs`, and `wezterm-gui/src/termwindow/render/mod.rs` with no direct U+10EEEE path found. `git diff --check`; `rustfmt --check rio-backend/src/ansi/kitty_virtual.rs`; Cargo/Nix not run under Mars compile-heavy gate. | low | yes |
| `d7cf88366c` | `yzt-t6w7.2` | Yazelix runs inside Zellij mouse mode, where Rio's default Alt-click hints lose to app mouse reporting and are awkward for daily links. | Make non-macOS link hints use Ctrl-click, recompute hint hover on modifier changes, fully consume the terminal-owned press, and open the link on release. | `rio-backend/src/config/hints.rs`, `frontends/rioterm/src/application.rs`, `frontends/rioterm/src/mouse/mod.rs`, `frontends/rioterm/src/screen/mod.rs` | Reference gate: Ghostty `src/Surface.zig` ctrl-or-super hyperlink handling and `src/input.zig`; WezTerm `docs/config/mouse.md` Ctrl-click example and `wezterm-gui/src/termwindow/mod.rs`. Rio issue query found open #1278, #1457, #1298, #1610, #1619; `git log HEAD..rio-upstream/main` showed no newer link/hint commits. Verified `cargo fmt --check`; `git diff --check`; `cargo check -p rioterm`; `cargo test -p rio-backend --features rio-window/x11,rio-window/wayland test_hints_default`. Runtime/manual click smoke pending. | low | yes |

## Commit Template

| Commit | Bead | Why It Was Needed | Main Change | Rio-Owned Files | Verification | Merge Cost | Surgical? |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `shortsha` | `bead-id` | One sentence. | One sentence. | `path` or `None`. | Include Ghostty/WezTerm refs, Rio issue query, upstream commit range, and commands/artifacts. | low/medium/high | yes or exception |

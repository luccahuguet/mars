# Mars Change Scorecard

Every Mars-owned code or runtime-behavior commit gets a row here. Pure documentation-only commits do not need scorecard rows. The goal is to make the fork delta easy to audit before pulling upstream Rio.

## Rules

- Add or update a row before closing the Bead for a code or runtime-behavior commit
- Keep the reason concrete: what broke, what user workflow it enables, or what gate it supports
- Record touched upstream Rio-owned files explicitly
- Record verification, even when verification is only static review
- Mark upstream merge cost as `low`, `medium`, or `high`

## Scorecard

| Commit | Bead | Why It Was Needed | Main Change | Rio-Owned Files | Verification | Merge Cost |
| --- | --- | --- | --- | --- | --- | --- |
| `f4894de6ee` | `yzt-clean-rio-rebuild-0kz` | Restart Mars from a clean Rio baseline after the old fork became unusable. | Added the initial clean Mars wrapper identity on top of upstream Rio. | None expected for runtime behavior. | `git log` shows one Mars commit on top of `rio-upstream/main`; dogfooding gate still required. | low |
| this commit | `yzt-clean-rio-rebuild-0kz.11` | Mars and Rio need separate config roots without Home Manager pretending to be Rio. | Make Mars default to `~/.config/mars`, support `MARS_CONFIG_HOME`, and keep Yazelix startup in TOML config. | `rio-backend/src/config/mod.rs` | `git diff --check`; `cargo fmt --check`; `python3 -m py_compile tools/mars_private_yazelix.py`. | low |
| pending | `yzt-clean-rio-rebuild-0kz.12` | Desktop launchers need stable Mars identity and visible icons during dogfooding. | Install Mars icons at standard hicolor sizes and point the desktop file at Mars metadata. | None. | Home Manager switch and visual launcher check. | low |
| pending | `yzt-clean-rio-rebuild-0kz.5` | Mars work needs repeatable resource measurements before feature changes. | Add reproducible perf gate orchestration with saved logs and delegated `pidstat`/`perf` sampling. | None. | `python -m py_compile tools/mars_perf_gate.py`; suite run still required after runtime is stable. | low |

## Commit Template

| Commit | Bead | Why It Was Needed | Main Change | Rio-Owned Files | Verification | Merge Cost |
| --- | --- | --- | --- | --- | --- | --- |
| `shortsha` | `bead-id` | One sentence. | One sentence. | `path` or `None`. | Commands or artifact path. | low/medium/high |

# Yazelix Terminal Release Closeout

Date: 2026-06-07

Status: experimental release gate closed for dogfooding.

Decision: keep `yazelix-terminal` / `yzxterm` as an opt-in experimental
first-party terminal path. It is credible enough to package, document,
dogfood, benchmark, and compare seriously against Ghostty and WezTerm. It is
not the default Yazelix terminal and is not stable-promotion ready.

Ghostty remains the mature default. WezTerm remains the stable alternate.
`yzxterm` is the protocol-forward Yazelix-owned path.

Compared with vanilla Rio, `yzxterm` is already ahead on the surfaces Yazelix
cares about most: Ghostty-compatible shader support, Yazelix cursor shader
assets, Rio trail/profile packaging, OSC 21/22/66/99/133, OSC 5522 text
clipboard, Kitty keyboard work, Kitty multiple cursors, safe Kitty file
transfer, DECCARA, unscrolling, stack image-preview fixes, BELL/visual
notification behavior, and Yazelix host-mode/package metadata. The remaining
risk is not whether the fork has value over Rio. The risk is whether it has
enough mature desktop soak to replace Ghostty as the default.

## Evidence Map

| Surface | Evidence |
| --- | --- |
| Rio fork lineage and source-use guardrails | `docs/yazelix/lineage_and_guardrails.md`, `docs/yazelix/source_absorption_workflow.md` |
| Ghostty parity contract | `docs/yazelix/ghostty_parity_contract.md` |
| Fork-owned feature ledger | `docs/yazelix/fork_feature_verification.md` |
| Inherited or deferred behavior ledger | `docs/yazelix/validated_not_added.md` |
| Conformance harness | `docs/yazelix/conformance_harness.md`, `python3 tools/yazelix_conformance.py verify` |
| Parser robustness strategy | `docs/yazelix/parser_robustness_strategy.md` |
| Cursor shader and animation evidence | `docs/yazelix/dossiers/cursor_shader_parity.md`, `docs/yazelix/cursor_animation_architecture.md` |
| Yazelix host mode | `docs/yazelix/yazelix_mode.md` |
| Package and profile metadata | `docs/yazelix/package_metadata.md` |
| Stack validation | `docs/yazelix/stack_validation.md` |
| Performance and graphics benchmark | `docs/yazelix/performance_and_graphics_benchmark.md` |
| Main Yazelix terminal comparison | `docs/terminal_emulators.md` in the main Yazelix repo |

## Stack Matrix

This matrix is for the experimental `yzxterm` decision. It does not claim a
fresh stable-promotion pass across every terminal. Ghostty and WezTerm are used
as comparison baselines and owner-classification references.

| Flow | `yzxterm` state | Ghostty baseline | WezTerm baseline | Owner or follow-up |
| --- | --- | --- | --- | --- |
| Yazelix host launch | Pass. `--yazelix` launches one child command and disables terminal-native workspace ownership | Pass. Default packaged runtime | Pass. Packaged alternate | None |
| Zellij pane and tab ownership | Pass. Zellij owns panes, tabs, sessions, layouts, and focus policy | Pass | Pass | None |
| Direct Kitty graphics placeholders | Pass. WGPU/GL screenshot evidence exists | Pass | Pass | None |
| Yazi image preview through Yazelix/Zellij | Pass. Kitty graphics preview screenshot exists through the stack | Pass. Default runtime uses the Yazelix Zellij/Yazi KGP path | Partial until refreshed in the main compatibility matrix | Main compatibility follow-up |
| Framed Zellij Kitty placement | Pass for the validated Yazi preview path; placement source rectangles and virtual dimensions were fixed | Pass through current default runtime path | Partial until refreshed | Main compatibility follow-up |
| Direct Sixel | Pass. Direct WGPU/GL screenshot evidence exists | Not a Ghostty parity requirement | Pass in WezTerm | None |
| iTerm2 inline images | Pass through the fork-owned atlas-to-overlay path | Not a Ghostty parity requirement | Pass in WezTerm | None |
| Helix opens in the Yazelix stack | Pass. Helix-in-stack screenshot evidence exists | Pass | Pass | None |
| Cursor animation in Helix | Rio native trail works. Ghostty-compatible shader behavior remains profile-specific and Helix cursor movement has editor limitations | Ghostty has the same Helix cursor-effect limitation reported by maintainer testing | WezTerm has no Ghostty shader baseline | `yazelix-1et` for Helix cursor effects |
| Kitty keyboard / enhanced input | Pass at the terminal protocol layer, with fixtures and black-box tooling | Pass | Partial because WezTerm requires explicit Kitty keyboard enablement | `yazelix-sujyr` tracks cross-terminal `Ctrl-Alt-hjkl` failure |
| Bracketed paste, focus, SGR mouse, synchronized output | Covered by conformance fixtures and existing terminal behavior | Pass | Pass | None |
| High-rate output and editor-like cursor movement | Benchmark harness covers scroll, shader idle, and Helix-style viewport proxies | Stable mature baseline | Stable mature baseline | Stable-promotion soak follow-up |

## UX Matrix

This matrix closes the experimental UX gate. The stable-promotion gate remains
separate and should collect fresh live evidence before `yzxterm` becomes the
default or is advertised as mature.

| UX area | `yzxterm` state | Ghostty / WezTerm comparison | Follow-up |
| --- | --- | --- | --- |
| Font quality and shaping | Inherits Rio text rendering; no blocking dogfooding regression recorded for the experimental release | Ghostty and WezTerm remain the mature daily-driver references | Stable-promotion UX soak |
| Grapheme clusters, emoji, fallback fonts | Covered by inherited Rio behavior and prior rendering fixes; not exhaustively promoted | Compare against Ghostty and WezTerm in future live audit | Stable-promotion UX soak |
| Selection, copy, and search | Inherited Rio UX; no known blocker for opt-in dogfooding | Ghostty and WezTerm remain mature references | Stable-promotion UX soak |
| IME | X11 IME callback warning was fixed in the fork. Full live IME behavior is not promoted | Ghostty and WezTerm remain references for live IME quality | Stable-promotion UX soak |
| High-DPI and resize | Stack screenshots and dogfooding did not leave a current release blocker; cross-platform claims are not made | Ghostty and WezTerm remain mature references | Stable-promotion UX soak |
| Native window behavior | Packaged desktop wrapper and yzxterm profile metadata exist; app identity is good enough for experimental use | Ghostty and WezTerm remain more mature desktop apps | Icon and native polish follow-up |
| Transparency and background behavior | Main Yazelix materializes transparency into `yzxterm` generated config; OSC 21 visual colors are implemented in the fork | Ghostty remains the default transparency target | None for experimental release |
| Cursor effects | Default profile uses Rio native trail. Shader profile keeps Ghostty-compatible shader support explicit and opt-in | Ghostty remains the strongest mature shader target | Helix cursor effects and future shader/trail unification |

## Known Gaps

- `yzxterm` is not the default and should not be promoted without a fresh
  maintainer stable-promotion pass.
- The default profile uses Rio native trail cursor behavior. The
  Ghostty-compatible shader stack is an explicit shader profile, not the
  default dogfooding profile.
- Helix cursor effects still have editor-side limitations. Rio native trail
  animation works during Helix movement, but Ghostty shader effects do not get a
  stronger claim than Ghostty itself currently gets in Helix.
- WebGPU trail geometry differs from the default Rio edge-style trail geometry.
  This is documented and accepted for the packaged Yazelix Rio / `yzxterm`
  WebGPU path.
- Same-flow Ghostty and WezTerm screenshots were not freshly recollected for
  this closeout. The main compatibility matrix should refresh them before
  stable promotion. Main Bead `yazelix-yzxterm-stable-promotion-ux-kfjkz`
  owns that future promotion audit.
- Full live IME, selection/search, high-DPI, and native desktop UX still need a
  stable-promotion audit under `yazelix-yzxterm-stable-promotion-ux-kfjkz`.
- Polished application icons are not a Ghostty parity blocker for the
  experimental runtime, but the current Rio-derived icon path is bad enough to
  remain real product work. The package installs Rio's current `misc/logo.svg`
  for `yazelix-terminal`, and upstream Rio has open icon pressure in
  `raphamorim/rio#896` and `raphamorim/rio#1401`, plus older branding
  discussion in `raphamorim/rio#299`.
- Linux-local graphics and benchmark evidence does not prove native Windows or
  macOS parity.

## Closeout Rule

Close the Ghostty-parity epic when the decision remains experimental:

- protocol and shader implementation blockers are closed
- stack evidence exists for Yazelix/Zellij/Yazi/Helix
- performance and graphics evidence is documented
- known gaps are explicit and assigned to follow-up work
- main Yazelix docs present `yzxterm` as experimental, not default

Do not use this closeout to promote `yzxterm` to the default terminal. A future
stable-promotion decision should collect fresh Ghostty and WezTerm same-flow
evidence, live IME/selection/search checks, high-DPI and resize checks, native
window polish evidence, and maintainer approval.

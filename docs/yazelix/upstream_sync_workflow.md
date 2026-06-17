# Upstream Rio Sync Workflow

Status: repeatable maintenance workflow for Mars Terminal's current
`mars` Rio fork.

This document covers Rio-to-Mars sync work. It is narrower than
`source_absorption_workflow.md`: Rio is the fork base, so direct code ports are
allowed, but the fork has enough local behavior that blind merges are no longer
the default safe move.

## Default Policy

Use selective cherry-pick or manual adaptation by default. Do not rebase the
fork or merge `rio-upstream/main` directly unless a separate bead proves the
range is mostly mechanical and the user approves the higher-risk integration
step.

Reasons:

- the fork owns packaged mars profiles, shader behavior, protocol probes,
  Yazelix host mode, and Beads/doc artifacts that upstream Rio does not carry
- high-conflict files have meaningful local behavior, not just formatting drift
- upstream still delivers small correctness fixes that are worth absorbing
  quickly when isolated

The practical rule is: port correctness fixes quickly, split broad UI or
architecture changes into separate evaluation beads, and record intentional
divergence.

## Delta Classes

Classify every upstream commit before editing code:

| Class | Definition | Default action |
| --- | --- | --- |
| upstream-compatible correctness | Bug fix, platform fix, protocol fix, or dependency-independent cleanup that can fit the fork without changing Yazelix behavior | Port in a focused commit with tests |
| fork-owned feature | Behavior added by Mars Terminal on top of Rio, such as shader ABI, protocol harnesses, mars packaging, host mode, custom cursor policy, or Beads/docs | Keep local; compare for conflicts but do not overwrite |
| packaging/runtime integration | Nix, packaged config, runtime profile, Home Manager handoff, or main Yazelix lock consumption | Own in Yazelix/main release transaction; do not expect Rio upstream to match |
| experimental surface | Shader experiments, UI experiments, benchmarks, screenshots, or maintainer-only diagnostics | Keep isolated; delete or graduate before broad sync |
| broad upstream UI/architecture | Tab drag, island layout, title ownership, platform chrome, or renderer/module reorganizations | Evaluate separately; adapt only after local seams are ready |

## Sync Loop

1. Fetch Rio and record refs.

   ```bash
   git fetch rio-upstream
   git rev-parse rio-upstream/main
   git describe --always --tags rio-upstream/main
   ```

2. List the candidate range from the last recorded comparison point.

   ```bash
   git log --oneline --reverse <last-comparison>..rio-upstream/main
   git show --stat <commit>
   ```

3. Compare by surface, not just by commit identity.

   `git cherry -v HEAD rio-upstream/main` is useful, but manual ports will not
   disappear from that list. Cross-check with file diffs and existing tests.

   ```bash
   git diff --stat HEAD..rio-upstream/main -- \
     frontends/rioterm/src/application.rs \
     frontends/rioterm/src/context/mod.rs \
     frontends/rioterm/src/renderer/island.rs \
     frontends/rioterm/src/renderer/mod.rs \
     frontends/rioterm/src/renderer/trail_cursor.rs \
     frontends/rioterm/src/screen/mod.rs \
     rio-backend/src/ansi/kitty_graphics_protocol.rs \
     rio-backend/src/crosswords/mod.rs \
     rio-backend/src/graphics/kitty/mod.rs \
     sugarloaf/src/text.rs \
     teletypewriter/src
   ```

4. Create or update Beads before implementation.

   Use one parent comparison bead for the upstream range. Split unrelated
   surfaces into child beads. Small correctness ports can share a commit only
   when they touch independent files and have focused tests. Broad UI changes
   must be separate.

5. Port small correctness fixes first.

   Good examples from the Rio `v0.4.7` pass:

   - Kitty graphics `X=`/`Y=` placement offsets
   - route-aware cursor trail reset
   - font fallback discovery restore
   - teletypewriter winsize pixel dimensions

6. Keep broad UI work behind an adaptation decision.

   Do not cherry-pick large island, screen, renderer, or application changes
   until the local fork ownership model is clear. Record whether the feature is
   ported, skipped, or deferred for an enabling refactor.

7. Validate the exact touched surface.

   Prefer focused Rust tests and `git diff --check`. Use heavyweight Nix or
   runtime validation only when the touched surface requires it, and respect the
   mars rebuild-speed gate.

8. Update the comparison ledger.

   Update `docs/yazelix/fork_feature_verification.md` with the new upstream
   comparison point, absorbed fixes, explicit divergences, and remaining
   follow-ups. Close Beads with the validation evidence.

## High-Conflict Files

These files should be treated as manual adaptation zones:

| Surface | Conflict reason | Refactor that lowers future cost |
| --- | --- | --- |
| `frontends/rioterm/src/screen/mod.rs` | Owns render flow, tab/island hit testing, shader frame state, cursor trails, hyperlink hover, pointer logic, and many Yazelix UI paths | Extract island input, overlay input, cursor/shader frame-state construction, and render scheduling into narrower modules |
| `frontends/rioterm/src/application.rs` | Mixes Rio event routing, confirm-quit dialog state, mouse dispatch, window lifecycle, native tabs, and redraw scheduling | Move route-local modal/input handling behind route methods; keep application-level code on window lifecycle |
| `frontends/rioterm/src/renderer/island.rs` | Fork owns progress timing fixes, title truncation, per-tab colors, rename input, and color picker behavior; upstream owns tab drag springs/layout | Centralize `TabStripLayout`; move per-tab metadata to tab/context state before drag reorder |
| `frontends/rioterm/src/context/mod.rs` and `context/title.rs` | Fork still stores title data in index-keyed maps; upstream moved title/custom metadata closer to each tab/context | Move title, custom title, and tab color metadata to tab-owned state so reorder operations naturally preserve metadata |
| `frontends/rioterm/src/renderer/trail_cursor.rs` | Fork owns Helix one-row trail behavior and redraw-warp snap policy; upstream owns route reset and reusable spring helpers | Keep fork snap policy, but consider sharing a small spring helper only if it reduces duplication without changing motion |
| `frontends/rioterm/src/renderer/mod.rs` | Fork owns shader/event-mode handoff, frame metrics, and renderer state beyond upstream Rio | Keep shader state isolated and avoid broad upstream renderer rewrites without screenshot/manual evidence |
| `sugarloaf/src/text.rs` and `frontends/rioterm/src/grid_emit.rs` | Font fallback behavior affects emoji/icons, glyph protocol, status symbols, and UI text | Keep fallback routing through `FontLibrary::resolve_font_for_char`; add tests before changing fallback search order |
| Kitty graphics files | Protocol behavior is correctness-sensitive and should follow spec/black-box evidence | Keep small parser/storage/renderer ports with protocol tests and no GPL Kitty implementation copying |
| `misc/`, `pkgRio.nix`, packaged shader/config files | These are mars runtime surfaces, not upstream Rio surfaces | Treat as fork-owned packaging; sync only when upstream changes affect build inputs or binary layout |

## Tab Drag And Island Decision

Inspected upstream Rio commits:

- `e5d023a2e8` `allow drag and reorder tabs`
- `c8609a2f8f` `few updates`
- `77177db733` `wip`
- `db9ded4788` `small cleanup`
- `0ad5dc4d10` `simplify tab titles`
- `9952bdae37` `smalle refactor`
- `5d49e19c4f` `change placeholder logic for default`
- `15529c01b4` `small cleanup`
- `77b615b678` `few fixes for the color picker`

Decision: do not port the tab drag/island series directly in the v0.4.7 sync.
Adapt it later only after the tab metadata ownership seam is cleaned up.

What upstream adds:

- `TabStripLayout` as shared tab geometry for render, hit testing, color picker,
  and drag math
- `TabDrag` state and slide springs for drag/reorder animation
- `ContextManager::move_current_tab_to`
- title/custom title/color behavior that follows tab movement because metadata
  lives on the tab/context side
- macOS titlebar drag handling for island gaps

Why direct port is unsafe today:

- current fork title state is still index-keyed through `ContextManagerTitles`
- current fork island state owns per-tab colors and custom titles in local maps
- reordering tabs without first moving that metadata would make title/color
  state follow indexes instead of the actual tab
- the touched files overlap with confirm-quit, shader/event-mode, hyperlink,
  cursor trail, and Yazelix host-mode deltas
- the upstream series is a broad UI refactor, not a small correctness fix

What to borrow later:

- `TabStripLayout` should be the first safe extraction because the fork already
  duplicates equal-width tab geometry across render, hit testing, and picker
  code
- `move_current_tab_to` semantics are better than repeated adjacent swaps for
  drag reorder, but only after tab-owned metadata is in place
- tab drag springs are acceptable if they reuse a small helper and keep redraw
  scheduling paced
- macOS titlebar drag changes should be adapted only if they preserve the
  fork's terminal chrome and island visibility rules

Validation for a future tab drag port:

- context-manager tests proving title, custom title, custom color, route id, and
  rich text id follow multi-slot moves
- island layout tests for click, picker, and drag hit zones
- manual smoke for click-select, right-click color picker, drag reorder, tab
  close, confirm-quit, and shader/event-mode cursor behavior
- `cargo fmt --check`
- `git diff --check`

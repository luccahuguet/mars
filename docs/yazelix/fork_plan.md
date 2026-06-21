# Mars Fork Plan

Mars is a Rio-derived terminal for Yazelix dogfooding. The fork stays easy to rebase by keeping Rio source changes rare, measured, and feature-specific.

## Lessons

- Large renderer, PTY, event-loop, and shader changes make terminal bugs hard to isolate
- Yazelix already uses Zellij, so Mars should avoid duplicating multiplexing UI unless there is a clear reason
- The first useful Mars is a stable Rio clone with Yazelix-friendly launch and config surfaces
- Reproducible performance evidence comes before visual features
- Ghostty is the primary behavior target; WezTerm is a useful mature comparison target

## Baseline

- Branch from latest upstream Rio
- Keep Rio-owned source files unchanged for the first milestone
- Add Mars identity through wrapper packaging, app id, desktop entry, icon, and package metadata
- Keep Rio and Mars config content identical at first; use different config directories and launch env for isolation
- Keep Yazelix main free of Mars CI, docs, public launchers, and runtime choices until dogfooding passes

## Feature Discipline

Every Mars feature follows a small-change rule:

- Use the least amount of code that solves the feature
- Plan the feature in a Bead before implementation
- Prefer config, wrapper, package, desktop, or docs changes before Rio source edits
- Add one focused feature at a time
- Review the diff against `rio-upstream/main` before closing the work
- Keep verification proportional to risk, with reproducible artifacts for renderer, PTY, parser, event-loop, shader, or window behavior

Feature design should answer:

- Why this belongs in Mars instead of Yazelix main, user config, or wrapper packaging
- Which upstream Rio files are touched
- Whether the change stays easy to carry while pulling upstream Rio commits
- What would let the feature be deleted, disabled, or upstreamed later

## First Milestones

1. `mars`: exact Rio behavior with Mars identity
2. `mars-yazelix`: private launcher/config root that runs `yzx start`
3. Reproducible performance gate: idle CPU, scroll/render stress, PTY flood, and bounded Yazelix screen workloads
4. Config isolation: Mars uses a clear Mars-owned config path, but the TOML stays Rio-compatible until a measured Mars-only feature needs different config

## Feature Order

1. Static background image support
2. Yazelix-safe defaults that do not overlap with Zellij
3. Theme/profile presets
4. Runtime diagnostics for frame time, PTY pressure, invalidations, and CPU
5. Shader/background effects, only behind an easy off switch

## Gate

Any change touching PTY reading, parser batching, renderer invalidation, compositor scheduling, event-loop behavior, or shaders needs a Bead and fresh reproducible artifacts before it ships.

# Mars Agent Guide

Mars is a Rio-derived terminal fork maintained for Yazelix and agent-driven
development. Keep changes small, measured, and easy to rebase against upstream
Rio.

## Workflow

- Use `br` for Beads planning in this repository
- Plan behavior work in Beads before editing Rio-owned source
- Prefer additive Mars-owned files over broad Rio source rewrites
- Keep each feature small, measurable, and easy to remove or upstream
- Compare relevant Ghostty and WezTerm behavior before terminal-behavior edits
- Search Rio issues and current upstream Rio commits before local Rio-owned
  source changes
- Record Mars-owned code or runtime-behavior commits in
  `docs/yazelix/change_scorecard.md`
- Preserve normal non-Nix packaging routes where practical, but avoid public
  support claims until each path is validated

## Source Boundaries

- Rio-owned source includes the inherited crates and frontend/backend terminal
  code
- Mars-owned surfaces include `pkgMars.nix`, `flake.nix`, `misc/mars.desktop`,
  `tools/mars_*`, and `docs/yazelix/*`
- Prefer package, wrapper, config, desktop, docs, and tool changes before
  renderer, PTY, parser, event-loop, or window changes
- If a direct Rio-owned source edit is smaller than an abstraction, keep it
  direct and document why in the Bead or scorecard

## Validation

Choose checks proportional to the touched surface:

- README/docs only: `git diff --check`
- GitHub workflow changes: `actionlint .github/workflows/<file>.yml`
- Nix package or metadata changes:
  `nix eval --json .#packages.x86_64-linux.mars.passthru.marsPackageMetadata --no-write-lock-file`
- Mars package changes:
  `nix build .#mars --no-link --print-build-logs`
- Rust changes:
  `cargo fmt -- --check --color always`,
  `cargo clippy --all-targets --all-features`, and
  `cargo test --features wgpu`

## Release Workflow

The release workflow runs for `v*.*.*` tags and manual dispatch only. Do not
re-enable ordinary `main` push releases unless the GoReleaser Pro and Apple
signing secret policy is decided and documented. The release-secrets evaluation
is tracked by Bead `yzt-c2d`.

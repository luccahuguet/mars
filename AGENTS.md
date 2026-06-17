# Agent Guidelines

Shared Yazelix agent workflow and release policy live in the main repo:

- https://github.com/luccahuguet/yazelix/blob/main/AGENTS.md
- In sibling local checkouts, read `../yazelix/AGENTS.md` first

Only Mars Terminal-specific guidance belongs here.

## Local Scope

- This repository is the experimental Rio-derived Mars Terminal workspace; current package and integration surfaces still use `yazelix-terminal` and `yzxterm`.
- Use Rio upstream as the implementation base and keep the fork delta reviewable.
- Treat Ghostty as the primary behavior and quality target.
- Treat WezTerm as a mature terminal-engine comparison target.
- Treat Kitty implementation code as GPL-owned reference material: use official specs and black-box behavior unless a licensing decision explicitly allows more.

## Local Commands

- For visual source edits, prefer `tools/yazelix_terminal_local.sh` before paying for a Nix package or Home Manager rebuild.
- Do not run yzxterm-related compile-heavy commands again until the rebuild-speed optimization beads are addressed, unless the maintainer explicitly overrides that gate.
- After the rebuild-speed gate is addressed, use the main Yazelix repo's fast outputs `#runtime_yzxterm_fast` and `#yzxterm_fast` for maintainer dogfooding.
- Keep the normal checked `#runtime_yzxterm` path as release evidence.

## Integration Notes

This repo has its own Beads database for terminal-local planning. Main Yazelix owns integrated runtime selection, Home Manager switching, and release transaction policy.

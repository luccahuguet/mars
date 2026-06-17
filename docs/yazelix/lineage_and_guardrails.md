# Mars Terminal Lineage And Guardrails

Mars Terminal is an experimental Rio-derived terminal intended to evaluate
whether Yazelix can get a Rust terminal to Ghostty-grade behavior.

The current package and integration surfaces still use `yazelix-terminal` and
`yzxterm` while the public project name moves toward Mars Terminal.

It is not yet a supported Yazelix runtime component.

## Base

- Fork base: Rio upstream
- Upstream remote: `https://github.com/raphamorim/rio.git`
- Local upstream remote name: `rio-upstream`
- Experiment origin: `git@github.com:luccahuguet/yazelix-terminal.git`
- Initial experiment branch: `yazelix-terminal-experiment`
- Base commit: `7e18dde1c90182a5170a7cca7779544967d7291c`
- Base commit subject: `flake.lock: Update (#1636)`
- Base commit date: `2026-05-31T07:16:47+02:00`

Rio is MIT licensed. The root `LICENSE` remains Rio's MIT license and copyright notice.

## Reference Sources

Local read-only reference clones:

- Ghostty: `/home/lucca/pjs/open_source/yazelix_related/ghostty`
  - Commit: `c4eba3da38c629dbd4b8f770da3e0c605f2a7f53`
  - License: MIT
- WezTerm: `/home/lucca/pjs/open_source/yazelix_related/wezterm`
  - Commit: `577474d89ee61aef4a48145cdec82a638d874751`
  - License: MIT
- Rio mirror: `/home/lucca/pjs/open_source/yazelix_related/rio`
  - Commit: `6a11aa33c7cf713a763f144b4a737215fa6fae0d`
  - License: MIT

Kitty is a protocol authority for many modern terminal extensions, but its implementation is GPL-3.0. Use Kitty official protocol specifications and black-box behavior by default. Do not copy Kitty implementation code into this MIT-derived fork unless a separate licensing decision is made first.

## Product Boundary

The goal is Ghostty parity for Yazelix-relevant behavior:

- Ghostty-compatible cursor shader semantics
- modern graphics, keyboard, clipboard, shell, and identity protocols
- reliable behavior through Zellij, Yazi, and Helix
- a Yazelix mode where Zellij owns workspace panes and tabs

The goal is not to clone Ghostty's native application shell or WezTerm's multiplexer.

## Integration Boundary

Main Yazelix must not depend on this repository until the experiment has evidence for:

- a buildable terminal on the target platform
- cursor shader feasibility
- protocol conformance for the must-have matrix
- clear launch/config semantics for Yazelix mode
- license and attribution notes for any code ported from permissive upstreams

Until then, main Yazelix may use this repo only through local overrides or manual experiments.

## Baseline Build Status

The first local metadata check succeeded with:

- `rustc 1.95.0`
- `cargo 1.95.0`
- Cargo workspace members resolved through `cargo metadata --no-deps --format-version 1`

Rio declares MSRV `1.96.0`, so the real build and launch baseline belongs to `yzt-7p3.4` and must use a Rust 1.96 toolchain or record the exact toolchain failure there.

## Hard Problem Rule

If a feature turns out to require a major renderer, parser, windowing, or architecture rewrite:

1. stop before speculative thrashing
2. document the source paths inspected
3. record why the straightforward approach failed
4. list rejected approaches and viable pivots
5. mark or split the bead accordingly
6. move to another ready bead

This keeps the experiment moving without burying hard decisions inside half-finished code.

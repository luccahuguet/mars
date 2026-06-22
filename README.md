# Mars Terminal

Mars is a Rio-derived Rust terminal fork maintained for Yazelix and
agent-driven development.

The project keeps Rio close enough to upstream to rebase deliberately, while
giving Yazelix a terminal stack it can control, test, package, and adapt when
terminal behavior matters. Mars aims for practical Ghostty parity where Yazelix
depends on it, strong Kitty protocol support, good Nix/runtime integration, and
small measured changes instead of broad fork drift.

Mars is the default packaged terminal for Yazelix. Ghostty remains the mature
alternate in Yazelix; Mars exists so the Rust terminal/runtime boundary can move
with Yazelix when protocol, cursor, graphics, and packaging work needs it.

![Mars running Yazelix](docs/assets/mars-yazelix-session.png)

## Current Shape

- The source tree is based on upstream Rio and still carries Rio crate names in
  many Rust packages
- The first-class Nix package is `.#mars`; `.#default` points at the same
  package
- The package wraps the upstream Rio binary as `bin/mars`, installs Mars desktop
  metadata and icons, and sets the app id to `mars`
- The package exposes `passthru.marsPackageMetadata` and installs the same data
  at `share/mars/package-metadata.json`
- The package includes generated Mars config roots for Yazelix:
  `share/mars`, `share/mars/baseline`, `share/mars/profiles/shaders`,
  `share/mars/emoji/twitter`, and `share/mars/emoji/serenityos`
- The package metadata advertises `MARS_APPEARANCE`, `MARS_EMOJI_FONT`,
  `MARS_EMOJI_FONT_SOURCE`, and `MARS_PROFILE` as the wrapper environment
  contract consumed by Yazelix
- On Linux, the Nix wrapper provides a package-owned default Vulkan ICD path
  when `VK_ICD_FILENAMES` is unset, while preserving explicit user overrides
- Rio package outputs remain exposed as `.#rio`, `.#rio-msrv`, `.#rio-stable`,
  and `.#rio-nightly` for comparison and upstream maintenance work

## Install

Build the Mars package with Nix:

```sh
nix build github:luccahuguet/mars#mars
./result/bin/mars
```

Install it into a Nix profile:

```sh
nix profile install github:luccahuguet/mars#mars
mars
```

Build from a local checkout:

```sh
nix build .#mars
./result/bin/mars
```

The Cargo workspace still follows upstream Rio's crate layout. Use Cargo for
source-level development and CI parity:

```sh
cargo build --release --features wgpu
```

## Yazelix Integration

Yazelix consumes Mars through the Nix package contract, not by guessing Rio
internals. The package metadata tells Yazelix where Mars configs live, which
emoji presets and appearance modes are supported, and which wrapper command to
launch.

Mars config roots are designed for generated Yazelix runtime state. User-facing
Yazelix configuration belongs in Yazelix; terminal implementation details stay
in Mars.

For local Yazelix dogfooding, use the private launcher:

```sh
tools/mars_private_yazelix.py
```

Set `MARS_BINARY=/path/to/mars` to test a specific build artifact. Set
`MARS_CONFIG_HOME` or `MARS_PRIVATE_CONFIG_HOME` to override the private config
root.

## Development

The Cargo workspace follows upstream Rio's crate layout and MSRV. Nix exposes
the Mars package and comparison Rio packages. Repository workflow rules live in
[`AGENTS.md`](AGENTS.md).

Useful project docs:

- [`docs/yazelix/fork_plan.md`](docs/yazelix/fork_plan.md)
- [`docs/yazelix/upstream_maintenance.md`](docs/yazelix/upstream_maintenance.md)
- [`docs/yazelix/clean_rio_rebuild_gate.md`](docs/yazelix/clean_rio_rebuild_gate.md)
- [`docs/yazelix/non_nix_graphics_launch_support.md`](docs/yazelix/non_nix_graphics_launch_support.md)

## Performance And Debugging

Mars keeps reproducible dogfooding tools in the repo. The performance gate
launches Mars with deterministic workloads and writes artifacts under
`artifacts/dogfooding/`.

Run the default suite:

```sh
tools/mars_perf_gate.py --suite --seconds 20
```

Run one scenario:

```sh
tools/mars_perf_gate.py --suite --scenario pty_flood --seconds 20
```

Enable gated internal PTY/render metrics for suite-launched Mars:

```sh
tools/mars_perf_gate.py --suite --scenario pty_flood --seconds 20 --internal-metrics
```

Trace a launch boundary:

```sh
mars-launch-trace -- mars -e true
```

## Verification

Useful local checks:

```sh
cargo fmt -- --check --color always
cargo clippy --all-targets --all-features
cargo test --features wgpu
nix build .#mars --no-link --print-build-logs
actionlint .github/workflows/release.yml
```

The GitHub `Test` workflow runs native Linux, macOS, and Windows Rust checks,
plus MSYS2 release builds for `MINGW64`, `UCRT64`, and `CLANG64`.

The GitHub `Nix Build` workflow builds the flake package on Linux ARM.

## Release Status

The release workflow is intentionally limited to `v*.*.*` tags and manual
dispatch. It uses inherited GoReleaser Pro release machinery, requires
`GORELEASER_KEY` for release execution, and only configures Apple signing when
signing secrets are present.

The release-secrets decision is tracked in Bead `yzt-c2d`. Until that is
resolved, treat Nix builds and source builds as the validated first-party
surfaces, and treat inherited Rio release packaging as a path to evaluate
instead of a public Mars guarantee.

## Upstream

Mars inherits substantial code and history from
[Rio](https://github.com/raphamorim/rio). Upstream Rio remains the baseline for
terminal behavior, renderer fixes, and cross-platform packaging context. Mars
should upstream generic fixes when they are useful beyond Yazelix.

## License

Mars follows Rio's MIT licensing.

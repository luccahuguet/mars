# Build Speed Workflow

Use the cheapest validation surface that matches the edit.

## Rust Inner Loop

Enter the Nix development shell once so Cargo sees the pinned Rust toolchain and
the graphics/font development libraries:

```sh
nix develop
```

Then use Cargo directly for source edits:

```sh
cargo check -p rioterm
cargo build -p rioterm
cargo run -p rioterm -- --version
```

This keeps incremental Cargo artifacts in `target/` and avoids rebuilding the
Nix package for every Rust edit.

To open the local cargo-built terminal with the Yazelix packaged config shape:

```sh
tools/yazelix_terminal_local.sh
```

The launcher builds `target/debug/rio` by default, materializes resolved config
templates under `target/yazelix-terminal-local/`, sets app id
`yazelix-terminal-local`, and preserves the desktop wrapper's profile,
renderer-strategy, graphics-wrapper, and child-environment cleanup contracts.
It never falls back to a host `rio` on `PATH`.

Useful local launcher knobs:

| Variable | Behavior |
| --- | --- |
| `YAZELIX_TERMINAL_LOCAL_SKIP_BUILD=1` | Run the existing local binary without invoking Cargo |
| `YAZELIX_TERMINAL_LOCAL_PROFILE=fast` | Build and run `cargo build --profile fast -p rioterm --features wgpu` |
| `YAZELIX_TERMINAL_LOCAL_PROFILE=release` | Build and run the local release binary |
| `YAZELIX_TERMINAL_LOCAL_BINARY=/path/to/rio` | Run an explicit binary instead of `target/<profile>/rio` |
| `YAZELIX_TERMINAL_PROFILE=baseline` | Use the no-effects baseline config |
| `YAZELIX_TERMINAL_RENDER_STRATEGY=game` | Generate a runtime config copy with `strategy = "game"` |
| `YAZELIX_TERMINAL_GRAPHICS_WRAPPER=none` | Skip nixGL/nixVulkan wrapper discovery |

## Packaging Loop

Use the fast package when validating local desktop entries, wrapper scripts, or
launcher behavior before paying for the full release profile:

```sh
nix build .#mars-fast -o result_mars_fast_package
```

`mars-fast` has the same wrapped desktop package shape as
`mars`, but its unwrapped Rust derivation uses Cargo profile `fast`
and sets `doCheck = false`. The fast profile disables LTO, avoids packed full
debug info, uses many codegen units, and keeps only modest `opt-level = 1`
optimization. It is for maintainer smoke testing, not release evidence.

When dogfooding through the main Yazelix runtime, use the main repo's explicit
fast outputs instead of overriding the child input into the normal release
runtime:

```sh
nix build .#runtime_yzxterm_fast --no-link --no-write-lock-file
nix run .#yzxterm_fast -- launch
```

The normal main-repo `#runtime_yzxterm` and Home Manager default path still use
the checked `mars` package and remain the release gate.

Wrapper-only changes should rebuild only the cheap `mars` wrapper
derivation. The Rust compile lives in `mars-unwrapped`, while the
desktop file, icon, terminfo, app-id wrapper, and graphics-wrapper launcher live
in `mars`.

## Release Gate

Use the checked package before claiming package correctness:

```sh
nix build .#mars -o result_mars_package
desktop-file-validate result_mars_package/share/applications/yazelix-terminal.desktop
result_mars_package/bin/mars --version
```

Use the protocol conformance harness when parser/protocol behavior changed:

```sh
nix run .#yazelix-protocol-conformance -- verify
```

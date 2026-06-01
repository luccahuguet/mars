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

## Packaging Loop

Use the fast package when validating local desktop entries, wrapper scripts, or
launcher behavior before paying for the full release profile:

```sh
nix build .#yazelix-terminal-fast -o result_yazelix_terminal_fast_package
```

`yazelix-terminal-fast` has the same wrapped desktop package shape as
`yazelix-terminal`, but its unwrapped Rust derivation uses Cargo profile `fast`
and sets `doCheck = false`. The fast profile disables LTO, avoids packed full
debug info, uses many codegen units, and keeps only modest `opt-level = 1`
optimization. It is for maintainer smoke testing, not release evidence.

Wrapper-only changes should rebuild only the cheap `yazelix-terminal` wrapper
derivation. The Rust compile lives in `yazelix-terminal-unwrapped`, while the
desktop file, icon, terminfo, app-id wrapper, and graphics-wrapper launcher live
in `yazelix-terminal`.

## Release Gate

Use the checked package before claiming package correctness:

```sh
nix build .#yazelix-terminal -o result_yazelix_terminal_package
desktop-file-validate result_yazelix_terminal_package/share/applications/yazelix-terminal.desktop
result_yazelix_terminal_package/bin/yazelix-terminal --version
```

Use the protocol conformance harness when parser/protocol behavior changed:

```sh
python3 tools/yazelix_conformance.py verify
```

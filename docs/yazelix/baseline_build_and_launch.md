# Baseline Build And Launch

Status: completed for bead `yzt-7p3.4`.

## Environment

- Session: COSMIC on Wayland (`XDG_SESSION_TYPE=wayland`)
- Rio version: `rioterm 0.4.6`
- Rio Rust toolchain: `rust-toolchain.toml` pins Rust `1.96`
- Shell PATH caveat: `/home/lucca/.nix-profile/bin/rustc` and `cargo` are Rust
  `1.95`, so direct commands must explicitly avoid that shadowing
- GPU visible to host Vulkan: NVIDIA GeForce RTX 3050 6GB Laptop GPU

## Build Results

Direct host build with rustup Cargo:

```text
RUSTC=/home/lucca/.rustup/toolchains/1.96-x86_64-unknown-linux-gnu/bin/rustc \
  ~/.cargo/bin/cargo build -p rioterm
```

Result: failed because host development packages are incomplete.

```text
The system library `fontconfig` required by crate `yeslogic-fontconfig-sys`
was not found.
The file `fontconfig.pc` needs to be installed and the PKG_CONFIG_PATH
environment variable must contain its parent directory.
```

The host has runtime libraries, but not the relevant `.pc` development files:

```text
pkg-config --modversion fontconfig  # not found
pkg-config --modversion vulkan      # not found
```

`sudo -n true` failed, so system package installation was not available without
interactive credentials during this run.

Flake-shell build:

```text
nix develop -c cargo build -p rioterm
```

Result: succeeded.

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 39.21s
```

The only observed compiler warning was in `rio-window`:

```text
warning: direct cast of function item into an integer
rio-window/src/platform_impl/linux/x11/ime/context.rs:178:38
```

## Launch Results

Raw launch outside the flake shell failed because Rio could not load the Wayland
runtime path expected by its current build:

```text
Error: Os(OsError {
  file: "rio-window/src/platform_impl/linux/wayland/event_loop/mod.rs",
  error: WaylandError(Connection(NoWaylandLib))
})
```

Flake-shell launch with the default native Vulkan backend failed in Sugarloaf:

```text
vkCreateInstance failed - is a Vulkan 1.3 driver installed?:
ERROR_INCOMPATIBLE_DRIVER
```

Host `vulkaninfo --summary` does see the NVIDIA driver, so this is not simply
"no Vulkan on the machine". The current failure is a mixed Nix/host graphics
boundary: the Nix-linked Rio binary and Nix loader/runtime do not cleanly use
the host NVIDIA stack in this environment.

Adding the host library directory to the Nix `LD_LIBRARY_PATH` is not valid; it
caused a glibc symbol mismatch:

```text
target/debug/rio: symbol lookup error:
/usr/lib/x86_64-linux-gnu/libc.so.6: undefined symbol:
__nptl_change_stack_perm, version GLIBC_PRIVATE
```

Pinning only the host NVIDIA ICD with `VK_ICD_FILENAMES` did not fix native
Vulkan launch.

## Successful Launch Evidence

Rio launches successfully inside the flake shell when configured to use the CPU
renderer:

```toml
[renderer]
use-cpu = true
```

Command used:

```text
RIO_CONFIG_HOME=/home/lucca/pjs/yazelix-terminal/artifacts/baseline/rio_cpu_config \
  target/debug/rio \
  --app-id yazelix-terminal-baseline \
  --title-placeholder "Mars Terminal Baseline" \
  -e bash -lc 'printf "yazelix-terminal baseline\nrioterm 0.4.6\nCPU renderer\nPID $$\n"; sleep 20'
```

Screenshot evidence:

```text
artifacts/baseline/Screenshot_2026-05-31_02-40-55.png
```

The screenshot shows a Rio window rendering the baseline command output.

## Implications

The baseline is good enough to proceed with parser, protocol, config, and
non-GPU terminal behavior work.

Cursor shader parity still needs a GPU-renderer launch path. The next shader
proof should start by choosing one of these paths:

- host-linked build after installing host development packages
- Nix graphics wrapper such as a nixGL-style launch boundary
- WGPU build/config path if it can use GL or Vulkan cleanly on this host
- native Vulkan fix inside Rio/Sugarloaf only if the failure reproduces outside
  the mixed Nix/host environment

Do not treat CPU renderer success as shader evidence.

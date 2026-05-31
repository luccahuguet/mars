# Yazelix Stack Validation

This note records the current validation state for running Yazelix inside
`yazelix-terminal` host mode.

## Environment

- Terminal command: `target/debug/rio --yazelix -e yzx enter --path /home/lucca/pjs/yazelix-terminal`
- Renderer: WGPU with `WGPU_BACKEND=gl`
- Yazelix runtime observed during the stack probe: `v17.2`
- Stack components observed by `yazi --debug`: Zellij `0.44.3`, Yazi `26.5.6`, Helix `25.07.1`

## Terminal Identity

`--yazelix` mode uses Rio's terminal identity for child capability detection:

- `TERM=rio`
- `TERM_PROGRAM=rio`
- `TERM_PROGRAM_VERSION=0.4.6`
- `YAZELIX_TERMINAL_HOST=yazelix-terminal`

The host mode scrubs inherited terminal identity markers such as Ghostty,
Kitty, WezTerm, Alacritty, Konsole, Windows Terminal, and inherited
`TERMINFO`/`TERMCAP` values before spawning the child command. This avoids
mixed identities when the experimental host is launched from another terminal.

## Image Protocols

Validated image paths:

- Direct Sixel under WGPU/GL:
  `artifacts/stack_validation/screenshots/direct_sixel_wgpu_gl.png`
- Direct Kitty graphics unicode-placeholder path under WGPU/GL:
  `artifacts/stack_validation/screenshots/direct_kitty_unicode_placeholder_wgpu_gl.png`
- Yazi image preview through Yazelix/Zellij using Kitty graphics:
  `artifacts/stack_validation/screenshots/yazelix_zellij_yazi_kitty_preview_wgpu_gl.png`

The Yazi stack probe reported:

```text
Brand.from_env      : Some(Rio)
Emulator.detect     : Emulator { kind: Left(Rio), version: "Zellij(4403)", ... }
Adapter.matches    : Kgp
```

Two renderer bugs were fixed during this validation:

- Sixel/iTerm atlas graphics were parsed and queued but not painted by the
  current renderer pipeline. Atlas graphics now feed the existing image-overlay
  texture path with a reserved renderer namespace.
- Kitty `U=1` virtual placements from Yazi omit explicit `c=`/`r=`
  dimensions. The backend now infers the placement grid from image dimensions
  and current cell metrics before registering the virtual placement.

The virtual placement source rectangle uses the renderer shader shape
`[u0, v0, width, height]`.

## Yazelix Workflow

Validated stack surfaces:

- `yzx enter` runs under `target/debug/rio --yazelix`
- Zellij owns panes/tabs while Rio native split ownership is disabled
- Yazi opens inside the Yazelix session and renders a PNG preview through
  Kitty graphics
- Helix opens inside the Yazelix session:
  `artifacts/stack_validation/screenshots/yazelix_zellij_yazi_helix_wgpu_gl.png`

The `--with core.skip_welcome_screen=true` path hit a main Yazelix config
contract error unrelated to the terminal fork. Stack probes used
`YAZELIX_STARTUP_PROFILE_SKIP_WELCOME=true` instead.

## Verification Commands

```bash
nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' graphics_namespace -- --nocapture
nix develop -c cargo test -p rioterm --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' yazelix_mode -- --nocapture
nix develop -c cargo test -p rio-backend --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_virtual -- --nocapture
nix develop -c cargo build -p rioterm --features wgpu
python3 tools/yazelix_conformance.py verify
git diff --check
```

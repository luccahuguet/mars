## Yazelix Terminal Fork

This branch is the Rio-derived `yazelix-terminal` experiment for Ghostty-grade
Yazelix terminal behavior. The fork status, feature ledger, and verification
evidence live in
[`docs/yazelix/fork_feature_verification.md`](docs/yazelix/fork_feature_verification.md).
Build-speed guidance lives in
[`docs/yazelix/build_speed_workflow.md`](docs/yazelix/build_speed_workflow.md).

## Yazelix Package Surface

The flake exposes the Yazelix-owned package and app names while keeping `rio`
aliases for compatibility:

```sh
nix build .#yazelix-terminal -o result_yazelix_terminal_package
nix run .#yazelix-terminal -- --version
```

Useful package outputs:

- `.#yazelix-terminal`: checked package with the desktop wrapper and full checks
- `.#yazelix-terminal-fast`: same wrapper shape using the maintainer fast Cargo profile for local iteration
- `.#yazelix-terminal-unwrapped`: unwrapped Rio-derived binary
- `.#rio`: compatibility alias to `.#yazelix-terminal`

The package installs:

- `bin/yazelix-terminal`
- `bin/yazelix-terminal-desktop`
- `share/applications/yazelix-terminal.desktop`
- `share/yazelix-terminal/config.toml`
- `share/yazelix-terminal/baseline/config.toml`

The desktop wrapper sets `--app-id yazelix-terminal`, searches for available
Nix graphics wrappers, and maps Yazelix-owned config directories into Rio's
supported `RIO_CONFIG_HOME` contract only for the terminal process. It ignores
ambient host `RIO_CONFIG_HOME`; use `YAZELIX_TERMINAL_CONFIG` for an explicit
Yazelix Terminal config override. Child shells launched by the packaged wrapper
do not inherit Yazelix Terminal's private `RIO_CONFIG_HOME` or package loader
paths, so plain host `rio` invocations keep using the user's host Rio defaults.
The packaged config disables confirm-before-quit, disables native window
decorations, sets the terminal font size to `18.0`, and uses the default event
renderer strategy with WebGPU, packaged cursor shaders, and trail cursor
effects. `YAZELIX_TERMINAL_PROFILE=baseline` selects the same packaged font,
window, and WebGPU baseline without custom shaders or trail cursor effects for
performance comparisons. `YAZELIX_TERMINAL_RENDER_STRATEGY=game` is kept as an
explicit diagnostic override and composes with either profile.

Wrapper override knobs:

| Variable | Behavior |
| --- | --- |
| `YAZELIX_TERMINAL_CONFIG` | Uses a custom Rio config directory; must contain readable `config.toml` |
| `YAZELIX_TERMINAL_PROFILE=full` | Uses the packaged shader/trail defaults |
| `YAZELIX_TERMINAL_PROFILE=baseline` | Uses the packaged no-effects baseline config |
| `YAZELIX_TERMINAL_EFFECTS=none` | Alias for the baseline no-effects profile |
| `YAZELIX_TERMINAL_RENDER_STRATEGY=events` | Uses the packaged config with Rio's default event renderer strategy |
| `YAZELIX_TERMINAL_RENDER_STRATEGY=game` | Creates a runtime copy of the packaged config with `strategy = "game"` for diagnostics |
| `YAZELIX_TERMINAL_GRAPHICS_WRAPPER=none` | Skips automatic nixGL/nixVulkan wrapper discovery |
| `YAZELIX_TERMINAL_GRAPHICS_WRAPPER=/path/to/wrapper` | Runs the terminal through the selected wrapper |

## Yazelix Stack Behavior

`--yazelix` host mode keeps Zellij, Yazi, and Helix as the workspace stack while
the terminal owns modern rendering and terminal protocols. In host mode:

- child applications still see Rio-compatible terminal identity for capability detection
- `YAZELIX_TERMINAL_HOST=yazelix-terminal` marks the fork-specific host
- inherited terminal identity markers are scrubbed before spawning the child
- Rio native split/config-editor ownership is disabled for Yazelix sessions

Kitty graphics, Sixel, iTerm2 images, OSC 133, OSC 66, OSC 99, OSC 52, Kitty
keyboard, Kitty multiple cursors, Kitty file transfer, OSC 5522 text clipboard,
unscrolling, and DECCARA coverage are tracked in the verification ledger.

Yazi image and PDF previews use Kitty graphics through the terminal image
overlay path. The renderer image-overlay ABI is `[u0, v0, width, height]`
across WGPU, native Vulkan, Metal, atlas graphics, and Kitty virtual
placements. Stack validation notes live in
[`docs/yazelix/stack_validation.md`](docs/yazelix/stack_validation.md).

## Validation

Release-oriented checks:

```sh
nix build .#yazelix-terminal -o result_yazelix_terminal_package
desktop-file-validate result_yazelix_terminal_package/share/applications/yazelix-terminal.desktop
result_yazelix_terminal_package/bin/yazelix-terminal --version
python3 tools/yazelix_conformance.py verify
```

Focused graphics checks:

```sh
nix develop -c cargo test -p rio-backend --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_virtual -- --nocapture
nix develop -c cargo test -p sugarloaf --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' image_shaders_use_origin_size_source_rect -- --nocapture
```

The upstream Rio README follows below.

<!-- LOGO -->
<h1>
<p align="center">
  <img src="https://rioterm.com/assets/rio-logo.png" alt="Rio terminal logo" width="128">
  <br>Rio Terminal
</h1>
  <p align="center">
    Rio is a modern terminal built to run everywhere.
    <br />
    <a href="#about">About</a>
    ·
    <a href="https://rioterm.com/docs/install">Install</a>
    ·
    <a href="https://rioterm.com/docs/config">Config</a>
    ·
    <a href="https://rioterm.com/changelog">Changelog</a>
    ·
    <a href="https://github.com/sponsors/raphamorim">Sponsor</a>
  </p>
</p>

Documentation: [rioterm.com](https://rioterm.com).

## Supporting the Project

If you use and like Rio, please consider sponsoring it: your support helps to cover the fees required to maintain the project and to validate the time spent working on it!

[![Sponsor Rio terminal](https://img.shields.io/github/sponsors/raphamorim?label=Sponsor%20Rio&logo=github&style=for-the-badge)](https://github.com/sponsors/raphamorim)

## Packaging

[![Packaging status](https://repology.org/badge/vertical-allrepos/rio-terminal.svg?columns=3)](https://repology.org/project/rio-terminal/versions)

> Demo with split and CRT on MacOS

![Demo Rio 0.2.0 on MacOS](https://rioterm.com/assets/posts/0.2.0/demo-rio.png)

> Demo with blurred background on Linux

![Demo blurred background](https://rioterm.com/assets/demos/demos-nixos-blur.png)

> Demo of Rio running on a Steam Deck

![Demo of Rio running on a Steam Deck](https://rioterm.com/assets/demos/demo-flatpak-steamdeck.jpg)

## Minimal stable rust version

Rio's MSRV is 1.96.0.

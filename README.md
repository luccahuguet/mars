# Yazelix Terminal Fork

Yazelix Terminal is an experimental active Yazelix fork of Rio for dogfooding a
first-party Rust terminal path with Yazelix-controlled package metadata,
profiles, protocols, notifications, Kitty graphics, and cursor-shader
boundaries.

| Field | Value |
| --- | --- |
| Upstream project | Rio |
| Fork category | Experimental active fork |
| Why this fork exists | Yazelix needs a Rust terminal it can evolve as a first-party runtime, with protocol coverage and runtime integration that Ghostty cannot expose to Yazelix directly |
| Current Yazelix delta | `yazelix-terminal` and `yzxterm` package/profile names, desktop wrapper, package metadata passthru, generated profile templates, Rio trail defaults, packaged emoji/Nerd Font glyph fallback, `yazelix-cursors` shader support, BELL/terminal notification behavior, Kitty graphics support, and Ghostty-compatible shader ABI |
| Non-goals | This fork does not own the full Yazelix workspace, Zellij/Yazi/Helix integration policy, or compatibility shims in the main repo |
| Standalone support | Supported for experimental users through the flake package and app outputs; normal Yazelix users consume it through main-repo runtime outputs |
| Upstream sync cadence | Monthly while dogfooding, and before any yzxterm release-gate decision |
| Promotion/removal gate | Promote only after release-profile validation proves real user value; upstream or delete local terminal deltas that become Rio-owned |

The fork status, feature ledger, and verification evidence live in
[`docs/yazelix/fork_feature_verification.md`](docs/yazelix/fork_feature_verification.md).
Build-speed guidance lives in
[`docs/yazelix/build_speed_workflow.md`](docs/yazelix/build_speed_workflow.md).
Cachix setup lives in [`docs/yazelix/cachix.md`](docs/yazelix/cachix.md).
Yazelix Terminal shader/profile ownership lives in
[`docs/yazelix/shader_profile_ownership.md`](docs/yazelix/shader_profile_ownership.md).
The yzxterm shader ABI lives in
[`docs/yazelix/shader_abi.md`](docs/yazelix/shader_abi.md).
Main Yazelix fork policy lives in
[Fork and child-repo maintenance](https://github.com/luccahuguet/yazelix/blob/main/docs/contracts/fork_child_repo_maintenance.md).

For visual source edits, run the cargo-built terminal with the Yazelix config
shape through `tools/yazelix_terminal_local.sh` before paying for a Nix package
or Home Manager rebuild.

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
- `share/yazelix-terminal/profiles/shaders/config.toml`
- `share/yazelix-terminal/emoji/twitter/config.toml`
- `share/yazelix-terminal/emoji/serenityos/config.toml`
- `share/yazelix-terminal/package-metadata.json`

The desktop wrapper sets the standalone app id to `yazelix-terminal`, or uses
`YAZELIX_TERMINAL_APP_ID` when a parent runtime needs the window to match its
own desktop entry. It searches for available Nix graphics wrappers and maps
Yazelix-owned config directories into Rio's supported `RIO_CONFIG_HOME`
contract only for the terminal process. It ignores ambient host
`RIO_CONFIG_HOME`; use `YAZELIX_TERMINAL_CONFIG` for an explicit Yazelix
Terminal config override. Child shells launched by the packaged wrapper do not
inherit Yazelix Terminal's private `RIO_CONFIG_HOME` or package loader paths,
so plain host `rio` invocations keep using the user's host Rio defaults.
The packaged config disables confirm-before-quit, disables native window
decorations, sets the terminal font size to `17.0`, and uses the default event
renderer strategy with WebGPU, Rio's native trail cursor effect, and a readable
650 ms cursor blink. It also maps private-use icon glyphs to
`Symbols Nerd Font Mono`, selected text-style status symbols to packaged
`Noto Sans Symbols 2`, and common emoji/status symbol ranges to the selected
packaged emoji fallback. `noto` remains the default, while `twitter` and
`serenityos` provide free alternate emoji font presets for visual dogfooding.
`YAZELIX_TERMINAL_PROFILE=baseline` selects the same packaged font, window, and
WebGPU baseline without custom shaders or trail cursor effects for performance
comparisons, while keeping the same cursor blink policy.
`YAZELIX_TERMINAL_PROFILE=shaders` selects the packaged Ghostty-compatible
cursor shader stack for compatibility and visual diagnostics. Shader cursor
visibility follows the renderer's effective blink state, so blink-off frames do
not keep a stale shader glow alive.
`YAZELIX_TERMINAL_EMOJI_FONT=twitter` or `serenityos` selects the matching
packaged profile root before launch. Invalid values fail clearly.
`YAZELIX_TERMINAL_APPEARANCE=light` or `auto` selects the packaged light or
adaptive dark/light pair. The default remains `dark` for continuity.
Yazelix Terminal owns these profile assets and Rio-aware shader details; main
Yazelix should select stable profile names rather than generate or inspect
Rio-specific shader config.
`YAZELIX_TERMINAL_RENDER_STRATEGY=game` is kept as an explicit diagnostic
override and composes with each profile.
Package metadata for main Yazelix and other consumers is documented in
[`docs/yazelix/package_metadata.md`](docs/yazelix/package_metadata.md).

Wrapper override knobs:

| Variable | Behavior |
| --- | --- |
| `YAZELIX_TERMINAL_CONFIG` | Uses a custom Rio config directory; must contain readable `config.toml` |
| `YAZELIX_TERMINAL_APP_ID` | Sets the Wayland app id / X11 class used by the wrapper; defaults to `yazelix-terminal` |
| `YAZELIX_TERMINAL_PROFILE=full` | Uses the packaged WebGPU + Rio trail defaults |
| `YAZELIX_TERMINAL_PROFILE=baseline` | Uses the packaged no-effects baseline config |
| `YAZELIX_TERMINAL_PROFILE=shaders` | Uses the opt-in Ghostty-compatible shader profile |
| `YAZELIX_TERMINAL_EFFECTS=none` | Alias for the baseline no-effects profile |
| `YAZELIX_TERMINAL_EMOJI_FONT=noto` | Uses the default Noto Color Emoji fallback |
| `YAZELIX_TERMINAL_EMOJI_FONT=twitter` | Uses the packaged Twitter/Twemoji color emoji fallback |
| `YAZELIX_TERMINAL_EMOJI_FONT=serenityos` | Uses the packaged SerenityOS emoji fallback |
| `YAZELIX_TERMINAL_APPEARANCE=dark` | Forces the packaged dark palette; this is the default |
| `YAZELIX_TERMINAL_APPEARANCE=light` | Forces the packaged light palette |
| `YAZELIX_TERMINAL_APPEARANCE=auto` | Uses the packaged adaptive dark/light pair and follows the platform theme |
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
unscrolling, clickable plain URLs, OSC 8 hyperlinks, and DECCARA coverage are
tracked in the verification ledger.

Plain URLs and OSC 8 hyperlinks open with Ctrl-click on Linux and Windows, or
Command-click on macOS. The terminal swallows the matching mouse press only
when it starts on an activatable link, so normal selection and mouse reporting
stay unchanged outside that gesture

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
nix run .#yazelix-protocol-conformance -- verify
```

The protocol conformance harness is an isolated Rust tool. The flake app is the
stable command surface; Cargo is useful while editing the harness directly:

```sh
cargo run --manifest-path tools/yazelix_protocol_conformance/Cargo.toml -- verify
```

Focused graphics checks:

```sh
nix develop -c cargo test -p rio-backend --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' kitty_virtual -- --nocapture
nix develop -c cargo test -p sugarloaf --features 'rio-window/x11 rio-window/wayland rio-window/wayland-dlopen' image_shaders_use_origin_size_source_rect -- --nocapture
```

Cursor animation architecture is documented in
[`docs/yazelix/cursor_animation_architecture.md`](docs/yazelix/cursor_animation_architecture.md).

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

# Non-Nix Graphics Launch Support

Review date: 2026-06-22

Mars should stay capable of normal non-Nix packaging. The clean Rio rebuild does
not make Mars a Nix-only terminal. It does, however, keep public support claims
behind validation so packaging work does not hide renderer, launch, or driver
bugs.

## Inputs Checked

- Mars package wrapper: `pkgMars.nix`
- Mars flake package surface: `flake.nix`
- Mars desktop entry: `misc/mars.desktop`
- Rio reference release machinery: `/home/lucca/pjs/yazelix-related/rio/.goreleaser.yaml`
- Rio source build docs: https://rioterm.com/docs/install/build-from-source
- Rio Linux install docs: https://rioterm.com/docs/install/linux
- Rio latest release checked with GitHub CLI: `v0.4.7`, published 2026-06-09
- Ghostty Linux packaging docs: https://ghostty.org/docs/install/binary
- WezTerm Linux packaging docs: https://wezterm.org/install/linux.html

The Rio `v0.4.7` release publishes Linux `.deb` and `.rpm` artifacts for
x86_64/aarch64 and X11/Wayland, plus macOS DMG, Windows MSI and portable
binaries, and checksums. Rio's public Linux docs also name distro packages and
Flathub. Mars should preserve the ability to revalidate those classes without
claiming them before the clean gate passes.

## Support Tiers

### Maintained Now

- Nix package wrapper via `.#mars` and `.#default`
- Mars desktop metadata and icon produced by the Nix wrapper
- Private Yazelix dogfooding through `tools/mars_private_yazelix.py`
- Desktop/process lifetime breadcrumbs through `mars-launch-trace`
- Raw/source launch diagnostics that identify host Vulkan loader failures

The Linux Nix wrapper may set package-owned Vulkan loader defaults when the
environment has no explicit `VK_ICD_FILENAMES`. That default belongs only to the
Nix package boundary.

### Intended But Validation-Gated

- Source-built Mars launched directly from the checkout
- Rio-style Linux `.deb` and `.rpm` artifacts
- Rio-style macOS DMG packaging
- Rio-style Windows MSI and portable binaries
- Distro packaging and Flathub-style distribution
- AppImage or other portable Linux packaging, if a future package owner wants it

These paths should remain possible. They are not public Mars support promises
until a bead validates the concrete package artifact, graphics/runtime policy,
desktop integration, and smoke behavior.

### Out Of Scope For This Bead

- Replacing the Nix wrapper fix
- Adding host-specific ICD search logic to Rio-owned renderer code
- Bundling proprietary GPU drivers
- Guessing distro GPU policy in Mars runtime code
- Readding Mars to Yazelix main, Home Manager, or public Yazelix docs

## Non-Nix Linux Diagnostic Flow

Use this flow when a non-Nix Mars launch fails before the first window, panics
around `vkCreateInstance`, or reports `VK_ERROR_INCOMPATIBLE_DRIVER`.

1. Check whether the host Vulkan loader can see a usable driver outside Mars:

   ```sh
   vulkaninfo --summary
   ```

   If this fails with the same class of loader or ICD error, treat the problem
   as host graphics runtime setup first, not as a Mars renderer bug.

2. Capture loader diagnostics without changing Mars code:

   ```sh
   VK_LOADER_DEBUG=error,warn vulkaninfo --summary
   VK_LOADER_DEBUG=error,warn mars -e true
   ```

3. Check whether the shell is forcing an ICD selection:

   ```sh
   env | grep '^VK_ICD_FILENAMES='
   ```

   A non-empty value is a user or package override. Generic non-Nix docs must
   not recommend copying package-specific ICD paths from a Nix wrapper.

4. Compare against upstream Rio when possible:

   ```sh
   rio
   ```

   If Rio and Mars fail the same way on the same host, keep the investigation
   upstream-shaped or package-shaped. If Rio launches but Mars fails, inspect
   Mars identity, wrapper, config root, and environment isolation before editing
   renderer code.

5. If `vulkaninfo --summary` succeeds and Mars still fails, capture:

   - Mars command line
   - `WAYLAND_DISPLAY` or `DISPLAY`
   - `VK_ICD_FILENAMES`, if set
   - `VK_LOADER_DEBUG=error,warn` output
   - whether upstream Rio succeeds in the same shell
   - whether the launch came from source, `.deb`, `.rpm`, Flatpak, AppImage, or
     another package boundary

## Packaging Roadmap

1. Keep the Nix package wrapper and private dogfooding path working.
2. Validate source/raw Linux launch with a host Vulkan loader that already
   passes `vulkaninfo --summary`.
3. Restore Mars Linux `.deb` and `.rpm` release artifacts from Rio's GoReleaser
   shape, with Mars identity and smoke tests.
4. Evaluate Flatpak separately because sandbox graphics, host GPU access, and
   desktop integration are package-specific.
5. Evaluate AppImage or other portable Linux packaging separately.
6. Audit macOS and Windows release artifacts after Linux dogfooding is stable.
7. Readd Mars to Yazelix main only after the clean Rio gate and reentry bead say
   it is ready as an experimental opt-in runtime.

Each package path needs its own bead. Passing one package path does not imply
support for the others.

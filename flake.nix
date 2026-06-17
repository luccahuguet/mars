{
  description = "Mars Terminal | A Rio-derived GPU terminal emulator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    systems.url = "github:nix-systems/default";
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [flake-parts.flakeModules.easyOverlay];

      systems = import inputs.systems;

      perSystem = {
        self',
        inputs',
        pkgs,
        system,
        lib,
        ...
      }: let
        toolchains = rec {
          msrv = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          stable = pkgs.rust-bin.stable.latest.minimal;
          nightly = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.minimal);
          rio = msrv;
          default = rio;
        };
        unwrappedPackageFor = rust-toolchain:
          pkgs.callPackage ./pkgRioUnwrapped.nix {inherit rust-toolchain;};
        uncheckedUnwrappedPackageFor = rust-toolchain:
          pkgs.callPackage ./pkgRioUnwrapped.nix {
            inherit rust-toolchain;
            doCheck = false;
          };
        fastUnwrappedPackageFor = rust-toolchain:
          pkgs.callPackage ./pkgRioUnwrapped.nix {
            inherit rust-toolchain;
            pname = "yazelix-terminal-fast-unwrapped";
            buildType = "fast";
            doCheck = false;
          };
        packageFor = unwrapped:
          pkgs.callPackage ./pkgRio.nix {
            inherit unwrapped;
            packageProfile = "release";
            packageChecked = true;
          };
        fastPackageFor = unwrapped:
          pkgs.callPackage ./pkgRio.nix {
            inherit unwrapped;
            pname = "yazelix-terminal-fast";
            packageProfile = "fast";
            packageChecked = false;
          };
        defaultUnwrappedPackage = unwrappedPackageFor toolchains.default;
        msrvUnwrappedPackage = unwrappedPackageFor toolchains.msrv;
        stableUnwrappedPackage = unwrappedPackageFor toolchains.stable;
        nightlyUnwrappedPackage = unwrappedPackageFor toolchains.nightly;
        fastUnwrappedPackage = fastUnwrappedPackageFor toolchains.default;
        defaultPackage = packageFor defaultUnwrappedPackage;
        msrvPackage = packageFor msrvUnwrappedPackage;
        stablePackage = packageFor stableUnwrappedPackage;
        nightlyPackage = packageFor nightlyUnwrappedPackage;
        fastPackage = fastPackageFor fastUnwrappedPackage;
        appFor = package: {
          type = "app";
          program = "${package}/bin/yazelix-terminal";
        };
        protocolConformanceTool = pkgs.rustPlatform.buildRustPackage {
          pname = "yazelix-protocol-conformance";
          version = "0.1.0";
          src = ./tools/yazelix_protocol_conformance;
          cargoLock.lockFile = ./tools/yazelix_protocol_conformance/Cargo.lock;
          doCheck = false;
        };
        toolAppFor = package: {
          type = "app";
          program = "${package}/bin/yazelix-protocol-conformance";
        };
        # Defines a devshell using the `rust-toolchain`, allowing for
        # different versions of rust to be used.
        mkDevShell = rust-toolchain: let
          unwrapped = unwrappedPackageFor rust-toolchain;
          runtimeDeps = unwrapped.runtimeDependencies;
          tools =
            unwrapped.nativeBuildInputs
            ++ unwrapped.buildInputs
            ++ [rust-toolchain];
        in
          pkgs.mkShell {
            packages = [self'.formatter] ++ tools;
            LD_LIBRARY_PATH = "${lib.makeLibraryPath runtimeDeps}";
          };
      in {
        formatter = pkgs.alejandra;
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [(import inputs.rust-overlay)];
        };

        overlayAttrs = {
          yazelix-terminal = self'.packages."yazelix-terminal";
          yazelix-terminal-unwrapped = self'.packages."yazelix-terminal-unwrapped";
          yazelix-terminal-fast = self'.packages."yazelix-terminal-fast";
          rio = self'.packages."yazelix-terminal";
          rio-unwrapped = self'.packages."yazelix-terminal-unwrapped";
          rio-fast = self'.packages."yazelix-terminal-fast";
        };
        packages = {
          default = defaultPackage;
          yazelix-terminal = defaultPackage;
          yazelix-terminal-unwrapped = defaultUnwrappedPackage;
          yazelix-terminal-fast = fastPackage;
          yazelix-terminal-fast-unwrapped = fastUnwrappedPackage;
          rio = defaultPackage;
          rio-unwrapped = defaultUnwrappedPackage;
          rio-fast = fastPackage;
          rio-fast-unwrapped = fastUnwrappedPackage;
          yazelix-terminal-msrv = msrvPackage;
          yazelix-terminal-msrv-unwrapped = msrvUnwrappedPackage;
          yazelix-terminal-stable = stablePackage;
          yazelix-terminal-stable-unwrapped = stableUnwrappedPackage;
          yazelix-terminal-nightly = nightlyPackage;
          yazelix-terminal-nightly-unwrapped = nightlyUnwrappedPackage;
          rio-msrv = msrvPackage;
          rio-msrv-unwrapped = msrvUnwrappedPackage;
          rio-stable = stablePackage;
          rio-stable-unwrapped = stableUnwrappedPackage;
          rio-nightly = nightlyPackage;
          rio-nightly-unwrapped = nightlyUnwrappedPackage;
          yazelix-protocol-conformance = protocolConformanceTool;
        };
        apps = {
          default = appFor self'.packages."yazelix-terminal";
          yazelix-terminal = appFor self'.packages."yazelix-terminal";
          yazelix-terminal-fast = appFor self'.packages."yazelix-terminal-fast";
          rio = appFor self'.packages.rio;
          rio-fast = appFor self'.packages."rio-fast";
          yazelix-protocol-conformance = toolAppFor protocolConformanceTool;
        };
        checks = {
          package = self'.packages."yazelix-terminal";
          package_layout = pkgs.runCommand "yazelix-terminal-package-layout" {} ''
            package=${self'.packages."yazelix-terminal"}
            config_paths="\
              share/yazelix-terminal/config.toml \
              share/yazelix-terminal/baseline/config.toml \
              share/yazelix-terminal/profiles/shaders/config.toml \
              share/yazelix-terminal/emoji/twitter/config.toml \
              share/yazelix-terminal/emoji/twitter/baseline/config.toml \
              share/yazelix-terminal/emoji/twitter/profiles/shaders/config.toml \
              share/yazelix-terminal/emoji/serenityos/config.toml \
              share/yazelix-terminal/emoji/serenityos/baseline/config.toml \
              share/yazelix-terminal/emoji/serenityos/profiles/shaders/config.toml"
            for path in \
              $config_paths \
              share/yazelix-terminal/fonts/NotoSansSymbols2-Regular.otf \
              share/yazelix-terminal/package-metadata.json
            do
              if [ ! -f "$package/$path" ]; then
                echo "missing package layout file: $path" >&2
                exit 1
              fi
            done
            for path in $config_paths; do
              if ! grep -Eq '^[[:space:]]*confirm-before-quit[[:space:]]*=[[:space:]]*true' "$package/$path"; then
                echo "packaged config must keep confirm-before-quit enabled: $path" >&2
                exit 1
              fi
            done
            touch "$out"
          '';
          conformance = pkgs.runCommand "yazelix-terminal-conformance" {} ''
            cd ${./.}
            ${protocolConformanceTool}/bin/yazelix-protocol-conformance verify
            touch "$out"
          '';
        };
        # Different devshells for different rust versions
        devShells = lib.mapAttrs (_: v: mkDevShell v) toolchains;
      };
    };
}

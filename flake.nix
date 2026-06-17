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
            pname = "mars-fast-unwrapped";
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
            pname = "mars-fast";
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
          program = "${package}/bin/mars";
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
          mars = self'.packages.mars;
          mars-unwrapped = self'.packages."mars-unwrapped";
          mars-fast = self'.packages."mars-fast";
          mars-fast-unwrapped = self'.packages."mars-fast-unwrapped";
        };
        packages = {
          default = defaultPackage;
          mars = defaultPackage;
          mars-unwrapped = defaultUnwrappedPackage;
          mars-fast = fastPackage;
          mars-fast-unwrapped = fastUnwrappedPackage;
          mars-msrv = msrvPackage;
          mars-msrv-unwrapped = msrvUnwrappedPackage;
          mars-stable = stablePackage;
          mars-stable-unwrapped = stableUnwrappedPackage;
          mars-nightly = nightlyPackage;
          mars-nightly-unwrapped = nightlyUnwrappedPackage;
          yazelix-protocol-conformance = protocolConformanceTool;
        };
        apps = {
          default = appFor self'.packages.mars;
          mars = appFor self'.packages.mars;
          mars-fast = appFor self'.packages."mars-fast";
          yazelix-protocol-conformance = toolAppFor protocolConformanceTool;
        };
        checks = {
          package = self'.packages.mars;
          package_layout = pkgs.runCommand "mars-package-layout" {} ''
            package=${self'.packages.mars}
            for path in bin/mars bin/mars-desktop; do
              if [ ! -x "$package/$path" ]; then
                echo "missing executable package layout file: $path" >&2
                exit 1
              fi
            done
            for stale_path in bin/rio bin/yazelix-terminal bin/yazelix-terminal-desktop; do
              if [ -e "$package/$stale_path" ]; then
                echo "stale package layout file still exists: $stale_path" >&2
                exit 1
              fi
            done
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
          conformance = pkgs.runCommand "mars-conformance" {} ''
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

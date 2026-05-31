{
  description = "Yazelix Terminal | A Rio-derived GPU terminal emulator";

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
        packageFor = unwrapped:
          pkgs.callPackage ./pkgRio.nix {inherit unwrapped;};
        defaultUnwrappedPackage = unwrappedPackageFor toolchains.default;
        msrvUnwrappedPackage = unwrappedPackageFor toolchains.msrv;
        stableUnwrappedPackage = unwrappedPackageFor toolchains.stable;
        nightlyUnwrappedPackage = unwrappedPackageFor toolchains.nightly;
        defaultPackage = packageFor defaultUnwrappedPackage;
        msrvPackage = packageFor msrvUnwrappedPackage;
        stablePackage = packageFor stableUnwrappedPackage;
        nightlyPackage = packageFor nightlyUnwrappedPackage;
        appFor = package: {
          type = "app";
          program = "${package}/bin/yazelix-terminal";
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
          rio = self'.packages."yazelix-terminal";
          rio-unwrapped = self'.packages."yazelix-terminal-unwrapped";
        };
        packages = {
          default = defaultPackage;
          yazelix-terminal = defaultPackage;
          yazelix-terminal-unwrapped = defaultUnwrappedPackage;
          rio = defaultPackage;
          rio-unwrapped = defaultUnwrappedPackage;
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
        };
        apps = {
          default = appFor self'.packages."yazelix-terminal";
          yazelix-terminal = appFor self'.packages."yazelix-terminal";
          rio = appFor self'.packages.rio;
        };
        checks = {
          package = self'.packages."yazelix-terminal";
          conformance = pkgs.runCommand "yazelix-terminal-conformance" {nativeBuildInputs = [pkgs.python3];} ''
            cd ${./.}
            python3 tools/yazelix_conformance.py verify
            touch "$out"
          '';
        };
        # Different devshells for different rust versions
        devShells = lib.mapAttrs (_: v: mkDevShell v) toolchains;
      };
    };
}

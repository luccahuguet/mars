{
  description = "Mars Terminal | A maintainable Rio-derived terminal fork";

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
        # Defines a devshell using the `rust-toolchain`, allowing for
        # different versions of rust to be used.
        mkDevShell = rust-toolchain: let
          runtimeDeps = self'.packages.rio.runtimeDependencies;
          tools =
            self'.packages.rio.nativeBuildInputs ++ self'.packages.rio.buildInputs ++ [rust-toolchain];
        in
          pkgs.mkShell {
            packages = [self'.formatter] ++ tools;
            LD_LIBRARY_PATH = "${lib.makeLibraryPath runtimeDeps}";
          };
        toolchains = rec {
          msrv = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          stable = pkgs.rust-bin.stable.latest.minimal;
          nightly = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.minimal);
          rio = msrv;
          default = rio;
        };
        mkRioPackage = rust-toolchain: pkgs.callPackage ./pkgRio.nix {inherit rust-toolchain;};
        rioPackages =
          lib.mapAttrs' (
            k: v: {
              name =
                if builtins.elem k ["rio" "default"]
                then k
                else "rio-${k}";
              value = mkRioPackage v;
            }
          )
          toolchains;
        marsPackage = pkgs.callPackage ./pkgMars.nix {
          jetbrainsMonoFont = pkgs.jetbrains-mono;
          notoEmojiFont = pkgs.noto-fonts-color-emoji;
          notoFonts = pkgs.noto-fonts;
          rioPackage = rioPackages.rio;
          symbolsNerdFont = pkgs.nerd-fonts.symbols-only;
          twitterEmojiFont = pkgs.twitter-color-emoji;
        };
      in {
        formatter = pkgs.alejandra;
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [(import inputs.rust-overlay)];
        };

        # Keep Rio available while exposing Mars as the fork-owned wrapper.
        overlayAttrs = {inherit (self'.packages) rio mars;};
        packages =
          rioPackages
          // {
            mars = marsPackage;
            default = marsPackage;
          };
        # Different devshells for different rust versions
        devShells = lib.mapAttrs (_: v: mkDevShell v) toolchains;
      };
    };
}

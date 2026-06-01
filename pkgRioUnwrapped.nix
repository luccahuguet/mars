{
  # rust-overlay deps
  rust-toolchain,
  makeRustPlatform,
  # Normal deps
  lib,
  stdenv,
  darwin,
  autoPatchelfHook,
  cmake,
  pkg-config,
  gcc-unwrapped,
  fontconfig,
  libGL,
  vulkan-loader,
  libxkbcommon,
  withX11 ? !stdenv.isDarwin,
  libX11,
  libXcursor,
  libXi,
  libXrandr,
  libxcb,
  withWayland ? !stdenv.isDarwin,
  wayland,
  shaderc,
  doCheck ? true,
  ...
}: let
  readTOML = f: builtins.fromTOML (builtins.readFile f);
  cargoToml = readTOML ./Cargo.toml;
  rioToml = readTOML ./frontends/rioterm/Cargo.toml;
  rustPlatform = makeRustPlatform {
    cargo = rust-toolchain;
    rustc = rust-toolchain;
  };
  rlinkLibs =
    lib.optionals stdenv.isLinux [
      (lib.getLib gcc-unwrapped)
      fontconfig
      libGL
      libxkbcommon
      vulkan-loader
    ]
    ++ lib.optionals withX11 [
      libX11
      libXcursor
      libXi
      libXrandr
      libxcb
    ]
    ++ lib.optionals withWayland [
      wayland
    ];
  packageBuildInputs = rlinkLibs ++ (lib.optionals stdenv.isDarwin [darwin.libutil]);
  packageNativeBuildInputs =
    [
      rustPlatform.bindgenHook
      shaderc
    ]
    ++ lib.optionals stdenv.isLinux [
      cmake
      pkg-config
      autoPatchelfHook
    ];

  inherit (lib.fileset) unions toSource;
in
  rustPlatform.buildRustPackage {
    pname = "yazelix-terminal-unwrapped";
    inherit (cargoToml.workspace.package) version;
    src = toSource {
      root = ./.;
      fileset = unions ([
          ./Cargo.lock
          ./Cargo.toml
          ./conformance/shaders/ghostty_cursor_probe.glsl
        ]
        ++ (map (x: ./. + "/${x}") cargoToml.workspace.members));
    };
    cargoLock.lockFile = ./Cargo.lock;

    cargoBuildFlags = "-p rioterm";

    buildInputs = packageBuildInputs;
    nativeBuildInputs = packageNativeBuildInputs;

    buildNoDefaultFeatures = true;
    buildFeatures =
      ["wgpu"]
      ++ (lib.optionals withX11 ["x11"])
      ++ (lib.optionals withWayland ["wayland"]);
    inherit doCheck;
    checkType = "debug";

    passthru = {
      runtimeDependencies = rlinkLibs;
      buildInputs = packageBuildInputs;
      nativeBuildInputs = packageNativeBuildInputs;
    };

    meta = {
      description = rioToml.package.description;
      longDescription = rioToml.package.extended-description;
      homepage = cargoToml.workspace.package.homepage;
      license = lib.licenses.mit;
      platforms = lib.platforms.unix;
      changelog = "https://github.com/raphamorim/rio/blob/master/CHANGELOG.md";
      mainProgram = "rio";
    };
  }

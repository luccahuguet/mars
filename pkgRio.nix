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
  makeWrapper,
  ncurses,
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

  inherit (lib.fileset) unions toSource;
in
  rustPlatform.buildRustPackage {
    pname = "yazelix-terminal";
    inherit (cargoToml.workspace.package) version;
    src = toSource {
      root = ./.;
      fileset = unions ([
          ./Cargo.lock
          ./Cargo.toml
          ./misc # Extra desktop/terminfo files
        ]
        ++ (map (x: ./. + "/${x}") cargoToml.workspace.members));
    };
    cargoLock.lockFile = ./Cargo.lock;

    cargoBuildFlags = "-p rioterm";

    buildInputs = rlinkLibs ++ (lib.optionals stdenv.isDarwin [darwin.libutil]);
    runtimeDependencies = rlinkLibs;

    nativeBuildInputs =
      [
        rustPlatform.bindgenHook
        makeWrapper
        ncurses
        shaderc
      ]
      ++ lib.optionals stdenv.isLinux [
        cmake
        pkg-config
        autoPatchelfHook
      ];

    outputs = [
      "out"
      "terminfo"
    ];

    postInstall =
      ''
        install -D -m 644 misc/logo.svg \
                          $out/share/icons/hicolor/scalable/apps/rio.svg
        install -D -m 644 misc/logo.svg \
                          $out/share/icons/hicolor/scalable/apps/yazelix-terminal.svg

        wrapProgram "$out/bin/rio" \
          --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath rlinkLibs}"
        ln -s "$out/bin/rio" "$out/bin/yazelix-terminal"

        install -dm 755 "$out/share/applications"
        substitute misc/rio.desktop "$out/share/applications/yazelix-terminal.desktop" \
          --replace-fail "TryExec=rio" "TryExec=$out/bin/yazelix-terminal" \
          --replace-fail "Exec=rio" "Exec=$out/bin/yazelix-terminal" \
          --replace-fail "Icon=rio" "Icon=yazelix-terminal" \
          --replace-fail "Name=Rio" "Name=Yazelix Terminal" \
          --replace-fail "StartupWMClass=Rio" "StartupWMClass=rio"

        # Install terminfo files
        install -dm 755 "$terminfo/share/terminfo/r/"
        tic -xe xterm-rio,rio,rio-direct -o "$terminfo/share/terminfo" misc/rio.terminfo
        mkdir -p $out/nix-support
        echo "$terminfo" >> $out/nix-support/propagated-user-env-packages
      ''
      + lib.optionalString stdenv.hostPlatform.isDarwin ''
        mkdir $out/Applications/
        mv misc/osx/Rio.app/ $out/Applications/
        mkdir $out/Applications/Rio.app/Contents/MacOS/
        ln -s $out/bin/rio $out/Applications/Rio.app/Contents/MacOS/
      '';

    buildNoDefaultFeatures = true;
    buildFeatures = (lib.optionals withX11 ["x11"]) ++ (lib.optionals withWayland ["wayland"]);
    checkType = "debug";
    meta = {
      description = rioToml.package.description;
      longDescription = rioToml.package.extended-description;
      homepage = cargoToml.workspace.package.homepage;
      license = lib.licenses.mit;
      platforms = lib.platforms.unix;
      changelog = "https://github.com/raphamorim/rio/blob/master/CHANGELOG.md";
      mainProgram = "yazelix-terminal";
    };
  }

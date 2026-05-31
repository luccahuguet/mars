{
  lib,
  stdenv,
  makeWrapper,
  ncurses,
  unwrapped,
  ...
}: let
  readTOML = f: builtins.fromTOML (builtins.readFile f);
  cargoToml = readTOML ./Cargo.toml;
  rioToml = readTOML ./frontends/rioterm/Cargo.toml;
  rlinkLibs = unwrapped.runtimeDependencies or [];

  inherit (lib.fileset) unions toSource;
in
  stdenv.mkDerivation {
    pname = "yazelix-terminal";
    inherit (cargoToml.workspace.package) version;
    src = toSource {
      root = ./.;
      fileset = unions [
        ./misc
      ];
    };

    nativeBuildInputs = [
      makeWrapper
      ncurses
    ];

    outputs = [
      "out"
      "terminfo"
    ];

    dontConfigure = true;
    dontBuild = true;

    installPhase =
      ''
        runHook preInstall

        install -D -m 644 misc/logo.svg \
                          $out/share/icons/hicolor/scalable/apps/rio.svg
        install -D -m 644 misc/logo.svg \
                          $out/share/icons/hicolor/scalable/apps/yazelix-terminal.svg

        makeWrapper "${unwrapped}/bin/rio" "$out/bin/rio" \
          --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath rlinkLibs}"
        ln -s "$out/bin/rio" "$out/bin/yazelix-terminal"
        substitute misc/yazelix_terminal_desktop.sh "$out/bin/yazelix-terminal-desktop" \
          --replace-fail "@yazelix_terminal_binary@" "$out/bin/yazelix-terminal"
        chmod 755 "$out/bin/yazelix-terminal-desktop"

        install -dm 755 "$out/share/applications"
        substitute misc/rio.desktop "$out/share/applications/yazelix-terminal.desktop" \
          --replace-fail "TryExec=rio" "TryExec=$out/bin/yazelix-terminal-desktop" \
          --replace-fail "Exec=rio" "Exec=$out/bin/yazelix-terminal-desktop" \
          --replace-fail "Icon=rio" "Icon=yazelix-terminal" \
          --replace-fail "Name=Rio" "Name=Yazelix Terminal" \
          --replace-fail "StartupWMClass=Rio" "StartupWMClass=yazelix-terminal"$'\n'"StartupNotify=true"

        # Install terminfo files
        install -dm 755 "$terminfo/share/terminfo/r/"
        tic -xe xterm-rio,rio,rio-direct -o "$terminfo/share/terminfo" misc/rio.terminfo
        mkdir -p $out/nix-support
        echo "$terminfo" >> $out/nix-support/propagated-user-env-packages

        runHook postInstall
      ''
      + lib.optionalString stdenv.hostPlatform.isDarwin ''
        mkdir $out/Applications/
        mv misc/osx/Rio.app/ $out/Applications/
        mkdir $out/Applications/Rio.app/Contents/MacOS/
        ln -s ${unwrapped}/bin/rio $out/Applications/Rio.app/Contents/MacOS/
      '';

    passthru = {
      inherit unwrapped;
      runtimeDependencies = rlinkLibs;
      buildInputs = unwrapped.buildInputs or [];
      nativeBuildInputs = unwrapped.nativeBuildInputs or [];
    };

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

{
  imagemagick,
  lib,
  makeWrapper,
  python3,
  rioPackage,
  symlinkJoin,
}:

symlinkJoin {
  name = "mars";
  paths = [rioPackage];
  nativeBuildInputs = [
    imagemagick
    makeWrapper
    python3
  ];

  postBuild = ''
    rm -f "$out/bin/rio"
    rm -f "$out/share/applications/rio.desktop"
    rm -f "$out/share/icons/hicolor/scalable/apps/rio.svg"

    makeWrapper "${rioPackage}/bin/rio" "$out/bin/mars" \
      --add-flags "--app-id mars"
    install -D -m 755 "${./tools/mars_launch_trace.py}" "$out/bin/mars-launch-trace"
    patchShebangs "$out/bin/mars-launch-trace"

    install -D -m 644 "${./misc/mars.desktop}" \
      "$out/share/applications/mars.desktop"
    substituteInPlace "$out/share/applications/mars.desktop" \
      --replace-fail "TryExec=mars" "TryExec=$out/bin/mars" \
      --replace-fail "Exec=mars" "Exec=$out/bin/mars"

    for size in 16 24 32 48 64 128 256 512 1024; do
      install -d "$out/share/icons/hicolor/''${size}x''${size}/apps"
      magick "${./misc/mars_terminal_icon.png}" -resize "''${size}x''${size}" \
        "$out/share/icons/hicolor/''${size}x''${size}/apps/mars.png"
    done
    install -D -m 644 "$out/share/icons/hicolor/512x512/apps/mars.png" \
      "$out/share/pixmaps/mars.png"
  '';

  passthru =
    (rioPackage.passthru or {})
    // {
      unwrappedRioPackage = rioPackage;
      runtimeDependencies = rioPackage.runtimeDependencies or [];
      nativeBuildInputs = rioPackage.nativeBuildInputs or [];
      buildInputs = rioPackage.buildInputs or [];
    };

  meta =
    (rioPackage.meta or {})
    // {
      description = "Mars Terminal, a maintainable Rio-derived terminal fork";
      homepage = "https://github.com/luccahuguet/mars";
      mainProgram = "mars";
      longDescription = ''
        Mars Terminal is currently a minimal wrapper around upstream Rio.
        Fork-specific terminal behavior must stay out until the clean Rio
        baseline passes Yazelix dogfooding gates.
      '';
      license = lib.licenses.mit;
    };
}

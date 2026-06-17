{
  lib,
  stdenv,
  makeWrapper,
  imagemagick,
  ncurses,
  noto-fonts,
  noto-fonts-color-emoji,
  serenityos-emoji-font,
  twitter-color-emoji,
  unwrapped,
  pname ? "mars",
  packageProfile ? "release",
  packageChecked ? true,
  ...
}: let
  readTOML = f: builtins.fromTOML (builtins.readFile f);
  cargoToml = readTOML ./Cargo.toml;
  rioToml = readTOML ./frontends/rioterm/Cargo.toml;
  rlinkLibs = unwrapped.runtimeDependencies or [];
  configRoots = {
    full = "share/mars";
    baseline = "share/mars/baseline";
    shaders = "share/mars/profiles/shaders";
  };
  emojiConfigRootsFor = emojiFont: {
    full = "share/mars/emoji/${emojiFont}";
    baseline = "share/mars/emoji/${emojiFont}/baseline";
    shaders = "share/mars/emoji/${emojiFont}/profiles/shaders";
  };
  supportedEmojiFonts = [
    "noto"
    "twitter"
    "serenityos"
  ];
  emojiFontPresets = {
    noto = {
      family = "Noto Color Emoji";
      fontDir = "${noto-fonts-color-emoji}/share/fonts";
      configRoots = configRoots;
    };
    twitter = {
      family = "Twitter Color Emoji";
      fontDir = "${twitter-color-emoji}/share/fonts";
      configRoots = emojiConfigRootsFor "twitter";
    };
    serenityos = {
      family = "SerenityOS Emoji";
      fontDir = "${serenityos-emoji-font}/share/fonts";
      configRoots = emojiConfigRootsFor "serenityos";
    };
  };
  emojiFontMetadata =
    lib.genAttrs supportedEmojiFonts
    (name: {
      family = emojiFontPresets.${name}.family;
      config_roots = emojiFontPresets.${name}.configRoots;
    });
  marsPackageMetadata = {
    schema_version = 1;
    terminal = "mars";
    package_name = pname;
    package_profile = packageProfile;
    checked_package = packageChecked;
    metadata_path = "share/mars/package-metadata.json";
    supported_profiles = [
      "full"
      "baseline"
      "shaders"
    ];
    default_profile = "full";
    baseline_profile = "baseline";
    shader_profile = "shaders";
    supported_appearance_modes = [
      "dark"
      "light"
      "auto"
    ];
    default_appearance_mode = "dark";
    supported_emoji_fonts = supportedEmojiFonts;
    default_emoji_font = "noto";
    emoji_fonts = emojiFontMetadata;
    shader_asset_root = "share/mars/shaders";
    config_roots = configRoots;
    wrapper_commands = {
      terminal = "bin/mars";
      desktop = "bin/mars-desktop";
    };
    wrapper_env = {
      profile = "MARS_PROFILE";
      effects = "MARS_EFFECTS";
      config = "MARS_CONFIG";
      app_id = "MARS_APP_ID";
      render_strategy = "MARS_RENDER_STRATEGY";
      graphics_wrapper = "MARS_GRAPHICS_WRAPPER";
      appearance = "MARS_APPEARANCE";
      emoji_font = "MARS_EMOJI_FONT";
    };
    main_yazelix_boundary = "Select package/profile by metadata; do not parse Mars config or shader files.";
  };

  inherit (lib.fileset) unions toSource;
in
  stdenv.mkDerivation {
    inherit pname;
    inherit (cargoToml.workspace.package) version;
    src = toSource {
      root = ./.;
      fileset = unions [
        ./misc
        ./sugarloaf/src/font/resources/SymbolsNerdFontMono/SymbolsNerdFontMono-Regular.ttf
      ];
    };

    nativeBuildInputs = [
      imagemagick
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

        for size in 16 32 48 64 128 256 512 1024; do
          icon_dir="$out/share/icons/hicolor/''${size}x''${size}/apps"
          install -dm 755 "$icon_dir"
          if [ "$size" = 1024 ]; then
            install -m 644 misc/mars_terminal_icon.png "$icon_dir/mars.png"
          else
            magick misc/mars_terminal_icon.png -resize "''${size}x''${size}" "$icon_dir/mars.png"
          fi
        done
        install -D -m 644 sugarloaf/src/font/resources/SymbolsNerdFontMono/SymbolsNerdFontMono-Regular.ttf \
                          $out/share/mars/fonts/SymbolsNerdFontMono-Regular.ttf
        install -D -m 644 ${noto-fonts}/share/fonts/noto/NotoSansSymbols2-Regular.otf \
                          $out/share/mars/fonts/NotoSansSymbols2-Regular.otf
        install -dm 755 $out/share/mars/shaders/generated_effects
        install -m 644 misc/mars_shaders/cursor_trail_dusk.glsl \
                         $out/share/mars/shaders/cursor_trail_dusk.glsl
        install -m 644 misc/mars_shaders/generated_effects/*.glsl \
                         $out/share/mars/shaders/generated_effects/

        render_yazelix_config() {
          src="$1"
          dst="$2"
          emoji_font_dir="$3"
          emoji_font_family="$4"
          tmp_with_fonts="$NIX_BUILD_TOP/$(basename "$dst").with-fonts"
          tmp_resolved_fonts="$NIX_BUILD_TOP/$(basename "$dst").resolved-fonts"

          while IFS= read -r line; do
            if [ "$line" = "@mars_fonts@" ]; then
              cat misc/mars_fonts.toml
            else
              printf '%s\n' "$line"
            fi
          done < "$src" > "$tmp_with_fonts"

          substitute "$tmp_with_fonts" "$tmp_resolved_fonts" \
            --replace-fail "@mars_font_dir@" "$out/share/mars/fonts" \
            --replace-fail "@mars_emoji_font_dir@" "$emoji_font_dir" \
            --replace-fail "@mars_emoji_font_family@" "$emoji_font_family"

          if grep -q "@mars_shader_dir@" "$tmp_resolved_fonts"; then
            substitute "$tmp_resolved_fonts" "$dst" \
              --replace-fail "@mars_shader_dir@" "$out/share/mars/shaders"
          else
            install -m 644 "$tmp_resolved_fonts" "$dst"
          fi

          chmod 644 "$dst"
          if grep -q "@mars_" "$dst"; then
            echo "unresolved Mars Terminal config placeholder in $dst" >&2
            exit 1
          fi
        }

        install_yazelix_themes() {
          theme_config_root="$1"

          install -dm 755 "$theme_config_root/themes"
          install -m 644 misc/mars_theme_dark.toml \
                           "$theme_config_root/themes/yazelix-dark.toml"
          install -m 644 misc/mars_theme_light.toml \
                           "$theme_config_root/themes/yazelix-light.toml"
        }

        render_yazelix_profile_set() {
          config_root="$1"
          emoji_font_dir="$2"
          emoji_font_family="$3"

          install -dm 755 "$config_root"
          render_yazelix_config misc/mars_config.toml \
            "$config_root/config.toml" \
            "$emoji_font_dir" \
            "$emoji_font_family"
          install_yazelix_themes "$config_root"
          install -dm 755 "$config_root/baseline"
          render_yazelix_config misc/mars_config_baseline.toml \
            "$config_root/baseline/config.toml" \
            "$emoji_font_dir" \
            "$emoji_font_family"
          install_yazelix_themes "$config_root/baseline"
          install -dm 755 "$config_root/profiles/shaders"
          render_yazelix_config misc/mars_config_shaders.toml \
            "$config_root/profiles/shaders/config.toml" \
            "$emoji_font_dir" \
            "$emoji_font_family"
          install_yazelix_themes "$config_root/profiles/shaders"
        }

        render_yazelix_profile_set "$out/share/mars" \
          "${emojiFontPresets.noto.fontDir}" \
          "${emojiFontPresets.noto.family}"
        render_yazelix_profile_set "$out/share/mars/emoji/twitter" \
          "${emojiFontPresets.twitter.fontDir}" \
          "${emojiFontPresets.twitter.family}"
        render_yazelix_profile_set "$out/share/mars/emoji/serenityos" \
          "${emojiFontPresets.serenityos.fontDir}" \
          "${emojiFontPresets.serenityos.family}"
        printf '%s\n' '${builtins.toJSON marsPackageMetadata}' > "$out/share/mars/package-metadata.json"
        chmod 644 "$out/share/mars/package-metadata.json"

        makeWrapper "${unwrapped}/bin/rio" "$out/bin/mars" \
          --set MARS_CHILD_ENV_SANITIZE 1 \
          --set MARS_LD_LIBRARY_PATH_PREFIX "${lib.makeLibraryPath rlinkLibs}" \
          --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath rlinkLibs}"
        substitute misc/mars_desktop.sh "$out/bin/mars-desktop" \
          --replace-fail "@mars_binary@" "$out/bin/mars" \
          --replace-fail "@mars_config_home@" "$out/share/mars" \
          --replace-fail "@mars_baseline_config_home@" "$out/share/mars/baseline" \
          --replace-fail "@mars_shader_config_home@" "$out/share/mars/profiles/shaders" \
          --replace-fail "@mars_emoji_config_home@" "$out/share/mars/emoji"
        chmod 755 "$out/bin/mars-desktop"

        install -dm 755 "$out/share/applications"
        substitute misc/rio.desktop "$out/share/applications/mars.desktop" \
          --replace-fail "TryExec=rio" "TryExec=$out/bin/mars-desktop" \
          --replace-fail "Exec=rio" "Exec=$out/bin/mars-desktop" \
          --replace-fail "Icon=rio" "Icon=mars" \
          --replace-fail "Name=Rio" "Name=Mars Terminal" \
          --replace-fail "StartupWMClass=Rio" "StartupWMClass=mars"$'\n'"StartupNotify=true"

        # Install terminfo files
        install -dm 755 "$terminfo/share/terminfo/m/" "$terminfo/share/terminfo/r/"
        tic -xe xterm-mars,mars,mars-direct,xterm-rio,rio,rio-direct -o "$terminfo/share/terminfo" misc/rio.terminfo
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
      inherit marsPackageMetadata;
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
      mainProgram = "mars";
    };
  }

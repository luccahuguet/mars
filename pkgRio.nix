{
  lib,
  stdenv,
  makeWrapper,
  ncurses,
  noto-fonts,
  noto-fonts-color-emoji,
  serenityos-emoji-font,
  twitter-color-emoji,
  unwrapped,
  pname ? "yazelix-terminal",
  packageProfile ? "release",
  packageChecked ? true,
  ...
}: let
  readTOML = f: builtins.fromTOML (builtins.readFile f);
  cargoToml = readTOML ./Cargo.toml;
  rioToml = readTOML ./frontends/rioterm/Cargo.toml;
  rlinkLibs = unwrapped.runtimeDependencies or [];
  configRoots = {
    full = "share/yazelix-terminal";
    baseline = "share/yazelix-terminal/baseline";
    shaders = "share/yazelix-terminal/profiles/shaders";
  };
  emojiConfigRootsFor = emojiFont: {
    full = "share/yazelix-terminal/emoji/${emojiFont}";
    baseline = "share/yazelix-terminal/emoji/${emojiFont}/baseline";
    shaders = "share/yazelix-terminal/emoji/${emojiFont}/profiles/shaders";
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
  yzxtermPackageMetadata = {
    schema_version = 1;
    terminal = "yazelix-terminal";
    package_name = pname;
    package_profile = packageProfile;
    checked_package = packageChecked;
    metadata_path = "share/yazelix-terminal/package-metadata.json";
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
    shader_asset_root = "share/yazelix-terminal/shaders";
    config_roots = configRoots;
    wrapper_commands = {
      terminal = "bin/yazelix-terminal";
      desktop = "bin/yazelix-terminal-desktop";
      rio_compat = "bin/rio";
    };
    wrapper_env = {
      profile = "YAZELIX_TERMINAL_PROFILE";
      effects = "YAZELIX_TERMINAL_EFFECTS";
      config = "YAZELIX_TERMINAL_CONFIG";
      app_id = "YAZELIX_TERMINAL_APP_ID";
      render_strategy = "YAZELIX_TERMINAL_RENDER_STRATEGY";
      graphics_wrapper = "YAZELIX_TERMINAL_GRAPHICS_WRAPPER";
      appearance = "YAZELIX_TERMINAL_APPEARANCE";
      emoji_font = "YAZELIX_TERMINAL_EMOJI_FONT";
    };
    main_yazelix_boundary = "Select package/profile by metadata; do not parse yzxterm configs or shader files.";
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
        install -D -m 644 sugarloaf/src/font/resources/SymbolsNerdFontMono/SymbolsNerdFontMono-Regular.ttf \
                          $out/share/yazelix-terminal/fonts/SymbolsNerdFontMono-Regular.ttf
        install -D -m 644 ${noto-fonts}/share/fonts/noto/NotoSansSymbols2-Regular.otf \
                          $out/share/yazelix-terminal/fonts/NotoSansSymbols2-Regular.otf
        install -dm 755 $out/share/yazelix-terminal/shaders/generated_effects
        install -m 644 misc/yazelix_terminal_shaders/cursor_trail_dusk.glsl \
                         $out/share/yazelix-terminal/shaders/cursor_trail_dusk.glsl
        install -m 644 misc/yazelix_terminal_shaders/generated_effects/*.glsl \
                         $out/share/yazelix-terminal/shaders/generated_effects/

        render_yazelix_config() {
          src="$1"
          dst="$2"
          emoji_font_dir="$3"
          emoji_font_family="$4"
          tmp_with_fonts="$NIX_BUILD_TOP/$(basename "$dst").with-fonts"
          tmp_resolved_fonts="$NIX_BUILD_TOP/$(basename "$dst").resolved-fonts"

          while IFS= read -r line; do
            if [ "$line" = "@yazelix_terminal_fonts@" ]; then
              cat misc/yazelix_terminal_fonts.toml
            else
              printf '%s\n' "$line"
            fi
          done < "$src" > "$tmp_with_fonts"

          substitute "$tmp_with_fonts" "$tmp_resolved_fonts" \
            --replace-fail "@yazelix_terminal_font_dir@" "$out/share/yazelix-terminal/fonts" \
            --replace-fail "@yazelix_terminal_emoji_font_dir@" "$emoji_font_dir" \
            --replace-fail "@yazelix_terminal_emoji_font_family@" "$emoji_font_family"

          if grep -q "@yazelix_terminal_shader_dir@" "$tmp_resolved_fonts"; then
            substitute "$tmp_resolved_fonts" "$dst" \
              --replace-fail "@yazelix_terminal_shader_dir@" "$out/share/yazelix-terminal/shaders"
          else
            install -m 644 "$tmp_resolved_fonts" "$dst"
          fi

          chmod 644 "$dst"
          if grep -q "@yazelix_terminal_" "$dst"; then
            echo "unresolved Mars Terminal config placeholder in $dst" >&2
            exit 1
          fi
        }

        install_yazelix_themes() {
          theme_config_root="$1"

          install -dm 755 "$theme_config_root/themes"
          install -m 644 misc/yazelix_terminal_theme_dark.toml \
                           "$theme_config_root/themes/yazelix-dark.toml"
          install -m 644 misc/yazelix_terminal_theme_light.toml \
                           "$theme_config_root/themes/yazelix-light.toml"
        }

        render_yazelix_profile_set() {
          config_root="$1"
          emoji_font_dir="$2"
          emoji_font_family="$3"

          install -dm 755 "$config_root"
          render_yazelix_config misc/yazelix_terminal_config.toml \
            "$config_root/config.toml" \
            "$emoji_font_dir" \
            "$emoji_font_family"
          install_yazelix_themes "$config_root"
          install -dm 755 "$config_root/baseline"
          render_yazelix_config misc/yazelix_terminal_config_baseline.toml \
            "$config_root/baseline/config.toml" \
            "$emoji_font_dir" \
            "$emoji_font_family"
          install_yazelix_themes "$config_root/baseline"
          install -dm 755 "$config_root/profiles/shaders"
          render_yazelix_config misc/yazelix_terminal_config_shaders.toml \
            "$config_root/profiles/shaders/config.toml" \
            "$emoji_font_dir" \
            "$emoji_font_family"
          install_yazelix_themes "$config_root/profiles/shaders"
        }

        render_yazelix_profile_set "$out/share/yazelix-terminal" \
          "${emojiFontPresets.noto.fontDir}" \
          "${emojiFontPresets.noto.family}"
        render_yazelix_profile_set "$out/share/yazelix-terminal/emoji/twitter" \
          "${emojiFontPresets.twitter.fontDir}" \
          "${emojiFontPresets.twitter.family}"
        render_yazelix_profile_set "$out/share/yazelix-terminal/emoji/serenityos" \
          "${emojiFontPresets.serenityos.fontDir}" \
          "${emojiFontPresets.serenityos.family}"
        printf '%s\n' '${builtins.toJSON yzxtermPackageMetadata}' > "$out/share/yazelix-terminal/package-metadata.json"
        chmod 644 "$out/share/yazelix-terminal/package-metadata.json"

        makeWrapper "${unwrapped}/bin/rio" "$out/bin/rio" \
          --set YAZELIX_TERMINAL_CHILD_ENV_SANITIZE 1 \
          --set YAZELIX_TERMINAL_LD_LIBRARY_PATH_PREFIX "${lib.makeLibraryPath rlinkLibs}" \
          --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath rlinkLibs}"
        ln -s "$out/bin/rio" "$out/bin/yazelix-terminal"
        substitute misc/yazelix_terminal_desktop.sh "$out/bin/yazelix-terminal-desktop" \
          --replace-fail "@yazelix_terminal_binary@" "$out/bin/yazelix-terminal" \
          --replace-fail "@yazelix_terminal_config_home@" "$out/share/yazelix-terminal" \
          --replace-fail "@yazelix_terminal_baseline_config_home@" "$out/share/yazelix-terminal/baseline" \
          --replace-fail "@yazelix_terminal_shader_config_home@" "$out/share/yazelix-terminal/profiles/shaders" \
          --replace-fail "@yazelix_terminal_emoji_config_home@" "$out/share/yazelix-terminal/emoji"
        chmod 755 "$out/bin/yazelix-terminal-desktop"

        install -dm 755 "$out/share/applications"
        substitute misc/rio.desktop "$out/share/applications/yazelix-terminal.desktop" \
          --replace-fail "TryExec=rio" "TryExec=$out/bin/yazelix-terminal-desktop" \
          --replace-fail "Exec=rio" "Exec=$out/bin/yazelix-terminal-desktop" \
          --replace-fail "Icon=rio" "Icon=yazelix-terminal" \
          --replace-fail "Name=Rio" "Name=Mars Terminal" \
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
      inherit yzxtermPackageMetadata;
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

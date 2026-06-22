{
  firaCodeNerdFont,
  imagemagick,
  lib,
  makeWrapper,
  mesa,
  notoEmojiFont,
  notoFonts,
  python3,
  rioPackage,
  serenityOsEmojiFont,
  stdenv,
  symlinkJoin,
  symbolsNerdFont,
  twitterEmojiFont,
  writeText,
}: let
  packageProfile = "full";
  supportsSerenityOsEmoji = !stdenv.isDarwin;
  configRoots = {
    full = "share/mars";
    baseline = "share/mars/baseline";
    shaders = "share/mars/profiles/shaders";
  };
  emojiConfigRoots =
    {
      noto = configRoots;
      twitter = {
        full = "share/mars/emoji/twitter";
        baseline = "share/mars/emoji/twitter/baseline";
        shaders = "share/mars/emoji/twitter/profiles/shaders";
      };
    }
    // lib.optionalAttrs supportsSerenityOsEmoji {
      serenityos = {
        full = "share/mars/emoji/serenityos";
        baseline = "share/mars/emoji/serenityos/baseline";
        shaders = "share/mars/emoji/serenityos/profiles/shaders";
      };
    };
  marsPackageMetadata = {
    schema_version = 1;
    terminal = "mars";
    package_name = "mars";
    package_profile = packageProfile;
    checked_package = true;
    metadata_path = "share/mars/package-metadata.json";
    wrapper_commands = {
      desktop = "bin/mars";
      terminal = "bin/mars";
    };
    config_roots = configRoots;
    supported_emoji_fonts =
      [
        "noto"
        "twitter"
      ]
      ++ lib.optional supportsSerenityOsEmoji "serenityos";
    supported_appearance_modes = [
      "dark"
      "light"
      "auto"
    ];
    default_appearance_mode = "dark";
    wrapper_env = {
      appearance = "MARS_APPEARANCE";
      emoji_font = "MARS_EMOJI_FONT";
      emoji_font_source = "MARS_EMOJI_FONT_SOURCE";
      profile = "MARS_PROFILE";
    };
    emoji_fonts = lib.mapAttrs (_: roots: {config_roots = roots;}) emojiConfigRoots;
  };
  packageMetadataJson = builtins.toJSON marsPackageMetadata;
  firaCodeNerdDir = "${firaCodeNerdFont}/share/fonts/truetype/NerdFonts/FiraCode";
  symbolsNerdDir = "${symbolsNerdFont}/share/fonts/truetype/NerdFonts/Symbols";
  notoSymbolsDir = "${notoFonts}/share/fonts/noto";
  notoEmojiDir = "${notoEmojiFont}/share/fonts/noto";
  twitterEmojiDir = "${twitterEmojiFont}/share/fonts/truetype/TwitterColorEmoji";
  serenityOsEmojiDir = "${serenityOsEmojiFont}/share/fonts/truetype";
  marsAnsiColors = ''
    black = "#000000"
    dim-black = "#6f7782"
    red = "#cd0000"
    green = "#00cd00"
    yellow = "#cdcd00"
    blue = "#1093f5"
    magenta = "#cd00cd"
    cyan = "#00cdcd"
    white = "#faebd7"
    light-black = "#8b949e"
    light-red = "#ff0000"
    light-green = "#00ff00"
    light-yellow = "#ffff00"
    light-blue = "#11b5f6"
    light-magenta = "#ff00ff"
    light-cyan = "#00ffff"
    light-white = "#ffffff"
  '';
  darkColors = ''
    background = "#111416"
    foreground = "#eeeeec"
    dim-foreground = "#9d9d9c"
    ${marsAnsiColors}
  '';
  lightColors = ''
    background = "#f5f3ef"
    foreground = "#202124"
    dim-foreground = "#62666d"
    ${marsAnsiColors}
  '';
  configFor = {
    emojiFamily,
    emojiDir,
    trailCursor,
  }: ''
    confirm-before-quit = false
    scrollback-history-limit = 0
    force-theme = "dark"
    enable-scroll-bar = false

    [adaptive-theme]
    dark = "yazelix-dark"
    light = "yazelix-light"

    [bell]
    audio = false
    visual = true

    [effects]
    trail-cursor = ${
      if trailCursor
      then "true"
      else "false"
    }

    [window]
    width = 960
    height = 620
    decorations = "Disabled"
    opacity = 0.78
    opacity-cells = false

    [panel]
    margin = [0.0]
    padding = [0.0]
    border-width = 0.0

    [fonts]
    family = "FiraCode Nerd Font Mono"
    size = 18.0
    additional-dirs = [
      "${firaCodeNerdDir}",
      "${symbolsNerdDir}",
      "${notoSymbolsDir}",
      "${emojiDir}"
    ]
    symbol-map = [
      { start = "E000", end = "F900", font-family = "Symbols Nerd Font Mono" },
      { start = "F0000", end = "F3000", font-family = "Symbols Nerd Font Mono" },
      { start = "1F5B0", end = "1F5C0", font-family = "Noto Sans Symbols2" },
      { start = "2600", end = "276F", font-family = "${emojiFamily}" },
      { start = "1F000", end = "1F5B0", font-family = "${emojiFamily}" },
      { start = "1F5C0", end = "1FB00", font-family = "${emojiFamily}" },
    ]

    [colors]
    ${darkColors}

    [navigation]
    mode = "Plain"
  '';
  darkTheme = ''
    [colors]
    ${darkColors}
    cursor = "#00e6ff"
  '';
  lightTheme = ''
    [colors]
    ${lightColors}
    cursor = "#0077cc"
  '';
in
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

      ${lib.optionalString stdenv.isLinux ''
        mesa_vulkan_icd_files="$(printf '%s:' ${mesa}/share/vulkan/icd.d/*.json)"
        mesa_vulkan_icd_files="''${mesa_vulkan_icd_files%:}"
      ''}
      wrapper_args=(
        --add-flags "--app-id mars"
      )
      ${lib.optionalString stdenv.isLinux ''
        wrapper_args+=(--run 'if [ -z "''${VK_ICD_FILENAMES:-}" ]; then export VK_ICD_FILENAMES='"$mesa_vulkan_icd_files"'; fi')
      ''}
      makeWrapper "${rioPackage}/bin/rio" "$out/bin/mars" "''${wrapper_args[@]}"
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

      install -D -m 644 ${writeText "mars-package-metadata.json" packageMetadataJson} \
        "$out/share/mars/package-metadata.json"

      install_mars_profile() {
        local root="$1"
        local config="$2"
        install -D -m 644 "$config" "$out/$root/config.toml"
        install -D -m 644 ${writeText "yazelix-dark.toml" darkTheme} \
          "$out/$root/themes/yazelix-dark.toml"
        install -D -m 644 ${writeText "yazelix-light.toml" lightTheme} \
          "$out/$root/themes/yazelix-light.toml"
      }

      install_mars_profile "${configRoots.full}" \
        ${writeText "mars-noto-full.toml" (configFor {
        emojiFamily = "Noto Color Emoji";
        emojiDir = notoEmojiDir;
        trailCursor = true;
      })}
      install_mars_profile "${configRoots.baseline}" \
        ${writeText "mars-noto-baseline.toml" (configFor {
        emojiFamily = "Noto Color Emoji";
        emojiDir = notoEmojiDir;
        trailCursor = false;
      })}
      install_mars_profile "${configRoots.shaders}" \
        ${writeText "mars-noto-shaders.toml" (configFor {
        emojiFamily = "Noto Color Emoji";
        emojiDir = notoEmojiDir;
        trailCursor = true;
      })}

      install_mars_profile "${emojiConfigRoots.twitter.full}" \
        ${writeText "mars-twitter-full.toml" (configFor {
        emojiFamily = "Twitter Color Emoji";
        emojiDir = twitterEmojiDir;
        trailCursor = true;
      })}
      install_mars_profile "${emojiConfigRoots.twitter.baseline}" \
        ${writeText "mars-twitter-baseline.toml" (configFor {
        emojiFamily = "Twitter Color Emoji";
        emojiDir = twitterEmojiDir;
        trailCursor = false;
      })}
      install_mars_profile "${emojiConfigRoots.twitter.shaders}" \
        ${writeText "mars-twitter-shaders.toml" (configFor {
        emojiFamily = "Twitter Color Emoji";
        emojiDir = twitterEmojiDir;
        trailCursor = true;
      })}

      ${lib.optionalString supportsSerenityOsEmoji ''
        install_mars_profile "${emojiConfigRoots.serenityos.full}" \
          ${writeText "mars-serenityos-full.toml" (configFor {
          emojiFamily = "SerenityOS Emoji";
          emojiDir = serenityOsEmojiDir;
          trailCursor = true;
        })}
        install_mars_profile "${emojiConfigRoots.serenityos.baseline}" \
          ${writeText "mars-serenityos-baseline.toml" (configFor {
          emojiFamily = "SerenityOS Emoji";
          emojiDir = serenityOsEmojiDir;
          trailCursor = false;
        })}
        install_mars_profile "${emojiConfigRoots.serenityos.shaders}" \
          ${writeText "mars-serenityos-shaders.toml" (configFor {
          emojiFamily = "SerenityOS Emoji";
          emojiDir = serenityOsEmojiDir;
          trailCursor = true;
        })}
      ''}
    '';

    passthru =
      (rioPackage.passthru or {})
      // {
        inherit marsPackageMetadata;
        unwrappedRioPackage = rioPackage;
        runtimeDependencies =
          (rioPackage.runtimeDependencies or [])
          ++ lib.optionals stdenv.isLinux [mesa];
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

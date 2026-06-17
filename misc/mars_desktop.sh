#!/usr/bin/env sh
set -eu

binary="@mars_binary@"
default_config_home="@mars_config_home@"
baseline_config_home="@mars_baseline_config_home@"
shader_config_home="@mars_shader_config_home@"
emoji_config_home="@mars_emoji_config_home@"

is_executable() {
  [ -n "$1" ] && [ -x "$1" ]
}

print_first_executable() {
  for candidate in "$@"; do
    if is_executable "$candidate"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

print_first_command() {
  for command_name in "$@"; do
    if command_path="$(command -v "$command_name" 2>/dev/null)"; then
      printf '%s\n' "$command_path"
      return 0
    fi
  done
  return 1
}

print_executable_or_command() {
  if is_executable "$1"; then
    printf '%s\n' "$1"
    return 0
  fi

  if command_path="$(command -v "$1" 2>/dev/null)"; then
    printf '%s\n' "$command_path"
    return 0
  fi

  return 1
}

find_graphics_wrapper() {
  case "${MARS_GRAPHICS_WRAPPER:-}" in
    none | NONE | 0)
      return 1
      ;;
    "")
      ;;
    *)
      if print_executable_or_command "$MARS_GRAPHICS_WRAPPER"; then
        return 0
      fi
      printf 'MARS_GRAPHICS_WRAPPER is set but not executable or on PATH: %s\n' "$MARS_GRAPHICS_WRAPPER" >&2
      exit 127
      ;;
  esac

  if [ -n "${YAZELIX_RUNTIME_DIR:-}" ]; then
    print_first_executable \
      "$YAZELIX_RUNTIME_DIR/libexec/nixVulkanMesa" \
      "$YAZELIX_RUNTIME_DIR/libexec/nixVulkanIntel" \
      "$YAZELIX_RUNTIME_DIR/libexec/nixGLMesa" \
      "$YAZELIX_RUNTIME_DIR/libexec/nixGLDefault" \
      "$YAZELIX_RUNTIME_DIR/libexec/nixGL" \
      "$YAZELIX_RUNTIME_DIR/libexec/nixGLIntel" \
      "$YAZELIX_RUNTIME_DIR/bin/nixVulkanMesa" \
      "$YAZELIX_RUNTIME_DIR/bin/nixVulkanIntel" \
      "$YAZELIX_RUNTIME_DIR/bin/nixGLMesa" \
      "$YAZELIX_RUNTIME_DIR/bin/nixGLIntel" \
      && return 0
  fi

  print_first_executable \
    "$HOME/.nix-profile/libexec/nixVulkanMesa" \
    "$HOME/.nix-profile/libexec/nixVulkanIntel" \
    "$HOME/.nix-profile/libexec/nixGLMesa" \
    "$HOME/.nix-profile/libexec/nixGLDefault" \
    "$HOME/.nix-profile/libexec/nixGL" \
    "$HOME/.nix-profile/libexec/nixGLIntel" \
    "$HOME/.nix-profile/bin/nixVulkanMesa" \
    "$HOME/.nix-profile/bin/nixVulkanIntel" \
    "$HOME/.nix-profile/bin/nixGLMesa" \
    "$HOME/.nix-profile/bin/nixGLIntel" \
    "/etc/profiles/per-user/${USER:-}/libexec/nixVulkanMesa" \
    "/etc/profiles/per-user/${USER:-}/libexec/nixVulkanIntel" \
    "/etc/profiles/per-user/${USER:-}/libexec/nixGLMesa" \
    "/etc/profiles/per-user/${USER:-}/libexec/nixGLDefault" \
    "/etc/profiles/per-user/${USER:-}/libexec/nixGL" \
    "/etc/profiles/per-user/${USER:-}/libexec/nixGLIntel" \
    "/etc/profiles/per-user/${USER:-}/bin/nixVulkanMesa" \
    "/etc/profiles/per-user/${USER:-}/bin/nixVulkanIntel" \
    "/etc/profiles/per-user/${USER:-}/bin/nixGLMesa" \
    "/etc/profiles/per-user/${USER:-}/bin/nixGLIntel" \
    && return 0

  print_first_command \
    nixVulkanMesa \
    nixVulkanIntel \
    nixGLMesa \
    nixGLDefault \
    nixGL \
    nixGLIntel
}

configure_rio_config() {
  if [ -n "${MARS_CONFIG:-}" ]; then
    if [ -d "$MARS_CONFIG" ] && [ -r "$MARS_CONFIG/config.toml" ]; then
      export RIO_CONFIG_HOME="$MARS_CONFIG"
      export MARS_CHILD_ENV_SANITIZE=1
      return 0
    fi
    printf 'MARS_CONFIG must point to a readable Rio config directory containing config.toml: %s\n' "$MARS_CONFIG" >&2
    exit 127
  fi

  selected_config_home="$(select_default_config_home)"
  appearance_mode="$(select_appearance_mode)"
  render_strategy="$(select_render_strategy)"

  case "$appearance_mode:$render_strategy" in
    auto:events)
      export RIO_CONFIG_HOME="$selected_config_home"
      ;;
    *)
      config_parent="${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/mars"
      config_home="$config_parent/effective-config-$$"
      mkdir -p "$config_home"
      write_effective_config \
        "$selected_config_home/config.toml" \
        "$config_home/config.toml" \
        "$appearance_mode" \
        "$render_strategy"
      if [ -d "$selected_config_home/themes" ]; then
        rm -rf "$config_home/themes"
        ln -s "$selected_config_home/themes" "$config_home/themes"
      fi
      chmod 600 "$config_home/config.toml"
      export RIO_CONFIG_HOME="$config_home"
      ;;
  esac
  export MARS_CHILD_ENV_SANITIZE=1
}

select_appearance_mode() {
  case "${MARS_APPEARANCE:-dark}" in
    "" | dark | Dark | DARK | default | Default | DEFAULT)
      printf '%s\n' "dark"
      ;;
    light | Light | LIGHT)
      printf '%s\n' "light"
      ;;
    auto | Auto | AUTO | system | System | SYSTEM | adaptive | Adaptive | ADAPTIVE)
      printf '%s\n' "auto"
      ;;
    *)
      printf 'Unsupported MARS_APPEARANCE: %s\n' "${MARS_APPEARANCE:-}" >&2
      printf 'Use dark, light, auto, system, adaptive, or default.\n' >&2
      exit 64
      ;;
  esac
}

select_render_strategy() {
  case "${MARS_RENDER_STRATEGY:-events}" in
    events | Events | EVENTS | event | Event | EVENT | default | none | NONE | 0)
      printf '%s\n' "events"
      ;;
    game | Game | GAME)
      printf '%s\n' "game"
      ;;
    *)
      printf 'Unsupported MARS_RENDER_STRATEGY: %s\n' "$MARS_RENDER_STRATEGY" >&2
      printf 'Use events, game, default, none, or 0.\n' >&2
      exit 64
      ;;
  esac
}

select_emoji_font() {
  case "${MARS_EMOJI_FONT:-noto}" in
    "" | noto | Noto | NOTO | default | Default | DEFAULT)
      printf '%s\n' "noto"
      ;;
    twitter | Twitter | TWITTER | twemoji | Twemoji | TWEMOJI)
      printf '%s\n' "twitter"
      ;;
    serenityos | SerenityOS | SERENITYOS | serenity | Serenity | SERENITY | serenity-os | Serenity-OS | SERENITY-OS)
      printf '%s\n' "serenityos"
      ;;
    *)
      printf 'Unsupported MARS_EMOJI_FONT: %s\n' "${MARS_EMOJI_FONT:-}" >&2
      printf 'Use noto, twitter, or serenityos.\n' >&2
      exit 64
      ;;
  esac
}

select_profile_config_home() {
  full_config_home="$1"
  no_effects_config_home="$2"
  shaders_config_home="$3"

  case "${MARS_PROFILE:-${MARS_EFFECTS:-full}}" in
    "" | full | Full | FULL | effects | Effects | EFFECTS | default | Default | DEFAULT)
      printf '%s\n' "$full_config_home"
      ;;
    baseline | Baseline | BASELINE | no-effects | no_effects | none | None | NONE | 0)
      printf '%s\n' "$no_effects_config_home"
      ;;
    shader | Shader | SHADER | shaders | Shaders | SHADERS | cursor-shaders | cursor_shaders | ghostty-shaders | ghostty_shaders)
      printf '%s\n' "$shaders_config_home"
      ;;
    *)
      printf 'Unsupported MARS_PROFILE/MARS_EFFECTS: %s\n' "${MARS_PROFILE:-${MARS_EFFECTS:-}}" >&2
      printf 'Use full, default, baseline, no-effects, shaders, none, or 0.\n' >&2
      exit 64
      ;;
  esac
}

select_default_config_home() {
  selected_emoji_font="$(select_emoji_font)"
  case "$selected_emoji_font" in
    noto)
      select_profile_config_home "$default_config_home" "$baseline_config_home" "$shader_config_home"
      ;;
    twitter | serenityos)
      select_profile_config_home \
        "$emoji_config_home/$selected_emoji_font" \
        "$emoji_config_home/$selected_emoji_font/baseline" \
        "$emoji_config_home/$selected_emoji_font/profiles/shaders"
      ;;
    *)
      printf 'Unsupported normalized mars emoji font: %s\n' "$selected_emoji_font" >&2
      exit 64
      ;;
  esac
}

write_effective_config() {
  src="$1"
  dst="$2"
  appearance_mode="$3"
  render_strategy="$4"
  awk -v appearance_mode="$appearance_mode" -v render_strategy="$render_strategy" '
    BEGIN { inserted = 0; in_renderer = 0 }
    BEGIN {
      if (appearance_mode != "auto") {
        print "force-theme = \"" appearance_mode "\""
      }
    }
    /^[[:space:]]*force-theme[[:space:]]*=/ { next }
    /^[[:space:]]*\[renderer\][[:space:]]*$/ {
      print
      if (render_strategy == "game") {
        print "strategy = \"game\""
        inserted = 1
      }
      in_renderer = 1
      next
    }
    /^[[:space:]]*\[/ { in_renderer = 0 }
    in_renderer && render_strategy == "game" && /^[[:space:]]*strategy[[:space:]]*=/ { next }
    { print }
    END {
      if (render_strategy == "game" && !inserted) {
        print ""
        print "[renderer]"
        print "strategy = \"game\""
      }
    }
  ' "$src" > "$dst"
}

configure_rio_config
export MARS_HOST_LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}"
app_id="${MARS_APP_ID:-mars}"

if graphics_wrapper="$(find_graphics_wrapper)"; then
  exec "$graphics_wrapper" "$binary" --app-id "$app_id" "$@"
fi

exec "$binary" --app-id "$app_id" "$@"

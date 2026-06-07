#!/usr/bin/env sh
set -eu

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

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

is_truthy() {
  case "${1:-}" in
    1 | true | TRUE | yes | YES | on | ON)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

find_graphics_wrapper() {
  case "${YAZELIX_TERMINAL_GRAPHICS_WRAPPER:-}" in
    none | NONE | 0)
      return 1
      ;;
    "")
      ;;
    *)
      if print_executable_or_command "$YAZELIX_TERMINAL_GRAPHICS_WRAPPER"; then
        return 0
      fi
      printf 'YAZELIX_TERMINAL_GRAPHICS_WRAPPER is set but not executable or on PATH: %s\n' "$YAZELIX_TERMINAL_GRAPHICS_WRAPPER" >&2
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

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
state_root="${YAZELIX_TERMINAL_LOCAL_STATE:-$repo_root/target/yazelix-terminal-local}"
font_dir="$repo_root/sugarloaf/src/font/resources/SymbolsNerdFontMono"
shader_dir="$repo_root/misc/yazelix_terminal_shaders"
font_template="$repo_root/misc/yazelix_terminal_fonts.toml"
full_template="$repo_root/misc/yazelix_terminal_config.toml"
baseline_template="$repo_root/misc/yazelix_terminal_config_baseline.toml"
shader_template="$repo_root/misc/yazelix_terminal_config_shaders.toml"

find_emoji_font_dir() {
  if [ -n "${YAZELIX_TERMINAL_LOCAL_EMOJI_FONT_DIR:-}" ]; then
    [ -d "$YAZELIX_TERMINAL_LOCAL_EMOJI_FONT_DIR" ] || die "YAZELIX_TERMINAL_LOCAL_EMOJI_FONT_DIR is not a directory: $YAZELIX_TERMINAL_LOCAL_EMOJI_FONT_DIR"
    printf '%s\n' "$YAZELIX_TERMINAL_LOCAL_EMOJI_FONT_DIR"
    return 0
  fi

  if command -v fc-match >/dev/null 2>&1; then
    emoji_font_file="$(fc-match -f '%{file}\n' 'Noto Emoji:color=false:outline=true' 2>/dev/null | sed -n '1p')"
    if [ -n "$emoji_font_file" ] && [ -r "$emoji_font_file" ]; then
      dirname -- "$emoji_font_file"
      return 0
    fi
  fi

  for candidate in \
    "$HOME/.nix-profile/share/fonts/truetype" \
    "$HOME/.nix-profile/share/fonts" \
    "/run/current-system/sw/share/fonts/truetype" \
    "/run/current-system/sw/share/fonts" \
    "/usr/share/fonts/truetype" \
    "/usr/share/fonts"
  do
    if [ -d "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  printf '%s\n' "$font_dir"
}

escape_sed_replacement() {
  printf '%s' "$1" | sed 's/[&|\\]/\\&/g'
}

write_resolved_config() {
  src="$1"
  dst="$2"
  emoji_font_dir="$(find_emoji_font_dir)"
  font_dir_escaped="$(escape_sed_replacement "$font_dir")"
  emoji_font_dir_escaped="$(escape_sed_replacement "$emoji_font_dir")"
  shader_dir_escaped="$(escape_sed_replacement "$shader_dir")"

  mkdir -p "$(dirname -- "$dst")"
  while IFS= read -r yzt_config_line; do
    if [ "$yzt_config_line" = "@yazelix_terminal_fonts@" ]; then
      cat "$font_template"
    else
      printf '%s\n' "$yzt_config_line"
    fi
  done < "$src" >"$dst.template.tmp"
  sed \
    -e "s|@yazelix_terminal_font_dir@|$font_dir_escaped|g" \
    -e "s|@yazelix_terminal_emoji_font_dir@|$emoji_font_dir_escaped|g" \
    -e "s|@yazelix_terminal_shader_dir@|$shader_dir_escaped|g" \
    "$dst.template.tmp" >"$dst.tmp"
  rm -f "$dst.template.tmp"
  if grep -q "@yazelix_terminal_" "$dst.tmp"; then
    rm -f "$dst.tmp"
    die "unresolved Yazelix Terminal config placeholder in $src"
  fi
  mv "$dst.tmp" "$dst"
  chmod 600 "$dst"
}

prepare_local_configs() {
  [ -r "$full_template" ] || die "full config template is not readable: $full_template"
  [ -r "$baseline_template" ] || die "baseline config template is not readable: $baseline_template"
  [ -r "$shader_template" ] || die "shader config template is not readable: $shader_template"
  [ -r "$font_template" ] || die "font config template is not readable: $font_template"
  [ -d "$font_dir" ] || die "local Symbols Nerd Font directory is missing: $font_dir"
  [ -r "$font_dir/SymbolsNerdFontMono-Regular.ttf" ] || die "local Symbols Nerd Font file is missing: $font_dir/SymbolsNerdFontMono-Regular.ttf"
  [ -d "$shader_dir" ] || die "local shader directory is missing: $shader_dir"
  [ -r "$shader_dir/cursor_trail_dusk.glsl" ] || die "cursor shader is missing: $shader_dir/cursor_trail_dusk.glsl"
  [ -r "$shader_dir/generated_effects/sweep.glsl" ] || die "generated sweep shader is missing: $shader_dir/generated_effects/sweep.glsl"
  [ -r "$shader_dir/generated_effects/rectangle_boom.glsl" ] || die "generated rectangle shader is missing: $shader_dir/generated_effects/rectangle_boom.glsl"

  write_resolved_config "$full_template" "$state_root/full/config.toml"
  write_resolved_config "$baseline_template" "$state_root/baseline/config.toml"
  write_resolved_config "$shader_template" "$state_root/shaders/config.toml"
}

select_default_config_home() {
  case "${YAZELIX_TERMINAL_PROFILE:-${YAZELIX_TERMINAL_EFFECTS:-full}}" in
    "" | full | Full | FULL | effects | Effects | EFFECTS | default | Default | DEFAULT)
      printf '%s\n' "$state_root/full"
      ;;
    baseline | Baseline | BASELINE | no-effects | no_effects | none | None | NONE | 0)
      printf '%s\n' "$state_root/baseline"
      ;;
    shader | Shader | SHADER | shaders | Shaders | SHADERS | cursor-shaders | cursor_shaders | ghostty-shaders | ghostty_shaders)
      printf '%s\n' "$state_root/shaders"
      ;;
    *)
      printf 'Unsupported YAZELIX_TERMINAL_PROFILE/YAZELIX_TERMINAL_EFFECTS: %s\n' "${YAZELIX_TERMINAL_PROFILE:-${YAZELIX_TERMINAL_EFFECTS:-}}" >&2
      printf 'Use full, default, baseline, no-effects, shaders, none, or 0.\n' >&2
      exit 64
      ;;
  esac
}

write_game_config() {
  src="$1"
  dst="$2"
  awk '
    BEGIN { inserted = 0; in_renderer = 0 }
    /^[[:space:]]*\[renderer\][[:space:]]*$/ {
      print
      print "strategy = \"game\""
      inserted = 1
      in_renderer = 1
      next
    }
    /^[[:space:]]*\[/ { in_renderer = 0 }
    in_renderer && /^[[:space:]]*strategy[[:space:]]*=/ { next }
    { print }
    END {
      if (!inserted) {
        print ""
        print "[renderer]"
        print "strategy = \"game\""
      }
    }
  ' "$src" > "$dst"
}

configure_rio_config() {
  if [ -n "${YAZELIX_TERMINAL_CONFIG:-}" ]; then
    if [ -d "$YAZELIX_TERMINAL_CONFIG" ] && [ -r "$YAZELIX_TERMINAL_CONFIG/config.toml" ]; then
      export RIO_CONFIG_HOME="$YAZELIX_TERMINAL_CONFIG"
      export YAZELIX_TERMINAL_CHILD_ENV_SANITIZE=1
      return 0
    fi
    printf 'YAZELIX_TERMINAL_CONFIG must point to a readable Rio config directory containing config.toml: %s\n' "$YAZELIX_TERMINAL_CONFIG" >&2
    exit 127
  fi

  prepare_local_configs
  selected_config_home="$(select_default_config_home)"
  case "${YAZELIX_TERMINAL_RENDER_STRATEGY:-events}" in
    events | Events | EVENTS | event | Event | EVENT | default | none | NONE | 0)
      export RIO_CONFIG_HOME="$selected_config_home"
      export YAZELIX_TERMINAL_CHILD_ENV_SANITIZE=1
      ;;
    game | Game | GAME)
      config_home="$state_root/game-config"
      mkdir -p "$config_home"
      write_game_config "$selected_config_home/config.toml" "$config_home/config.toml"
      chmod 600 "$config_home/config.toml"
      export RIO_CONFIG_HOME="$config_home"
      export YAZELIX_TERMINAL_CHILD_ENV_SANITIZE=1
      ;;
    *)
      printf 'Unsupported YAZELIX_TERMINAL_RENDER_STRATEGY: %s\n' "$YAZELIX_TERMINAL_RENDER_STRATEGY" >&2
      printf 'Use events, game, default, none, or 0.\n' >&2
      exit 64
      ;;
  esac
}

cargo_build() {
  features="${YAZELIX_TERMINAL_LOCAL_FEATURES:-wgpu}"
  profile="${YAZELIX_TERMINAL_LOCAL_PROFILE:-debug}"

  cd "$repo_root"
  case "$profile" in
    debug | Debug | DEBUG)
      if [ -n "$features" ]; then
        cargo build -p rioterm --features "$features"
      else
        cargo build -p rioterm
      fi
      printf '%s\n' "$repo_root/target/debug/rio"
      ;;
    fast | Fast | FAST)
      if [ -n "$features" ]; then
        cargo build -p rioterm --profile fast --features "$features"
      else
        cargo build -p rioterm --profile fast
      fi
      printf '%s\n' "$repo_root/target/fast/rio"
      ;;
    release | Release | RELEASE)
      if [ -n "$features" ]; then
        cargo build -p rioterm --release --features "$features"
      else
        cargo build -p rioterm --release
      fi
      printf '%s\n' "$repo_root/target/release/rio"
      ;;
    *)
      printf 'Unsupported YAZELIX_TERMINAL_LOCAL_PROFILE: %s\n' "$profile" >&2
      printf 'Use debug, fast, or release.\n' >&2
      exit 64
      ;;
  esac
}

select_binary() {
  if [ -n "${YAZELIX_TERMINAL_LOCAL_BINARY:-}" ]; then
    printf '%s\n' "$YAZELIX_TERMINAL_LOCAL_BINARY"
    return 0
  fi

  profile="${YAZELIX_TERMINAL_LOCAL_PROFILE:-debug}"
  case "$profile" in
    debug | Debug | DEBUG) default_binary="$repo_root/target/debug/rio" ;;
    fast | Fast | FAST) default_binary="$repo_root/target/fast/rio" ;;
    release | Release | RELEASE) default_binary="$repo_root/target/release/rio" ;;
    *)
      printf 'Unsupported YAZELIX_TERMINAL_LOCAL_PROFILE: %s\n' "$profile" >&2
      printf 'Use debug, fast, or release.\n' >&2
      exit 64
      ;;
  esac

  if is_truthy "${YAZELIX_TERMINAL_LOCAL_SKIP_BUILD:-0}"; then
    printf '%s\n' "$default_binary"
    return 0
  fi

  cargo_build
}

binary="$(select_binary)"
[ -x "$binary" ] || die "local Rio binary is not executable: $binary"

configure_rio_config
export YAZELIX_TERMINAL_HOST_LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}"
app_id="${YAZELIX_TERMINAL_LOCAL_APP_ID:-yazelix-terminal-local}"
title="${YAZELIX_TERMINAL_LOCAL_TITLE:-Yazelix Terminal Local}"

if graphics_wrapper="$(find_graphics_wrapper)"; then
  exec "$graphics_wrapper" "$binary" --app-id "$app_id" --title-placeholder "$title" "$@"
fi

exec "$binary" --app-id "$app_id" --title-placeholder "$title" "$@"

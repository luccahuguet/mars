#!/usr/bin/env sh
set -eu

binary="@yazelix_terminal_binary@"
default_config_home="@yazelix_terminal_config_home@"

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

configure_rio_config() {
  if [ -n "${YAZELIX_TERMINAL_CONFIG:-}" ]; then
    if [ -d "$YAZELIX_TERMINAL_CONFIG" ] && [ -r "$YAZELIX_TERMINAL_CONFIG/config.toml" ]; then
      export RIO_CONFIG_HOME="$YAZELIX_TERMINAL_CONFIG"
      return 0
    fi
    printf 'YAZELIX_TERMINAL_CONFIG must point to a readable Rio config directory containing config.toml: %s\n' "$YAZELIX_TERMINAL_CONFIG" >&2
    exit 127
  fi

  if [ -n "${RIO_CONFIG_HOME:-}" ]; then
    return 0
  fi

  case "${YAZELIX_TERMINAL_RENDER_STRATEGY:-events}" in
    events | Events | EVENTS | event | Event | EVENT | default | none | NONE | 0)
      export RIO_CONFIG_HOME="$default_config_home"
      ;;
    game | Game | GAME)
      config_parent="${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/yazelix-terminal"
      config_home="$config_parent/game-config"
      mkdir -p "$config_home"
      install -m 600 "$default_config_home/config.toml" "$config_home/config.toml"
      {
        printf '\n[renderer]\n'
        printf 'strategy = "game"\n'
      } >> "$config_home/config.toml"
      export RIO_CONFIG_HOME="$config_home"
      ;;
    *)
      printf 'Unsupported YAZELIX_TERMINAL_RENDER_STRATEGY: %s\n' "$YAZELIX_TERMINAL_RENDER_STRATEGY" >&2
      printf 'Use events, game, default, none, or 0.\n' >&2
      exit 64
      ;;
  esac
}

configure_rio_config

if graphics_wrapper="$(find_graphics_wrapper)"; then
  exec "$graphics_wrapper" "$binary" --app-id yazelix-terminal "$@"
fi

exec "$binary" --app-id yazelix-terminal "$@"

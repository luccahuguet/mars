#!/usr/bin/env sh
set -eu

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

usage() {
  printf 'usage: %s /path/to/yazelix-terminal-package\n' "$0" >&2
  printf '   or: YAZELIX_TERMINAL_PACKAGE=/path/to/package %s\n' "$0" >&2
}

package_dir="${1:-${YAZELIX_TERMINAL_PACKAGE:-}}"
if [ -z "$package_dir" ]; then
  usage
  exit 64
fi

config="$package_dir/share/yazelix-terminal/config.toml"
wrapper="$package_dir/bin/yazelix-terminal-desktop"

[ -r "$config" ] || die "packaged config is not readable: $config"
[ -x "$wrapper" ] || die "packaged desktop wrapper is not executable: $wrapper"

if grep -Eq '^[[:space:]]*strategy[[:space:]]*=[[:space:]]*"game"' "$config"; then
  die "packaged config defaults to renderer.strategy = \"game\""
fi

version_log="$(mktemp "${TMPDIR:-/tmp}/yzt-event-version.XXXXXX")"
trap 'rm -f "$version_log"' EXIT INT HUP TERM

if ! "$wrapper" --version >"$version_log" 2>&1; then
  cat "$version_log" >&2
  die "wrapper did not start with the packaged event-mode config"
fi

runtime_dir="$(mktemp -d "${TMPDIR:-/tmp}/yzt-event-runtime.XXXXXX")"
game_log="$(mktemp "${TMPDIR:-/tmp}/yzt-event-game.XXXXXX")"
trap 'rm -rf "$runtime_dir"; rm -f "$version_log" "$game_log"' EXIT INT HUP TERM

if ! XDG_RUNTIME_DIR="$runtime_dir" YAZELIX_TERMINAL_RENDER_STRATEGY=game "$wrapper" --version >"$game_log" 2>&1; then
  cat "$game_log" >&2
  die "wrapper did not start with explicit game-mode override"
fi

game_config="$runtime_dir/yazelix-terminal/game-config/config.toml"
[ -r "$game_config" ] || die "explicit game-mode config was not materialized: $game_config"
if ! grep -Eq '^[[:space:]]*strategy[[:space:]]*=[[:space:]]*"game"' "$game_config"; then
  die "explicit game-mode config does not set renderer.strategy = \"game\""
fi

printf 'Yazelix Terminal event-mode package smoke passed\n'
printf '%s\n' '- packaged config does not default to renderer.strategy = "game"'
printf '%s\n' '- desktop wrapper starts with packaged config'
printf '%s\n' '- explicit YAZELIX_TERMINAL_RENDER_STRATEGY=game escape hatch works'

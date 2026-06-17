#!/usr/bin/env sh
set -eu

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

usage() {
  printf 'usage: %s /path/to/mars-package\n' "$0" >&2
  printf '   or: MARS_PACKAGE=/path/to/package %s\n' "$0" >&2
}

package_dir="${1:-${MARS_PACKAGE:-}}"
if [ -z "$package_dir" ]; then
  usage
  exit 64
fi

config="$package_dir/share/mars/config.toml"
baseline_config="$package_dir/share/mars/baseline/config.toml"
shader_config="$package_dir/share/mars/profiles/shaders/config.toml"
wrapper="$package_dir/bin/mars-desktop"

[ -r "$config" ] || die "packaged config is not readable: $config"
[ -r "$baseline_config" ] || die "packaged baseline config is not readable: $baseline_config"
[ -r "$shader_config" ] || die "packaged shader profile config is not readable: $shader_config"
[ -x "$wrapper" ] || die "packaged desktop wrapper is not executable: $wrapper"

run_wrapper_without_host_config() (
  unset MARS_CONFIG
  unset RIO_CONFIG_HOME
  "$@"
)

assert_confirm_before_quit_enabled() {
  confirm_config="$1"
  if ! grep -Eq '^[[:space:]]*confirm-before-quit[[:space:]]*=[[:space:]]*true' "$confirm_config"; then
    die "packaged config must keep confirm-before-quit enabled: $confirm_config"
  fi
}

find_effective_config() {
  runtime_dir="$1"
  find "$runtime_dir/mars" -maxdepth 2 -type f -path '*/effective-config-*/config.toml' | sed -n '1p'
}

for confirm_config in \
  "$config" \
  "$baseline_config" \
  "$shader_config" \
  "$package_dir/share/mars/emoji/twitter/config.toml" \
  "$package_dir/share/mars/emoji/twitter/baseline/config.toml" \
  "$package_dir/share/mars/emoji/twitter/profiles/shaders/config.toml" \
  "$package_dir/share/mars/emoji/serenityos/config.toml" \
  "$package_dir/share/mars/emoji/serenityos/baseline/config.toml" \
  "$package_dir/share/mars/emoji/serenityos/profiles/shaders/config.toml"
do
  [ -r "$confirm_config" ] || die "packaged config is not readable: $confirm_config"
  assert_confirm_before_quit_enabled "$confirm_config"
done

if grep -Eq '^[[:space:]]*strategy[[:space:]]*=[[:space:]]*"game"' "$config"; then
  die "packaged config defaults to renderer.strategy = \"game\""
fi
if grep -Eq '^[[:space:]]*custom-shader[[:space:]]*=' "$config"; then
  die "packaged default config should not enable custom shaders"
fi
if ! grep -Eq '^[[:space:]]*trail-cursor[[:space:]]*=[[:space:]]*true' "$config"; then
  die "packaged default config should enable Rio trail-cursor"
fi
if grep -Eq '^[[:space:]]*custom-shader[[:space:]]*=' "$baseline_config"; then
  die "baseline config should not enable custom shaders"
fi
if grep -Eq '^[[:space:]]*trail-cursor[[:space:]]*=' "$baseline_config"; then
  die "baseline config should not enable trail-cursor"
fi
if ! grep -Eq '^[[:space:]]*custom-shader[[:space:]]*=' "$shader_config"; then
  die "shader profile config should enable custom shaders"
fi
if ! grep -Eq '^[[:space:]]*trail-cursor[[:space:]]*=[[:space:]]*true' "$shader_config"; then
  die "shader profile config should keep Rio trail-cursor enabled"
fi

version_log="$(mktemp "${TMPDIR:-/tmp}/yzt-event-version.XXXXXX")"
shader_log="$(mktemp "${TMPDIR:-/tmp}/yzt-shader-version.XXXXXX")"
trap 'rm -f "$version_log" "$shader_log"' EXIT INT HUP TERM

if ! MARS_PROFILE=full run_wrapper_without_host_config "$wrapper" --version >"$version_log" 2>&1; then
  cat "$version_log" >&2
  die "wrapper did not start with the packaged event-mode config"
fi
if ! MARS_PROFILE=shaders run_wrapper_without_host_config "$wrapper" --version >"$shader_log" 2>&1; then
  cat "$shader_log" >&2
  die "wrapper did not start with the packaged shader profile config"
fi

runtime_dir="$(mktemp -d "${TMPDIR:-/tmp}/yzt-event-runtime.XXXXXX")"
game_log="$(mktemp "${TMPDIR:-/tmp}/yzt-event-game.XXXXXX")"
baseline_runtime_dir="$(mktemp -d "${TMPDIR:-/tmp}/yzt-baseline-runtime.XXXXXX")"
baseline_log="$(mktemp "${TMPDIR:-/tmp}/yzt-baseline.XXXXXX")"
trap 'rm -rf "$runtime_dir" "$baseline_runtime_dir"; rm -f "$version_log" "$shader_log" "$game_log" "$baseline_log"' EXIT INT HUP TERM

if ! XDG_RUNTIME_DIR="$runtime_dir" MARS_PROFILE=full MARS_RENDER_STRATEGY=game run_wrapper_without_host_config "$wrapper" --version >"$game_log" 2>&1; then
  cat "$game_log" >&2
  die "wrapper did not start with explicit game-mode override"
fi

game_config="$(find_effective_config "$runtime_dir")"
[ -r "$game_config" ] || die "explicit game-mode config was not materialized: $game_config"
if ! grep -Eq '^[[:space:]]*strategy[[:space:]]*=[[:space:]]*"game"' "$game_config"; then
  die "explicit game-mode config does not set renderer.strategy = \"game\""
fi
if grep -Eq '^[[:space:]]*custom-shader[[:space:]]*=' "$game_config"; then
  die "default game-mode config should not enable custom shaders"
fi
if ! grep -Eq '^[[:space:]]*trail-cursor[[:space:]]*=[[:space:]]*true' "$game_config"; then
  die "default game-mode config should keep Rio trail-cursor enabled"
fi

if ! XDG_RUNTIME_DIR="$baseline_runtime_dir" MARS_PROFILE=baseline MARS_RENDER_STRATEGY=game run_wrapper_without_host_config "$wrapper" --version >"$baseline_log" 2>&1; then
  cat "$baseline_log" >&2
  die "wrapper did not start with baseline no-effects profile"
fi

baseline_game_config="$(find_effective_config "$baseline_runtime_dir")"
[ -r "$baseline_game_config" ] || die "baseline game-mode config was not materialized: $baseline_game_config"
if ! grep -Eq '^[[:space:]]*strategy[[:space:]]*=[[:space:]]*"game"' "$baseline_game_config"; then
  die "baseline game-mode config does not set renderer.strategy = \"game\""
fi
if grep -Eq '^[[:space:]]*custom-shader[[:space:]]*=' "$baseline_game_config"; then
  die "baseline game-mode config should not enable custom shaders"
fi
if grep -Eq '^[[:space:]]*trail-cursor[[:space:]]*=' "$baseline_game_config"; then
  die "baseline game-mode config should not enable trail-cursor"
fi

printf 'Mars Terminal event-mode package smoke passed\n'
printf '%s\n' '- packaged config does not default to renderer.strategy = "game"'
printf '%s\n' '- packaged configs keep confirm-before-quit enabled'
printf '%s\n' '- packaged default uses Rio trail-cursor without custom shaders'
printf '%s\n' '- desktop wrapper starts with packaged default and shader profiles'
printf '%s\n' '- explicit MARS_RENDER_STRATEGY=game escape hatch works'
printf '%s\n' '- baseline no-effects profile starts and composes with game strategy'

#!/usr/bin/env python3
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path


HEX_COLOR = re.compile(r"^#[0-9a-fA-F]{6}$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Write a launch-local Mars config using the current Yazelix cursor color."
    )
    parser.add_argument(
        "--source-config",
        required=True,
        help="base Mars config.toml to copy and patch",
    )
    parser.add_argument(
        "--output-root",
        help="directory where a launch config directory should be created",
    )
    parser.add_argument(
        "--yzc",
        default=os.environ.get("YZC", "yzc"),
        help="yzc executable to query",
    )
    parser.add_argument(
        "--cursor-json",
        help="test hook: use this yzc current --format json payload instead of running yzc",
    )
    return parser.parse_args()


def load_cursor(args: argparse.Namespace) -> dict:
    if args.cursor_json is not None:
        raw = args.cursor_json
    else:
        result = subprocess.run(
            [args.yzc, "current", "--format", "json"],
            check=True,
            text=True,
            capture_output=True,
        )
        raw = result.stdout

    try:
        cursor = json.loads(raw or "{}")
    except json.JSONDecodeError as error:
        raise ValueError(f"yzc current returned invalid JSON: {error}") from error
    if not isinstance(cursor, dict):
        raise ValueError("yzc current JSON must be an object")
    return cursor


def validate_cursor_color(cursor: dict) -> str | None:
    color = cursor.get("color")
    if color is None:
        return None
    if not isinstance(color, str) or not HEX_COLOR.fullmatch(color):
        raise ValueError(f"yzc current returned invalid cursor color: {color!r}")
    return color.lower()


def patch_cursor_color(config: str, color: str | None) -> str:
    if color is None:
        return config

    lines = config.splitlines(keepends=True)
    colors_index = None
    for index, line in enumerate(lines):
        if line.strip() == "[colors]":
            colors_index = index
            break

    cursor_line = f'cursor = "{color}"\n'
    if colors_index is None:
        prefix = config
        if prefix and not prefix.endswith("\n"):
            prefix += "\n"
        if prefix and not prefix.endswith("\n\n"):
            prefix += "\n"
        return f"{prefix}[colors]\n{cursor_line}"

    next_section = len(lines)
    for index in range(colors_index + 1, len(lines)):
        stripped = lines[index].strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            next_section = index
            break

    for index in range(colors_index + 1, next_section):
        if re.match(r"^\s*cursor\s*=", lines[index]):
            indent = re.match(r"^(\s*)", lines[index]).group(1)
            lines[index] = f'{indent}cursor = "{color}"\n'
            return "".join(lines)

    lines.insert(colors_index + 1, cursor_line)
    return "".join(lines)


def output_root(raw: str | None) -> Path:
    if raw:
        return Path(raw).expanduser()
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR")
    if runtime_dir:
        return Path(runtime_dir) / "mars-yazelix-cursor-config"
    return Path(tempfile.gettempdir()) / f"mars-yazelix-cursor-config-{os.getuid()}"


def main() -> int:
    args = parse_args()
    source = Path(args.source_config).expanduser()
    if not source.is_file():
        print(f"Mars source config is missing: {source}", file=sys.stderr)
        return 1

    try:
        cursor = load_cursor(args)
        color = validate_cursor_color(cursor)
        source_config = source.read_text(encoding="utf-8")
        tomllib.loads(source_config)
        patched_config = patch_cursor_color(source_config, color)
        tomllib.loads(patched_config)
    except (OSError, subprocess.CalledProcessError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"Could not materialize Mars Yazelix cursor config: {error}", file=sys.stderr)
        return 1

    root = output_root(args.output_root)
    try:
        destination = Path(tempfile.mkdtemp(prefix="launch-", dir=root))
    except FileNotFoundError:
        root.mkdir(parents=True, exist_ok=True)
        destination = Path(tempfile.mkdtemp(prefix="launch-", dir=root))

    try:
        (destination / "config.toml").write_text(patched_config, encoding="utf-8")
        family = cursor.get("family")
        if color is None:
            v1_mode = "disabled"
        elif family == "mono":
            v1_mode = "monocolor"
        else:
            v1_mode = "monocolor-fallback"
        metadata = {
            "name": cursor.get("name"),
            "family": family,
            "color": color,
            "v1_mode": v1_mode,
        }
        (destination / "yazelix_cursor.json").write_text(
            json.dumps(metadata, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    except OSError as error:
        print(f"Could not write Mars Yazelix cursor config: {error}", file=sys.stderr)
        return 1

    print(destination)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

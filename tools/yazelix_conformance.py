#!/usr/bin/env python3
"""Small conformance harness for the Yazelix terminal experiment."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "conformance" / "fixtures" / "manifest.json"
DEFAULT_ENV_OUTPUT = ROOT / "artifacts" / "conformance" / "env.json"
DEFAULT_SCREENSHOT_DIR = ROOT / "artifacts" / "conformance" / "screenshots"
DEFAULT_CPU_CONFIG = ROOT / "artifacts" / "conformance" / "rio_cpu_config"
DEFAULT_SHADER_SCREENSHOT_DIR = ROOT / "artifacts" / "shader_probe" / "screenshots"
DEFAULT_SHADER_CONFIG = ROOT / "artifacts" / "shader_probe" / "rio_wgpu_config"


def load_manifest() -> dict[str, Any]:
    with MANIFEST.open("r", encoding="utf-8") as manifest_file:
        manifest = json.load(manifest_file)
    if manifest.get("version") != 1:
        raise SystemExit(f"unsupported manifest version in {MANIFEST}")
    return manifest


def fixture_bytes(fixture: dict[str, Any]) -> bytes:
    try:
        return bytes.fromhex(fixture["hex"])
    except ValueError as err:
        raise SystemExit(f"fixture {fixture.get('id')} has invalid hex: {err}") from err


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run_capture(argv: list[str]) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            argv,
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except FileNotFoundError:
        return {
            "argv": argv,
            "ok": False,
            "status": "not_found",
            "stdout": "",
            "stderr": "",
        }
    return {
        "argv": argv,
        "ok": completed.returncode == 0,
        "status": completed.returncode,
        "stdout": completed.stdout.strip(),
        "stderr": completed.stderr.strip(),
    }


def command_list(_: argparse.Namespace) -> int:
    manifest = load_manifest()
    for fixture in manifest["fixtures"]:
        data = fixture_bytes(fixture)
        print(
            f"{fixture['id']}\t{fixture['tier']}\t{fixture['protocol']}\t"
            f"{len(data)} bytes\tsha256={sha256_hex(data)}"
        )
    return 0


def command_emit(args: argparse.Namespace) -> int:
    manifest = load_manifest()
    matches = [f for f in manifest["fixtures"] if f["id"] == args.fixture]
    if not matches:
        known = ", ".join(f["id"] for f in manifest["fixtures"])
        raise SystemExit(f"unknown fixture {args.fixture!r}; known fixtures: {known}")
    sys.stdout.buffer.write(fixture_bytes(matches[0]))
    return 0


def command_verify(_: argparse.Namespace) -> int:
    manifest = load_manifest()
    seen: set[str] = set()
    for fixture in manifest["fixtures"]:
        fixture_id = fixture.get("id")
        if not fixture_id:
            raise SystemExit("fixture missing id")
        if fixture_id in seen:
            raise SystemExit(f"duplicate fixture id: {fixture_id}")
        seen.add(fixture_id)
        data = fixture_bytes(fixture)
        if not data:
            raise SystemExit(f"fixture {fixture_id} is empty")
        print(f"ok {fixture_id} {len(data)} bytes sha256={sha256_hex(data)}")
    shader = ROOT / "conformance" / "shaders" / "ghostty_cursor_probe.glsl"
    shader_text = shader.read_text(encoding="utf-8")
    for required in (
        "iChannel0",
        "iResolution",
        "iCurrentCursor",
        "iCurrentCursorColor",
        "iCursorVisible",
    ):
        if required not in shader_text:
            raise SystemExit(f"shader probe missing {required}")
    print(f"ok {shader.relative_to(ROOT)}")
    return 0


def command_record_env(args: argparse.Namespace) -> int:
    output = Path(args.output).expanduser().resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "repo": str(ROOT),
        "timestamp_unix": int(time.time()),
        "commands": {
            "git_head": run_capture(["git", "rev-parse", "HEAD"]),
            "git_status": run_capture(["git", "status", "--short", "--branch"]),
            "rio_version": run_capture([args.rio_bin, "--version"]),
            "rustc": run_capture(["rustc", "--version"]),
            "cargo": run_capture(["cargo", "--version"]),
            "rustup_active_toolchain": run_capture(["rustup", "show", "active-toolchain"]),
            "vulkaninfo_summary": run_capture(["vulkaninfo", "--summary"]),
        },
    }
    output.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    print(output)
    return 0


def ensure_cpu_config(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    config = path / "config.toml"
    config.write_text("[renderer]\nuse-cpu = true\n", encoding="utf-8")


def ensure_shader_config(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    config = path / "config.toml"
    config.write_text(
        "[renderer]\n"
        'backend = "Webgpu"\n'
        'custom-shader = ["conformance/shaders/ghostty_cursor_probe.glsl"]\n',
        encoding="utf-8",
    )


def capture_cosmic_screenshot(
    output_dir: Path,
    process: subprocess.Popen[Any],
    settle_seconds: int,
    sleep_seconds: int,
) -> int:
    screenshot_tool = shutil.which("cosmic-screenshot")
    if screenshot_tool is None:
        raise SystemExit("cosmic-screenshot not found")

    try:
        time.sleep(max(1, int(settle_seconds)))
        shot = subprocess.run(
            [
                screenshot_tool,
                "--interactive=false",
                "--modal=false",
                "--notify=false",
                "--save-dir",
                str(output_dir),
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if shot.returncode != 0:
            raise SystemExit(shot.stderr.strip() or "screenshot failed")
        print(shot.stdout.strip())
        return 0
    finally:
        try:
            process.wait(timeout=max(1, int(sleep_seconds) + 3))
        except subprocess.TimeoutExpired:
            process.terminate()
            process.wait(timeout=5)


def command_launch_cpu_screenshot(args: argparse.Namespace) -> int:
    output_dir = Path(args.output_dir).expanduser().resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    cpu_config = Path(args.config_dir).expanduser().resolve()
    ensure_cpu_config(cpu_config)

    env = os.environ.copy()
    env["RIO_CONFIG_HOME"] = str(cpu_config)

    command = [
        "nix",
        "develop",
        "-c",
        args.rio_bin,
        "--app-id",
        "yazelix-terminal-conformance",
        "--title-placeholder",
        "Yazelix Terminal Conformance",
        "-e",
        "bash",
        "--noprofile",
        "--norc",
        "-c",
        (
            "printf 'yazelix-terminal conformance\\n"
            "CPU renderer screenshot probe\\n"
            "PID $$\\n'; "
            f"sleep {int(args.sleep_seconds)}"
        ),
    ]
    process = subprocess.Popen(command, cwd=ROOT, env=env)
    return capture_cosmic_screenshot(
        output_dir,
        process,
        args.settle_seconds,
        args.sleep_seconds,
    )


def command_launch_wgpu_shader_screenshot(args: argparse.Namespace) -> int:
    output_dir = Path(args.output_dir).expanduser().resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    shader_config = Path(args.config_dir).expanduser().resolve()
    ensure_shader_config(shader_config)

    env = os.environ.copy()
    env["RIO_CONFIG_HOME"] = str(shader_config)
    env["WGPU_BACKEND"] = args.wgpu_backend

    command = [
        "nix",
        "develop",
        "-c",
        args.rio_bin,
        "--app-id",
        "yazelix-terminal-shader-probe",
        "--title-placeholder",
        "Yazelix Terminal Shader Probe",
        "-e",
        "bash",
        "--noprofile",
        "--norc",
        "-c",
        (
            "printf 'yazelix-terminal shader probe\\n"
            "Ghostty cursor uniforms via WGPU\\n"
            "PID $$\\n'; "
            f"sleep {int(args.sleep_seconds)}"
        ),
    ]
    process = subprocess.Popen(command, cwd=ROOT, env=env)
    return capture_cosmic_screenshot(
        output_dir,
        process,
        args.settle_seconds,
        args.sleep_seconds,
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(required=True)

    list_parser = subcommands.add_parser("list", help="List fixture ids and hashes")
    list_parser.set_defaults(func=command_list)

    emit_parser = subcommands.add_parser("emit", help="Write one fixture byte stream")
    emit_parser.add_argument("fixture")
    emit_parser.set_defaults(func=command_emit)

    verify_parser = subcommands.add_parser("verify", help="Validate fixtures and shader probe")
    verify_parser.set_defaults(func=command_verify)

    env_parser = subcommands.add_parser("record-env", help="Record local version/source evidence")
    env_parser.add_argument("--output", default=str(DEFAULT_ENV_OUTPUT))
    env_parser.add_argument("--rio-bin", default="target/debug/rio")
    env_parser.set_defaults(func=command_record_env)

    shot_parser = subcommands.add_parser(
        "launch-cpu-screenshot",
        help="Launch Rio with CPU renderer and capture a COSMIC screenshot",
    )
    shot_parser.add_argument("--output-dir", default=str(DEFAULT_SCREENSHOT_DIR))
    shot_parser.add_argument("--config-dir", default=str(DEFAULT_CPU_CONFIG))
    shot_parser.add_argument("--rio-bin", default="target/debug/rio")
    shot_parser.add_argument("--sleep-seconds", default=8, type=int)
    shot_parser.add_argument("--settle-seconds", default=2, type=int)
    shot_parser.set_defaults(func=command_launch_cpu_screenshot)

    shader_shot_parser = subcommands.add_parser(
        "launch-wgpu-shader-screenshot",
        help="Launch Rio with WGPU custom shader probe and capture a COSMIC screenshot",
    )
    shader_shot_parser.add_argument(
        "--output-dir", default=str(DEFAULT_SHADER_SCREENSHOT_DIR)
    )
    shader_shot_parser.add_argument("--config-dir", default=str(DEFAULT_SHADER_CONFIG))
    shader_shot_parser.add_argument("--rio-bin", default="target/debug/rio")
    shader_shot_parser.add_argument("--wgpu-backend", default="gl")
    shader_shot_parser.add_argument("--sleep-seconds", default=8, type=int)
    shader_shot_parser.add_argument("--settle-seconds", default=2, type=int)
    shader_shot_parser.set_defaults(func=command_launch_wgpu_shader_screenshot)

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())

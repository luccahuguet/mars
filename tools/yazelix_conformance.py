#!/usr/bin/env python3
"""Small conformance harness for the Yazelix terminal experiment."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import select
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "conformance" / "fixtures" / "manifest.json"
KEYBOARD_MANIFEST = ROOT / "conformance" / "fixtures" / "kitty_keyboard_blackbox.json"
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


def load_keyboard_manifest() -> dict[str, Any]:
    with KEYBOARD_MANIFEST.open("r", encoding="utf-8") as manifest_file:
        manifest = json.load(manifest_file)
    if manifest.get("version") != 1:
        raise SystemExit(f"unsupported keyboard manifest version in {KEYBOARD_MANIFEST}")
    return manifest


def fixture_bytes(fixture: dict[str, Any]) -> bytes:
    try:
        return bytes.fromhex(fixture["hex"])
    except ValueError as err:
        raise SystemExit(f"fixture {fixture.get('id')} has invalid hex: {err}") from err


def hex_bytes(value: str, context: str) -> bytes:
    try:
        return bytes.fromhex(value)
    except ValueError as err:
        raise SystemExit(f"{context} has invalid hex: {err}") from err


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def keyboard_cases_by_id(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    cases: dict[str, dict[str, Any]] = {}
    for case in manifest.get("cases", []):
        case_id = case.get("id")
        if not case_id:
            raise SystemExit("keyboard case missing id")
        if case_id in cases:
            raise SystemExit(f"duplicate keyboard case id: {case_id}")
        cases[case_id] = case
    return cases


def expected_keyboard_fragments(case: dict[str, Any]) -> list[bytes]:
    expect = case.get("expect", {})
    mode = expect.get("mode")
    if mode == "exact":
        return [hex_bytes(expect.get("hex", ""), f"keyboard case {case['id']} expected hex")]
    if mode == "contains":
        fragments = expect.get("fragments", [])
        if not fragments:
            raise SystemExit(f"keyboard case {case['id']} has no expected fragments")
        return [
            hex_bytes(fragment, f"keyboard case {case['id']} expected fragment")
            for fragment in fragments
        ]
    raise SystemExit(f"keyboard case {case['id']} has unsupported expectation mode: {mode}")


def keyboard_capture_matches(case: dict[str, Any], captured: bytes) -> bool:
    expect = case["expect"]
    fragments = expected_keyboard_fragments(case)
    if expect["mode"] == "exact":
        return captured == fragments[0]

    cursor = 0
    for fragment in fragments:
        offset = captured.find(fragment, cursor)
        if offset == -1:
            return False
        cursor = offset + len(fragment)
    return True


def validate_keyboard_manifest() -> None:
    manifest = load_keyboard_manifest()
    cleanup = hex_bytes(manifest.get("cleanup_hex", ""), "keyboard cleanup")
    if not cleanup:
        raise SystemExit("keyboard cleanup is empty")
    for case in keyboard_cases_by_id(manifest).values():
        setup = hex_bytes(case.get("setup_hex", ""), f"keyboard case {case['id']} setup")
        if not setup:
            raise SystemExit(f"keyboard case {case['id']} setup is empty")
        expected_keyboard_fragments(case)
        for field in ("tier", "keys", "workflow", "source"):
            if not case.get(field):
                raise SystemExit(f"keyboard case {case['id']} missing {field}")


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
    validate_keyboard_manifest()
    print(f"ok {KEYBOARD_MANIFEST.relative_to(ROOT)}")
    return 0


def command_keyboard_list(_: argparse.Namespace) -> int:
    manifest = load_keyboard_manifest()
    for case in manifest["cases"]:
        expect = case["expect"]
        if expect["mode"] == "exact":
            expectation = expect["hex"]
        else:
            expectation = ",".join(expect["fragments"])
        print(
            f"{case['id']}\t{case['tier']}\t{case['keys']}\t"
            f"{expect['mode']}\t{expectation}"
        )
    return 0


def selected_keyboard_cases(
    manifest: dict[str, Any],
    selected_ids: list[str] | None,
) -> list[dict[str, Any]]:
    cases = keyboard_cases_by_id(manifest)
    if not selected_ids:
        return list(cases.values())

    selected: list[dict[str, Any]] = []
    for case_id in selected_ids:
        try:
            selected.append(cases[case_id])
        except KeyError as err:
            known = ", ".join(cases)
            raise SystemExit(f"unknown keyboard case {case_id!r}; known cases: {known}") from err
    return selected


def read_keyboard_capture(timeout_seconds: float, idle_seconds: float) -> bytes:
    import termios
    import tty

    fd = sys.stdin.fileno()
    old_settings = termios.tcgetattr(fd)
    data = bytearray()
    start = time.monotonic()
    last_input: float | None = None

    try:
        tty.setraw(fd)
        while time.monotonic() - start < timeout_seconds:
            readable, _, _ = select.select([sys.stdin], [], [], 0.05)
            if readable:
                chunk = os.read(fd, 4096)
                if not chunk:
                    break
                data.extend(chunk)
                last_input = time.monotonic()
            elif data and last_input is not None:
                if time.monotonic() - last_input >= idle_seconds:
                    break
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)

    return bytes(data)


def command_keyboard_capture(args: argparse.Namespace) -> int:
    manifest = load_keyboard_manifest()
    cleanup = hex_bytes(manifest["cleanup_hex"], "keyboard cleanup")
    cases = selected_keyboard_cases(manifest, args.case)
    output = Path(args.output).expanduser().resolve() if args.output else (
        ROOT / "artifacts" / "conformance" / "keyboard_captures" / f"{args.terminal}.json"
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "version": 1,
        "terminal": args.terminal,
        "timestamp_unix": int(time.time()),
        "spec": manifest["spec"],
        "cases": [],
    }

    for index, case in enumerate(cases, start=1):
        print(
            f"[{index}/{len(cases)}] {case['id']}: {case['keys']}\n"
            f"  {case['workflow']}\n"
            "  Press Enter to arm capture, then press the requested key sequence.",
            file=sys.stderr,
        )
        input()
        sys.stdout.buffer.write(hex_bytes(case["setup_hex"], f"keyboard case {case['id']} setup"))
        sys.stdout.buffer.flush()
        try:
            captured = read_keyboard_capture(args.timeout, args.idle_timeout)
        finally:
            sys.stdout.buffer.write(cleanup)
            sys.stdout.buffer.flush()
        matched = keyboard_capture_matches(case, captured)
        print(
            f"  captured {len(captured)} bytes: {captured.hex()} "
            f"{'ok' if matched else 'mismatch'}",
            file=sys.stderr,
        )
        report["cases"].append(
            {
                "id": case["id"],
                "captured_hex": captured.hex(),
                "matched": matched,
            }
        )

    output.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    print(output)
    return 0 if all(case["matched"] for case in report["cases"]) else 1


def command_keyboard_verify_capture(args: argparse.Namespace) -> int:
    manifest = load_keyboard_manifest()
    cases = keyboard_cases_by_id(manifest)
    capture_path = Path(args.capture).expanduser()
    report = json.loads(capture_path.read_text(encoding="utf-8"))
    if report.get("version") != 1:
        raise SystemExit(f"unsupported capture version in {capture_path}")

    captured_by_id = {
        capture["id"]: capture
        for capture in report.get("cases", [])
        if capture.get("id")
    }
    if args.require_all:
        missing = sorted(set(cases) - set(captured_by_id))
        if missing:
            raise SystemExit(f"capture missing keyboard cases: {', '.join(missing)}")

    failed = False
    for case_id, capture in captured_by_id.items():
        if case_id not in cases:
            raise SystemExit(f"capture has unknown keyboard case: {case_id}")
        data = hex_bytes(capture.get("captured_hex", ""), f"capture {case_id}")
        matched = keyboard_capture_matches(cases[case_id], data)
        print(f"{'ok' if matched else 'fail'} {case_id} {len(data)} bytes")
        failed = failed or not matched
    return 1 if failed else 0


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


def ensure_shader_config(path: Path, shader_paths: list[str]) -> None:
    path.mkdir(parents=True, exist_ok=True)
    config = path / "config.toml"
    shader_entries = ", ".join(json.dumps(shader) for shader in shader_paths)
    config.write_text(
        "[renderer]\n"
        'backend = "Webgpu"\n'
        f"custom-shader = [{shader_entries}]\n",
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
        early_status = process.poll()
        if early_status is not None:
            raise SystemExit(
                f"launched terminal exited before screenshot: {early_status}"
            )

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

        status = process.wait(timeout=max(1, int(sleep_seconds) + 3))
        if status != 0:
            raise SystemExit(f"launched terminal exited after screenshot: {status}")

        print(shot.stdout.strip())
        return 0
    except BaseException:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        raise


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
    shader_paths = args.shader or ["conformance/shaders/ghostty_cursor_probe.glsl"]
    ensure_shader_config(shader_config, shader_paths)

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

    keyboard_list_parser = subcommands.add_parser(
        "keyboard-list",
        help="List Kitty keyboard black-box comparison cases",
    )
    keyboard_list_parser.set_defaults(func=command_keyboard_list)

    keyboard_capture_parser = subcommands.add_parser(
        "keyboard-capture",
        help="Capture Kitty keyboard bytes from the terminal running this command",
    )
    keyboard_capture_parser.add_argument("--terminal", required=True)
    keyboard_capture_parser.add_argument(
        "--case",
        action="append",
        help="Case id to capture; repeat to capture multiple ids",
    )
    keyboard_capture_parser.add_argument("--output")
    keyboard_capture_parser.add_argument("--timeout", default=4.0, type=float)
    keyboard_capture_parser.add_argument("--idle-timeout", default=0.35, type=float)
    keyboard_capture_parser.set_defaults(func=command_keyboard_capture)

    keyboard_verify_parser = subcommands.add_parser(
        "keyboard-verify-capture",
        help="Verify a keyboard capture JSON report against checked-in expectations",
    )
    keyboard_verify_parser.add_argument("capture")
    keyboard_verify_parser.add_argument("--require-all", action="store_true")
    keyboard_verify_parser.set_defaults(func=command_keyboard_verify_capture)

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
    shader_shot_parser.add_argument(
        "--shader",
        action="append",
        help="Shader path to load; repeat for a Ghostty-style shader chain",
    )
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

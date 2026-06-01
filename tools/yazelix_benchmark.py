#!/usr/bin/env python3
"""Benchmark harnesses for the Yazelix terminal experiment."""

from __future__ import annotations

import argparse
import csv
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ARTIFACT_ROOT = ROOT / "artifacts" / "benchmarks" / "frame_time"
DEFAULT_SHADER = ROOT / "conformance" / "shaders" / "ghostty_cursor_probe.glsl"
YAZELIX_SHADER_DIR = ROOT / "misc" / "yazelix_terminal_shaders"
WGPU_CONFIG_TEMPLATES = {
    "wgpu",
    "wgpu-no-effects",
    "wgpu-shader",
    "wgpu-shader-event",
    "yzt-default",
    "yzt-default-game",
}


def default_terminal() -> Path:
    release = ROOT / "target" / "release" / "rio"
    if release.exists():
        return release
    return ROOT / "target" / "debug" / "rio"


def timestamp_name() -> str:
    return time.strftime("%Y_%m_%d_%H%M%S")


def json_string(value: str) -> str:
    return json.dumps(value)


def shader_list(paths: list[Path]) -> str:
    return "[\n  " + ",\n  ".join(json_string(str(path)) for path in paths) + "\n]"


def yazelix_default_shader_paths() -> list[Path]:
    return [
        YAZELIX_SHADER_DIR / "cursor_trail_dusk.glsl",
        YAZELIX_SHADER_DIR / "generated_effects" / "sweep.glsl",
        YAZELIX_SHADER_DIR / "generated_effects" / "rectangle_boom.glsl",
    ]


def write_rio_config(config_home: Path, template: str) -> Path | None:
    if template == "host":
        return None

    config_home.mkdir(parents=True, exist_ok=True)
    config = config_home / "config.toml"
    if template == "cpu":
        text = "[renderer]\nuse-cpu = true\n"
    elif template in {"wgpu", "wgpu-no-effects"}:
        text = '[renderer]\nbackend = "Webgpu"\n'
    elif template in {"wgpu-shader", "wgpu-shader-event"}:
        strategy = 'strategy = "game"\n' if template == "wgpu-shader" else ""
        text = (
            '[renderer]\n'
            'backend = "Webgpu"\n'
            f"{strategy}"
            f"custom-shader = [{json_string(str(DEFAULT_SHADER))}]\n"
        )
    elif template in {"yzt-default", "yzt-default-game"}:
        strategy = 'strategy = "game"\n' if template == "yzt-default-game" else ""
        text = (
            '[renderer]\n'
            'backend = "Webgpu"\n'
            f"{strategy}"
            f"custom-shader = {shader_list(yazelix_default_shader_paths())}\n"
            "\n[effects]\n"
            "trail-cursor = true\n"
        )
    else:
        raise SystemExit(f"unsupported config template: {template}")
    config.write_text(text, encoding="utf-8")
    return config


def built_in_workload(name: str, lines: int, hold_seconds: float) -> list[str]:
    if name == "scroll":
        script = (
            f"for i in $(seq 1 {lines}); do "
            "printf 'yazelix frame benchmark line %05d\\n' \"$i\"; "
            f"done; sleep {hold_seconds}"
        )
    elif name == "idle":
        script = f"printf 'yazelix frame benchmark idle\\n'; sleep {hold_seconds}"
    elif name == "kitty-graphics":
        script = (
            "python3 tools/yazelix_conformance.py emit kitty_graphics_1x1_rgba; "
            f"printf '\\nkitty graphics fixture emitted\\n'; sleep {hold_seconds}"
        )
    elif name == "sixel":
        script = (
            "python3 tools/yazelix_conformance.py emit sixel_minimal; "
            f"printf '\\nsixel fixture emitted\\n'; sleep {hold_seconds}"
        )
    elif name == "helix-jk":
        script = f"""
python3 - <<'PY'
import shutil
import sys
import time

moves = max(1, {lines})
hold_seconds = max(0.0, {hold_seconds!r})
terminal_size = shutil.get_terminal_size((80, 24))
rows = max(12, min(terminal_size.lines - 2, 48))
row = 0
direction = 1

sys.stdout.write("\\x1b[?25h\\x1b[2J\\x1b[H")
for index in range(rows):
    sys.stdout.write(
        f"{{index + 1:03d}} helix cursor churn line {{index + 1:03d}} "
        "abcdefghijklmnopqrstuvwxyz 0123456789\\r\\n"
    )
sys.stdout.write("\\x1b[H")
sys.stdout.flush()

for index in range(moves):
    if row >= rows - 2:
        direction = -1
    elif row <= 0:
        direction = 1
    row += direction
    sys.stdout.write("\\x1b[B" if direction > 0 else "\\x1b[A")
    if index % 8 == 7:
        sys.stdout.flush()
        time.sleep(0.002)

sys.stdout.flush()
time.sleep(hold_seconds)
PY
"""
    elif name == "helix-viewport":
        script = f"""
python3 - <<'PY'
import shutil
import sys
import time

moves = max(1, {lines})
hold_seconds = max(0.0, {hold_seconds!r})
terminal_size = shutil.get_terminal_size((80, 24))
viewport_rows = max(12, min(terminal_size.lines - 2, 48))
viewport_cols = max(20, min(terminal_size.columns, 120))
scroll_margin = 5 if viewport_rows >= 16 else max(2, viewport_rows // 4)
file_line_count = max(viewport_rows * 4, moves + viewport_rows)
cursor_line = 0
viewport_top = 0
direction = 1


def file_line(index):
    marker = ">" if index == cursor_line else " "
    text = (
        f"{{marker}} {{index + 1:05d}} "
        "helix viewport benchmark "
        "abcdefghijklmnopqrstuvwxyz 0123456789"
    )
    return text[: max(1, viewport_cols - 1)]


def draw_viewport():
    sys.stdout.write("\\x1b[H")
    for row in range(viewport_rows):
        sys.stdout.write(f"\\x1b[{{row + 1}};1H\\x1b[2K{{file_line(viewport_top + row)}}")
    draw_status()
    move_cursor()


def draw_status():
    status_row = viewport_rows + 1
    text = (
        f" helix-like viewport top={{viewport_top + 1}} "
        f"cursor={{cursor_line + 1}} margin={{scroll_margin}} "
    )
    sys.stdout.write(f"\\x1b[{{status_row}};1H\\x1b[7m{{text[: max(1, viewport_cols - 1)]}}\\x1b[0m")


def move_cursor():
    screen_row = cursor_line - viewport_top
    sys.stdout.write(f"\\x1b[{{screen_row + 1}};3H")


sys.stdout.write("\\x1b[?1049h\\x1b[?25h\\x1b[2J")
draw_viewport()
sys.stdout.flush()

for index in range(moves):
    old_cursor = cursor_line
    old_top = viewport_top

    if cursor_line >= file_line_count - 1:
        direction = -1
    elif cursor_line <= 0:
        direction = 1

    cursor_line += direction
    bottom_margin_line = viewport_top + viewport_rows - scroll_margin - 1
    top_margin_line = viewport_top + scroll_margin

    if cursor_line > bottom_margin_line:
        viewport_top = min(
            cursor_line - (viewport_rows - scroll_margin - 1),
            file_line_count - viewport_rows,
        )
    elif cursor_line < top_margin_line:
        viewport_top = max(cursor_line - scroll_margin, 0)

    if viewport_top != old_top:
        draw_viewport()
    else:
        for changed in sorted({{old_cursor, cursor_line}}):
            if viewport_top <= changed < viewport_top + viewport_rows:
                row = changed - viewport_top
                sys.stdout.write(f"\\x1b[{{row + 1}};1H\\x1b[2K{{file_line(changed)}}")
        draw_status()
        move_cursor()

    if index % 8 == 7:
        sys.stdout.flush()
        time.sleep(0.002)

sys.stdout.flush()
time.sleep(hold_seconds)
sys.stdout.write("\\x1b[?1049l")
sys.stdout.flush()
PY
"""
    else:
        raise SystemExit(f"unsupported workload: {name}")

    return ["bash", "--noprofile", "--norc", "-c", script]


def default_workload_lines(workload: str) -> int:
    if workload == "helix-jk":
        return 4_000
    if workload == "helix-viewport":
        return 400
    return 20_000


def read_proc_sample(pid: int, start_ns: int) -> dict[str, int] | None:
    proc = Path("/proc") / str(pid)
    try:
        stat = (proc / "stat").read_text(encoding="utf-8").split()
        status = (proc / "status").read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        return None
    except ProcessLookupError:
        return None

    rss_kb = 0
    for line in status:
        if line.startswith("VmRSS:"):
            rss_kb = int(line.split()[1])
            break

    return {
        "elapsed_ns": time.monotonic_ns() - start_ns,
        "cpu_ticks": int(stat[13]) + int(stat[14]),
        "rss_kb": rss_kb,
    }


def write_samples(path: Path, samples: list[dict[str, int]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as sample_file:
        writer = csv.DictWriter(
            sample_file,
            fieldnames=["elapsed_ns", "cpu_ticks", "rss_kb"],
        )
        writer.writeheader()
        writer.writerows(samples)


def start_gpu_sampler(
    mode: str,
    artifact_dir: Path,
) -> tuple[subprocess.Popen[bytes], Any, Any, Path] | None:
    if mode == "none":
        return None

    nvidia_smi = shutil.which("nvidia-smi")
    if nvidia_smi is None:
        if mode == "nvidia-smi":
            raise SystemExit("--gpu-sampler=nvidia-smi requested but nvidia-smi is not on PATH")
        return None

    samples_path = artifact_dir / "gpu_samples.csv"
    stderr_path = artifact_dir / "gpu_samples.stderr.log"
    stdout_file = samples_path.open("wb")
    stderr_file = stderr_path.open("wb")
    process = subprocess.Popen(
        [
            nvidia_smi,
            "--query-gpu=timestamp,index,utilization.gpu,utilization.memory,memory.used,power.draw",
            "--format=csv,nounits",
            "--loop-ms=200",
        ],
        stdout=stdout_file,
        stderr=stderr_file,
    )
    return process, stdout_file, stderr_file, samples_path


def stop_gpu_sampler(
    sampler: tuple[subprocess.Popen[bytes], Any, Any, Path] | None,
) -> Path | None:
    if sampler is None:
        return None

    process, stdout_file, stderr_file, samples_path = sampler
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=2.0)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=2.0)
    stdout_file.close()
    stderr_file.close()
    return samples_path


def read_samples(path: Path | None) -> list[dict[str, int]]:
    if path is None or not path.exists():
        return []
    with path.open("r", encoding="utf-8", newline="") as sample_file:
        return [
            {
                "elapsed_ns": int(row["elapsed_ns"]),
                "cpu_ticks": int(row["cpu_ticks"]),
                "rss_kb": int(row["rss_kb"]),
            }
            for row in csv.DictReader(sample_file)
        ]


def read_frame_log(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as frame_file:
        for line_number, line in enumerate(frame_file, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError as err:
                raise SystemExit(f"{path}:{line_number}: invalid frame JSON: {err}") from err
    return events


def percentile(sorted_values: list[float], pct: float) -> float:
    if not sorted_values:
        return 0.0
    if len(sorted_values) == 1:
        return sorted_values[0]
    rank = (len(sorted_values) - 1) * pct
    low = int(rank)
    high = min(low + 1, len(sorted_values) - 1)
    weight = rank - low
    return sorted_values[low] * (1.0 - weight) + sorted_values[high] * weight


def stats_ms(ns_values: list[int]) -> dict[str, float | int]:
    values = sorted(value / 1_000_000.0 for value in ns_values)
    if not values:
        return {"count": 0}
    return {
        "count": len(values),
        "min": values[0],
        "mean": sum(values) / len(values),
        "median": percentile(values, 0.50),
        "p95": percentile(values, 0.95),
        "p99": percentile(values, 0.99),
        "max": values[-1],
    }


def stats_values(values: list[int]) -> dict[str, float | int]:
    sorted_values = sorted(float(value) for value in values)
    if not sorted_values:
        return {"count": 0}
    return {
        "count": len(sorted_values),
        "min": sorted_values[0],
        "mean": sum(sorted_values) / len(sorted_values),
        "median": percentile(sorted_values, 0.50),
        "p95": percentile(sorted_values, 0.95),
        "p99": percentile(sorted_values, 0.99),
        "max": sorted_values[-1],
        "total": sum(sorted_values),
    }


def frame_deltas(events: list[dict[str, Any]]) -> list[int]:
    deltas: list[int] = []
    previous_end: int | None = None
    for event in events:
        end = int(event["end_elapsed_ns"])
        if previous_end is not None:
            deltas.append(max(0, end - previous_end))
        previous_end = end
    return deltas


def summarize_proc(samples: list[dict[str, int]]) -> dict[str, float | int | None]:
    if not samples:
        return {
            "sample_count": 0,
            "wall_seconds": None,
            "cpu_seconds": None,
            "cpu_percent": None,
            "max_rss_kb": None,
        }

    first = samples[0]
    last = samples[-1]
    ticks_per_second = os.sysconf(os.sysconf_names["SC_CLK_TCK"])
    wall_seconds = max(0.0, (last["elapsed_ns"] - first["elapsed_ns"]) / 1_000_000_000.0)
    cpu_seconds = max(0.0, (last["cpu_ticks"] - first["cpu_ticks"]) / ticks_per_second)
    cpu_percent = None if wall_seconds == 0.0 else (cpu_seconds / wall_seconds) * 100.0
    return {
        "sample_count": len(samples),
        "wall_seconds": wall_seconds,
        "cpu_seconds": cpu_seconds,
        "cpu_percent": cpu_percent,
        "max_rss_kb": max(sample["rss_kb"] for sample in samples),
    }


def summarize(events: list[dict[str, Any]], samples: list[dict[str, int]]) -> dict[str, Any]:
    redraws = [event for event in events if event.get("event") == "redraw"]
    presented = [event for event in redraws if event.get("presented")]
    first_redraw = redraws[0] if redraws else None
    first_presented = presented[0] if presented else None
    vblank_values = [
        int(event["vblank_interval_ns"])
        for event in redraws
        if int(event.get("vblank_interval_ns", 0)) > 0
    ]

    summary = {
        "schema": 1,
        "frame_count": len(redraws),
        "presented_frame_count": len(presented),
        "first_redraw_ms": None
        if first_redraw is None
        else int(first_redraw["end_elapsed_ns"]) / 1_000_000.0,
        "first_presented_frame_ms": None
        if first_presented is None
        else int(first_presented["end_elapsed_ns"]) / 1_000_000.0,
        "redraw_delta_ms": stats_ms(frame_deltas(redraws)),
        "presented_delta_ms": stats_ms(frame_deltas(presented)),
        "render_duration_ms": stats_ms(
            [int(event["render_duration_ns"]) for event in redraws]
        ),
        "target_vblank_ms": stats_ms(vblank_values),
        "process": summarize_proc(samples),
    }

    phase_fields = [
        ("renderer_run_duration_ms", "renderer_run_duration_ns"),
        ("terminal_lock_wait_duration_ms", "terminal_lock_wait_duration_ns"),
        ("terminal_snapshot_duration_ms", "terminal_snapshot_duration_ns"),
        ("snapshot_visible_duration_ms", "snapshot_visible_duration_ns"),
        ("panel_collect_duration_ms", "panel_collect_duration_ns"),
        ("grid_emit_duration_ms", "grid_emit_duration_ns"),
        ("sugarloaf_render_duration_ms", "sugarloaf_render_duration_ns"),
    ]
    for output_name, field_name in phase_fields:
        values = [int(event[field_name]) for event in redraws if field_name in event]
        if values:
            summary[output_name] = stats_ms(values)

    count_fields = [
        "panel_count",
        "visible_row_count",
        "row_rebuild_count",
        "full_row_rebuild_count",
        "dirty_row_rebuild_count",
        "terminal_lock_busy_count",
    ]
    for field_name in count_fields:
        values = [int(event[field_name]) for event in redraws if field_name in event]
        if values:
            summary[field_name] = stats_values(values)

    return summary


def flatten_summary(summary: dict[str, Any], prefix: str = "") -> list[tuple[str, Any]]:
    rows: list[tuple[str, Any]] = []
    for key, value in summary.items():
        name = f"{prefix}.{key}" if prefix else key
        if isinstance(value, dict):
            rows.extend(flatten_summary(value, name))
        else:
            rows.append((name, value))
    return rows


def write_summary(summary: dict[str, Any], json_path: Path, csv_path: Path) -> None:
    json_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with csv_path.open("w", encoding="utf-8", newline="") as csv_file:
        writer = csv.writer(csv_file)
        writer.writerow(["metric", "value"])
        writer.writerows(flatten_summary(summary))


def child_command_from_args(args: argparse.Namespace) -> list[str]:
    if args.command:
        command = list(args.command)
        if command and command[0] == "--":
            command = command[1:]
        if not command:
            raise SystemExit("--command requires argv after --")
        return command
    lines = args.lines if args.lines is not None else default_workload_lines(args.workload)
    return built_in_workload(args.workload, lines, args.hold_seconds)


def command_frame_run(args: argparse.Namespace) -> int:
    terminal = Path(args.terminal).expanduser()
    if not terminal.exists():
        raise SystemExit(f"terminal binary does not exist: {terminal}")

    artifact_dir = Path(args.artifact_dir or DEFAULT_ARTIFACT_ROOT / timestamp_name())
    if not artifact_dir.is_absolute():
        artifact_dir = ROOT / artifact_dir
    artifact_dir.mkdir(parents=True, exist_ok=True)
    frame_log = artifact_dir / "frame_log.jsonl"
    samples_csv = artifact_dir / "proc_samples.csv"
    stdout_log = artifact_dir / "stdout.log"
    stderr_log = artifact_dir / "stderr.log"
    config_home = artifact_dir / "rio_config_home"
    config_path = write_rio_config(config_home, args.config_template)

    env = os.environ.copy()
    env["YAZELIX_TERMINAL_FRAME_LOG"] = str(frame_log)
    if config_path is not None:
        env["RIO_CONFIG_HOME"] = str(config_home)
    if args.config_template in WGPU_CONFIG_TEMPLATES:
        env.setdefault("WGPU_BACKEND", "vulkan")
    for item in args.env:
        if "=" not in item:
            raise SystemExit(f"--env requires NAME=VALUE, got: {item}")
        name, value = item.split("=", 1)
        env[name] = value

    terminal_argv = [
        str(terminal),
        *args.terminal_arg,
        "-e",
        *child_command_from_args(args),
    ]
    gpu_sampler = start_gpu_sampler(args.gpu_sampler, artifact_dir)
    start_ns = time.monotonic_ns()
    samples: list[dict[str, int]] = []
    timed_out = False
    try:
        with stdout_log.open("wb") as stdout_file, stderr_log.open("wb") as stderr_file:
            process = subprocess.Popen(
                terminal_argv,
                cwd=ROOT,
                env=env,
                stdout=stdout_file,
                stderr=stderr_file,
            )
            deadline = time.monotonic() + args.timeout
            while process.poll() is None:
                sample = read_proc_sample(process.pid, start_ns)
                if sample is not None:
                    samples.append(sample)
                if time.monotonic() >= deadline:
                    timed_out = True
                    process.terminate()
                    try:
                        process.wait(timeout=2.0)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=2.0)
                    break
                time.sleep(args.sample_interval)

            final_sample = read_proc_sample(process.pid, start_ns)
            if final_sample is not None:
                samples.append(final_sample)
            returncode = process.poll()
    finally:
        gpu_samples = stop_gpu_sampler(gpu_sampler)
    end_ns = time.monotonic_ns()

    write_samples(samples_csv, samples)
    events = read_frame_log(frame_log) if frame_log.exists() else []
    summary = summarize(events, samples)
    summary["command"] = {
        "terminal_argv": terminal_argv,
        "returncode": returncode,
        "timed_out": timed_out,
        "wall_time_seconds": (end_ns - start_ns) / 1_000_000_000.0,
        "frame_log": str(frame_log.relative_to(ROOT)),
        "proc_samples": str(samples_csv.relative_to(ROOT)),
        "gpu_samples": (
            None if gpu_samples is None else str(gpu_samples.relative_to(ROOT))
        ),
        "config_template": args.config_template,
        "config_path": None if config_path is None else str(config_path.relative_to(ROOT)),
    }
    write_summary(summary, artifact_dir / "summary.json", artifact_dir / "summary.csv")

    print(json.dumps(summary, indent=2, sort_keys=True))
    if timed_out:
        return 124
    return 0 if returncode == 0 else int(returncode or 1)


def command_frame_summary(args: argparse.Namespace) -> int:
    events = read_frame_log(Path(args.frame_log))
    samples = read_samples(Path(args.samples) if args.samples else None)
    summary = summarize(events, samples)
    if args.json_out or args.csv_out:
        json_path = Path(args.json_out or Path(args.frame_log).with_suffix(".summary.json"))
        csv_path = Path(args.csv_out or Path(args.frame_log).with_suffix(".summary.csv"))
        write_summary(summary, json_path, csv_path)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


def command_self_test(_: argparse.Namespace) -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        frame_log = Path(temp_dir) / "frame_log.jsonl"
        frame_log.write_text(
            "\n".join(
                [
                    '{"event":"benchmark_start","pid":123,"elapsed_ns":0}',
                    '{"event":"redraw","frame_index":0,"window_id":"WindowId(1)",'
                    '"route":"terminal","presented":true,"dirty_after":false,'
                    '"game_mode":false,"start_elapsed_ns":9000000,'
                    '"end_elapsed_ns":10000000,"render_duration_ns":1000000,'
                    '"delta_ns":0,"vblank_interval_ns":16666000}',
                    '{"event":"redraw","frame_index":1,"window_id":"WindowId(1)",'
                    '"route":"terminal","presented":false,"dirty_after":true,'
                    '"game_mode":false,"start_elapsed_ns":25000000,'
                    '"end_elapsed_ns":26000000,"render_duration_ns":1000000,'
                    '"delta_ns":16000000,"vblank_interval_ns":16666000}',
                    '{"event":"redraw","frame_index":2,"window_id":"WindowId(1)",'
                    '"route":"terminal","presented":true,"dirty_after":false,'
                    '"game_mode":false,"start_elapsed_ns":42000000,'
                    '"end_elapsed_ns":43000000,"render_duration_ns":1000000,'
                    '"delta_ns":17000000,"vblank_interval_ns":16666000}',
                    "",
                ]
            ),
            encoding="utf-8",
        )
        summary = summarize(read_frame_log(frame_log), [])
        assert summary["frame_count"] == 3
        assert summary["presented_frame_count"] == 2
        assert summary["first_redraw_ms"] == 10.0
        assert summary["first_presented_frame_ms"] == 10.0
        assert summary["redraw_delta_ms"]["count"] == 2
        assert summary["presented_delta_ms"]["count"] == 1
        assert summary["presented_delta_ms"]["max"] == 33.0
        config_home = Path(temp_dir) / "rio_config_home"
        config = write_rio_config(config_home, "yzt-default")
        assert config is not None
        config_text = config.read_text(encoding="utf-8")
        assert "cursor_trail_dusk.glsl" in config_text
        assert "trail-cursor = true" in config_text
        baseline_config = write_rio_config(config_home, "wgpu-no-effects")
        assert baseline_config is not None
        baseline_text = baseline_config.read_text(encoding="utf-8")
        assert "custom-shader" not in baseline_text
        assert "trail-cursor" not in baseline_text
        assert child_command_from_args(
            argparse.Namespace(command=[], workload="helix-jk", lines=16, hold_seconds=0.01)
        )
        assert child_command_from_args(
            argparse.Namespace(
                command=[],
                workload="helix-viewport",
                lines=16,
                hold_seconds=0.01,
            )
        )
    print("ok yazelix_benchmark self-test")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command_name", required=True)

    run = subcommands.add_parser(
        "frame-run",
        help="run a terminal with Rio frame logging enabled when supported",
    )
    run.add_argument("--terminal", default=str(default_terminal()))
    run.add_argument(
        "--terminal-arg",
        action="append",
        default=[],
        help="terminal argv item before -e; use --terminal-arg=--flag for flag values",
    )
    run.add_argument("--artifact-dir")
    run.add_argument(
        "--config-template",
        choices=[
            "host",
            "cpu",
            "wgpu",
            "wgpu-no-effects",
            "wgpu-shader",
            "wgpu-shader-event",
            "yzt-default",
            "yzt-default-game",
        ],
        default="wgpu",
    )
    run.add_argument(
        "--workload",
        choices=[
            "scroll",
            "idle",
            "kitty-graphics",
            "sixel",
            "helix-jk",
            "helix-viewport",
        ],
        default="scroll",
    )
    run.add_argument(
        "--lines",
        type=int,
        help="workload iteration count; defaults to 20000 for scroll-like workloads, 4000 for helix-jk, and 400 for helix-viewport",
    )
    run.add_argument("--hold-seconds", type=float, default=0.2)
    run.add_argument("--sample-interval", type=float, default=0.05)
    run.add_argument("--timeout", type=float, default=15.0)
    run.add_argument("--env", action="append", default=[])
    run.add_argument(
        "--gpu-sampler",
        choices=["auto", "none", "nvidia-smi"],
        default="auto",
        help="optional GPU sampler; auto uses nvidia-smi when present",
    )
    run.add_argument(
        "command",
        nargs=argparse.REMAINDER,
        help="optional child command after --, passed to rio -e",
    )
    run.set_defaults(func=command_frame_run)

    summary = subcommands.add_parser("frame-summary", help="summarize a frame log")
    summary.add_argument("frame_log")
    summary.add_argument("--samples")
    summary.add_argument("--json-out")
    summary.add_argument("--csv-out")
    summary.set_defaults(func=command_frame_summary)

    self_test = subcommands.add_parser("self-test", help="run harness unit checks")
    self_test.set_defaults(func=command_self_test)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())

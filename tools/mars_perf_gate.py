#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import random
import re
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import TextIO


MARS_PROCESS_NAMES = {"mars", "mars-desktop"}
MARS_APP_ID_PATTERN = re.compile(r"(^| )--app-id(=| )mars( |$)")
DEFAULT_REPRO_SCENARIOS = (
    "idle",
    "pty_flood",
    "scroll_render",
    "yzx_screen_mandelbrot",
)
CORPUS_GENERATOR_VERSION = 1
CORPUS_KINDS = ("pty_flood", "scroll_render", "utf8", "osc_control", "sync_bursts")
ENV_PREFIXES = (
    "MARS",
    "RIO_CONFIG_HOME",
    "TERM",
    "TERM_PROGRAM",
    "ZELLIJ",
    "YAZELIX",
    "XDG_SESSION_TYPE",
    "WAYLAND_DISPLAY",
    "DISPLAY",
)


@dataclass(frozen=True)
class Scenario:
    name: str
    description: str


@dataclass
class MeasurementJob:
    name: str
    process: subprocess.Popen[str]
    output: TextIO


SCENARIOS = {
    "idle": Scenario(
        name="idle",
        description="Open Mars with a quiet Python process and sample idle terminal cost.",
    ),
    "pty_flood": Scenario(
        name="pty_flood",
        description="Print a fixed number of high-volume lines through Mars.",
    ),
    "scroll_render": Scenario(
        name="scroll_render",
        description="Print a fixed long scrollback stream with varied line widths.",
    ),
    "yzx_screen_mandelbrot": Scenario(
        name="yzx_screen_mandelbrot",
        description="Run `yzx screen mandelbrot` for a bounded duration when yzx is available.",
    ),
    "corpus_replay": Scenario(
        name="corpus_replay",
        description="Replay an existing deterministic terminal input corpus through Mars.",
    ),
}


def utc_timestamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def sample_timestamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")


def run_capture(argv: list[str]) -> str:
    result = subprocess.run(argv, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return result.stdout


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def corpus_metadata_path(corpus: Path) -> Path:
    return corpus.with_name(f"{corpus.name}.json")


def read_corpus_metadata(corpus: Path) -> dict[str, object]:
    metadata_path = corpus_metadata_path(corpus)
    if not metadata_path.exists():
        return {}
    try:
        parsed = json.loads(metadata_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    return parsed if isinstance(parsed, dict) else {}


def corpus_manifest_lines(corpus: Path, run_dir: Path) -> list[str]:
    metadata = read_corpus_metadata(corpus)
    metadata_path = corpus_metadata_path(corpus)
    copied_metadata = run_dir / "corpus_metadata.json"
    copied_metadata.write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    lines = [
        f"corpus_path={corpus}",
        f"corpus_metadata_path={metadata_path if metadata_path.exists() else ''}",
        f"corpus_metadata_artifact={copied_metadata}",
        f"corpus_size_bytes={corpus.stat().st_size}",
        f"corpus_sha256={file_sha256(corpus)}",
    ]
    for key in [
        "generator_version",
        "kind",
        "seed",
        "rows",
        "columns",
        "line_count",
        "byte_count",
        "sha256",
    ]:
        lines.append(f"corpus_{key}={metadata.get(key, '')}")
    return lines


def command_exists(command: str) -> bool:
    return shutil.which(command) is not None


def is_mars_process(command: str) -> bool:
    argv0 = command.split(maxsplit=1)[0] if command.strip() else ""
    return (
        Path(argv0).name in MARS_PROCESS_NAMES
        or MARS_APP_ID_PATTERN.search(command) is not None
    )


def find_mars_pid() -> int | None:
    output = run_capture(["pgrep", "-af", "mars|mars-desktop|--app-id[ =]mars"])
    matches: list[int] = []
    for line in output.splitlines():
        fields = line.split(maxsplit=1)
        if not fields:
            continue
        pid = int(fields[0])
        if pid == os.getpid():
            continue
        command = fields[1] if len(fields) > 1 else ""
        if is_mars_process(command):
            matches.append(pid)
    return matches[-1] if matches else None


def assert_process(pid: int) -> None:
    try:
        os.kill(pid, 0)
    except OSError as exc:
        raise SystemExit(f"process is not running: {pid}") from exc


def terminate_process_group(process: subprocess.Popen[str]) -> None:
    process_group = process.pid
    try:
        os.killpg(process_group, signal.SIGTERM)
    except ProcessLookupError:
        return

    if process.poll() is None:
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process_group, signal.SIGKILL)
            except ProcessLookupError:
                return
            process.wait(timeout=3)

    time.sleep(0.2)
    try:
        os.killpg(process_group, signal.SIGKILL)
    except ProcessLookupError:
        pass


def safe_label(raw: str) -> str:
    value = re.sub(r"[^A-Za-z0-9_.-]+", "_", raw).strip("_")
    return value or "manual"


def corpus_line(kind: str, index: int, columns: int, rng: random.Random) -> bytes:
    if kind == "pty_flood":
        payload = "".join(rng.choice("abcdef0123456789") for _ in range(max(columns, 16)))
        return f"mars_corpus_pty {index:06d} {payload}\n".encode()
    if kind == "scroll_render":
        width = 1 + (index % 10)
        return f"mars_corpus_scroll {index:06d} {'0123456789abcdef' * width}\n".encode()
    if kind == "utf8":
        words = ["Yazelix", "Mars", "Rio", "cursor", "render", "ação", "東京", "λ", "✓"]
        payload = " ".join(rng.choice(words) for _ in range(max(4, columns // 12)))
        return f"mars_corpus_utf8 {index:06d} {payload}\n".encode()
    if kind == "osc_control":
        color = rng.choice(["31", "32", "33", "34", "35", "36"])
        title = f"mars-corpus-{index:06d}"
        return f"\x1b]0;{title}\x07\x1b[{color}mmars_corpus_osc {index:06d}\x1b[0m\n".encode()
    if kind == "sync_bursts":
        body = f"mars_corpus_sync {index:06d} {'#' * (1 + index % max(columns // 2, 1))}\n"
        return f"\x1b[?2026h{body}\x1b[?2026l".encode()
    raise ValueError(f"unknown corpus kind: {kind}")


def generate_corpus(args: argparse.Namespace) -> int:
    corpus = Path(args.generate_corpus).expanduser()
    corpus.parent.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.corpus_seed)

    with corpus.open("wb") as handle:
        if args.corpus_kind == "scroll_render":
            handle.write(b"\x1b[2J\x1b[H")
        for index in range(args.corpus_lines):
            handle.write(corpus_line(args.corpus_kind, index, args.corpus_columns, rng))

    metadata = {
        "generator": "mars_perf_gate.py",
        "generator_version": CORPUS_GENERATOR_VERSION,
        "kind": args.corpus_kind,
        "seed": args.corpus_seed,
        "rows": args.corpus_rows,
        "columns": args.corpus_columns,
        "line_count": args.corpus_lines,
        "byte_count": corpus.stat().st_size,
        "sha256": file_sha256(corpus),
    }
    metadata_path = corpus_metadata_path(corpus)
    metadata_path.write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(corpus)
    print(metadata_path)
    return 0


def write_context(
    path: Path,
    label: str,
    seconds: int,
    pid: int | None,
    extra_lines: list[str] | None = None,
) -> None:
    lines = [
        f"timestamp_utc={utc_timestamp()}",
        f"label={label}",
        f"seconds={seconds}",
        f"pid={pid if pid is not None else ''}",
        f"repo={Path.cwd()}",
        f"git_branch={run_capture(['git', 'branch', '--show-current']).strip()}",
        f"git_head={run_capture(['git', 'rev-parse', 'HEAD']).strip()}",
        f"uname={run_capture(['uname', '-a']).strip()}",
        f"pidstat_available={'yes' if command_exists('pidstat') else 'no'}",
        f"perf_available={'yes' if command_exists('perf') else 'no'}",
    ]

    for key, value in sorted(os.environ.items()):
        if any(key == prefix or key.startswith(f"{prefix}_") for prefix in ENV_PREFIXES):
            lines.append(f"{key}={value}")

    if extra_lines:
        lines.append("")
        lines.append("run_context:")
        lines.extend(extra_lines)

    lines.append("")
    lines.append("matching_processes:")
    lines.append(run_capture(["pgrep", "-af", "mars|rio|zellij|codex|yzx"]).rstrip())
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def append_process_sample(path: Path, pid: int) -> None:
    output = run_capture(
        [
            "ps",
            "-p",
            str(pid),
            "-o",
            "pid=,ppid=,pcpu=,pmem=,rss=,vsz=,stat=,comm=,args=",
        ]
    )
    now = sample_timestamp()
    with path.open("a", encoding="utf-8") as handle:
        for line in output.splitlines():
            if line.strip():
                handle.write(f"{now}\t{line.strip()}\n")


def append_thread_sample(path: Path, pid: int) -> None:
    output = run_capture(["ps", "-L", "-p", str(pid), "-o", "pid=,tid=,pcpu=,pmem=,comm="])
    now = sample_timestamp()
    with path.open("a", encoding="utf-8") as handle:
        for line in output.splitlines():
            if line.strip():
                handle.write(f"{now}\t{line.strip()}\n")


def start_pidstat(pid: int, seconds: int, run_dir: Path) -> list[MeasurementJob]:
    if not command_exists("pidstat"):
        (run_dir / "pidstat_missing.txt").write_text(
            "pidstat is not available; ps samples are the fallback.\n",
            encoding="utf-8",
        )
        return []

    jobs: list[MeasurementJob] = []
    specs = [
        (["pidstat", "-u", "-r", "-h", "-p", str(pid), "1", str(seconds)], "pidstat.txt"),
        (["pidstat", "-t", "-u", "-h", "-p", str(pid), "1", str(seconds)], "pidstat_threads.txt"),
    ]
    for argv, filename in specs:
        target = (run_dir / filename).open("w", encoding="utf-8")
        process = subprocess.Popen(argv, stdout=target, stderr=subprocess.STDOUT, text=True)
        jobs.append(MeasurementJob(name=filename, process=process, output=target))
    return jobs


def start_perf_stat(pid: int, seconds: int, run_dir: Path, enabled: bool) -> list[MeasurementJob]:
    if not enabled:
        return []
    if not command_exists("perf"):
        (run_dir / "perf_stat_skipped.txt").write_text(
            "perf is not available on PATH.\n",
            encoding="utf-8",
        )
        return []

    output_path = run_dir / "perf_stat.txt"
    argv = ["perf", "stat", "-p", str(pid), "-o", str(output_path), "--", "sleep", str(seconds)]
    target = (run_dir / "perf_stat_driver.log").open("w", encoding="utf-8")
    process = subprocess.Popen(argv, stdout=target, stderr=subprocess.STDOUT, text=True)
    return [MeasurementJob(name="perf_stat", process=process, output=target)]


def wait_measurement_jobs(jobs: list[MeasurementJob]) -> None:
    for job in jobs:
        try:
            job.process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            job.process.send_signal(signal.SIGTERM)
            try:
                job.process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                job.process.kill()
                job.process.wait(timeout=2)
        finally:
            job.output.close()


def read_process_cpu(samples: Path) -> tuple[float, float, int]:
    values: list[float] = []
    for line in samples.read_text(encoding="utf-8").splitlines()[1:]:
        fields = line.split()
        if len(fields) < 4:
            continue
        try:
            values.append(float(fields[3]))
        except ValueError:
            continue

    if not values:
        return 0.0, 0.0, 0
    return sum(values) / len(values), max(values), len(values)


def read_phase(phase_file: Path | None) -> str:
    if phase_file is None or not phase_file.exists():
        return ""
    return phase_file.read_text(encoding="utf-8").strip()


def write_summary(
    path: Path,
    run_dir: Path,
    label: str,
    seconds: int,
    pid: int,
    samples: Path,
    threads: Path,
    phase_file: Path | None = None,
) -> None:
    last_sample = ""
    sample_lines = samples.read_text(encoding="utf-8").splitlines()
    if len(sample_lines) > 1:
        last_sample = sample_lines[-1]
    average_cpu, max_cpu, process_sample_count = read_process_cpu(samples)

    thread_sample_lines = threads.read_text(encoding="utf-8").splitlines()[1:]
    last_thread_sample = ""
    for line in reversed(thread_sample_lines):
        fields = line.split()
        if fields:
            last_thread_sample = fields[0]
            break

    thread_rows = []
    for line in thread_sample_lines:
        fields = line.split()
        if len(fields) >= 5 and fields[0] == last_thread_sample:
            try:
                cpu = float(fields[3])
            except ValueError:
                cpu = 0.0
            thread_rows.append((cpu, line))
    thread_rows.sort(reverse=True)

    lines = [
        "Mars performance gate sample",
        f"artifact_dir={run_dir}",
        f"label={label}",
        f"seconds={seconds}",
        f"pid={pid}",
        f"primary_sampler={'pidstat' if (run_dir / 'pidstat.txt').exists() else 'ps_fallback'}",
        f"pidstat_process={(run_dir / 'pidstat.txt').exists()}",
        f"pidstat_threads={(run_dir / 'pidstat_threads.txt').exists()}",
        f"perf_stat={(run_dir / 'perf_stat.txt').exists()}",
        f"ps_process_sample_count={process_sample_count}",
        f"ps_process_cpu_average={average_cpu:.2f}",
        f"ps_process_cpu_max={max_cpu:.2f}",
        f"workload_phase={read_phase(phase_file)}",
        "",
        "last_process_sample:",
        last_sample,
        "",
        "top_threads_last_sample:",
    ]
    lines.extend(row for _, row in thread_rows[:10])
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def sample_process(
    label: str,
    seconds: int,
    pid: int,
    run_dir: Path,
    extra_context: list[str],
    phase_file: Path | None = None,
    perf_stat: bool = False,
) -> Path:
    assert_process(pid)

    context = run_dir / "context.txt"
    samples = run_dir / "process_samples.tsv"
    threads = run_dir / "thread_samples.tsv"
    summary = run_dir / "summary.txt"

    write_context(context, label, seconds, pid, extra_context)
    samples.write_text("sample_utc\tpid\tppid\tpcpu\tpmem\trss\tvsz\tstat\tcomm\targs\n", encoding="utf-8")
    threads.write_text("sample_utc\tpid\ttid\tpcpu\tpmem\tcomm\n", encoding="utf-8")

    jobs = start_pidstat(pid, seconds, run_dir)
    jobs.extend(start_perf_stat(pid, seconds, run_dir, perf_stat))
    for _ in range(seconds):
        append_process_sample(samples, pid)
        append_thread_sample(threads, pid)
        time.sleep(1)

    wait_measurement_jobs(jobs)

    write_summary(summary, run_dir, label, seconds, pid, samples, threads, phase_file)
    return summary


def write_repro_config(config_home: Path, title: str) -> None:
    config_home.mkdir(parents=True, exist_ok=True)
    (config_home / "config.toml").write_text(
        "\n".join(
            [
                "confirm-before-quit = false",
                "",
                "[window]",
                f'title-placeholder = "{title}"',
                "",
            ]
        ),
        encoding="utf-8",
    )


def write_workload_script(
    path: Path,
    phase_file: Path,
    scenario: str,
    sample_seconds: int,
    pty_lines: int,
    scroll_lines: int,
    cooldown_seconds: int,
    corpus: Path | None = None,
) -> None:
    hold_seconds = sample_seconds + cooldown_seconds + 2
    lines = [
        "#!/usr/bin/env python3",
        "import shutil",
        "import subprocess",
        "import sys",
        "import time",
        "from pathlib import Path",
        "",
        f"phase_file = Path({str(phase_file)!r})",
        "",
        "def mark_phase(value):",
        "    phase_file.write_text(value + '\\n', encoding='utf-8')",
        "",
        "def flush_print(value=''):",
        "    print(value)",
        "    sys.stdout.flush()",
        "",
        f"scenario = {scenario!r}",
        f"hold_seconds = {hold_seconds}",
        f"pty_lines = {pty_lines}",
        f"scroll_lines = {scroll_lines}",
        f"sample_seconds = {sample_seconds}",
        f"corpus_path = Path({str(corpus)!r}) if {corpus is not None!r} else None",
        "",
        "mark_phase('started')",
        "flush_print(f'mars perf workload start: {scenario}')",
    ]

    if scenario == "idle":
        lines.extend(
            [
                "mark_phase('holding')",
                "time.sleep(hold_seconds)",
            ]
        )
    elif scenario == "pty_flood":
        lines.extend(
            [
                "payload = 'abcdef0123456789' * 8",
                "for index in range(pty_lines):",
                "    print(f'mars_perf_pty {index:06d} {payload}')",
                "    if index % 1000 == 0:",
                "        sys.stdout.flush()",
                "sys.stdout.flush()",
                "mark_phase('output_done')",
                "time.sleep(hold_seconds)",
            ]
        )
    elif scenario == "scroll_render":
        lines.extend(
            [
                "print('\\x1b[2J\\x1b[H', end='')",
                "for index in range(scroll_lines):",
                "    width = (index % 10) + 1",
                "    print(f'mars_perf_scroll {index:06d} ' + ('0123456789abcdef' * width))",
                "    if index % 1000 == 0:",
                "        sys.stdout.flush()",
                "sys.stdout.flush()",
                "mark_phase('output_done')",
                "time.sleep(hold_seconds)",
            ]
        )
    elif scenario == "yzx_screen_mandelbrot":
        lines.extend(
            [
                "if shutil.which('yzx') is None:",
                "    flush_print('skip: yzx is not on PATH')",
                "else:",
                "    try:",
                "        subprocess.run(['yzx', 'screen', 'mandelbrot'], timeout=sample_seconds, check=False)",
                "    except subprocess.TimeoutExpired:",
                "        flush_print('bounded yzx screen run reached timeout')",
                "    mark_phase('screen_done')",
                "time.sleep(hold_seconds)",
            ]
        )
    elif scenario == "corpus_replay":
        lines.extend(
            [
                "if corpus_path is None:",
                "    raise SystemExit('corpus_replay requires a corpus path')",
                "with corpus_path.open('rb') as handle:",
                "    while True:",
                "        chunk = handle.read(65536)",
                "        if not chunk:",
                "            break",
                "        sys.stdout.buffer.write(chunk)",
                "    sys.stdout.buffer.flush()",
                "mark_phase('output_done')",
                "time.sleep(hold_seconds)",
            ]
        )
    else:
        raise ValueError(f"unknown scenario: {scenario}")

    lines.extend(
        [
            "mark_phase('done')",
            "flush_print(f'mars perf workload done: {scenario}')",
        ]
    )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    path.chmod(0o755)


def launch_mars(
    mars_binary: str,
    config_home: Path,
    workload: Path,
    run_dir: Path,
) -> subprocess.Popen[str]:
    env = os.environ.copy()
    env["MARS_CONFIG_HOME"] = str(config_home)
    stdout_file = (run_dir / "mars_stdout.log").open("w", encoding="utf-8")
    stderr_file = (run_dir / "mars_stderr.log").open("w", encoding="utf-8")
    try:
        return subprocess.Popen(
            [mars_binary, "-e", sys.executable, str(workload)],
            env=env,
            stdout=stdout_file,
            stderr=stderr_file,
            text=True,
            start_new_session=True,
        )
    finally:
        stdout_file.close()
        stderr_file.close()


def scenario_sample_seconds(scenario: str, args: argparse.Namespace) -> int:
    if scenario == "idle":
        return args.seconds
    return args.seconds + args.cooldown_seconds


def run_repro_suite(args: argparse.Namespace) -> int:
    if shutil.which(args.mars_binary) is None:
        print(f"Mars binary not found or not executable: {args.mars_binary}", file=sys.stderr)
        return 2

    scenarios = args.scenario or list(DEFAULT_REPRO_SCENARIOS)
    unknown = [scenario for scenario in scenarios if scenario not in SCENARIOS]
    if unknown:
        print(f"unknown scenarios: {', '.join(unknown)}", file=sys.stderr)
        return 2
    corpus = Path(args.corpus).expanduser() if args.corpus else None
    if "corpus_replay" in scenarios:
        if corpus is None:
            print("corpus_replay requires --corpus PATH", file=sys.stderr)
            return 2
        if not corpus.is_file():
            print(f"corpus file not found: {corpus}", file=sys.stderr)
            return 2
    if len(set(scenarios)) != len(scenarios):
        print("duplicate scenarios are not allowed; use --repeat for repeated runs", file=sys.stderr)
        return 2

    root = Path(os.environ.get("MARS_PERF_ARTIFACT_DIR", "artifacts/dogfooding"))
    suite_dir = root / f"{utc_timestamp()}_repro_suite"
    suite_dir.mkdir(parents=True, exist_ok=False)
    suite_summary = suite_dir / "suite_summary.txt"

    suite_lines = [
        "Mars reproducible performance suite",
        f"artifact_dir={suite_dir}",
        f"mars_binary={args.mars_binary}",
        f"workload_seconds={args.seconds}",
        f"startup_delay={args.startup_delay}",
        f"pty_lines={args.pty_lines}",
        f"scroll_lines={args.scroll_lines}",
        f"cooldown_seconds={args.cooldown_seconds}",
        f"repeat_count={args.repeat}",
        f"primary_sampler={'pidstat' if command_exists('pidstat') else 'ps_fallback'}",
        f"perf_stat_requested={args.perf_stat}",
        f"scenarios={','.join(scenarios)}",
    ]
    if corpus is not None:
        suite_lines.extend(
            [
                f"corpus_path={corpus}",
                f"corpus_size_bytes={corpus.stat().st_size}",
                f"corpus_sha256={file_sha256(corpus)}",
            ]
        )
    suite_lines.append("")

    exit_code = 0
    for repeat_index in range(1, args.repeat + 1):
        for scenario in scenarios:
            run_label = safe_label(scenario)
            if args.repeat > 1:
                run_label = f"r{repeat_index:02d}_{run_label}"
            run_dir = suite_dir / run_label
            run_dir.mkdir()
            config_home = run_dir / "mars_config"
            phase_file = run_dir / "workload_phase.txt"
            workload = run_dir / "workload.py"
            sample_seconds = scenario_sample_seconds(scenario, args)
            write_repro_config(config_home, f"Mars perf {run_label}")
            write_workload_script(
                workload,
                phase_file,
                scenario,
                args.seconds,
                args.pty_lines,
                args.scroll_lines,
                args.cooldown_seconds,
                corpus if scenario == "corpus_replay" else None,
            )

            manifest = [
                f"scenario={scenario}",
                f"repeat_index={repeat_index}",
                f"repeat_count={args.repeat}",
                f"description={SCENARIOS[scenario].description}",
                f"mars_binary={args.mars_binary}",
                f"command={args.mars_binary} -e {sys.executable} {workload}",
                f"config_home={config_home}",
                f"workload={workload}",
                f"phase_file={phase_file}",
                f"workload_seconds={args.seconds}",
                f"sample_seconds={sample_seconds}",
                f"pty_lines={args.pty_lines}",
                f"scroll_lines={args.scroll_lines}",
                f"cooldown_seconds={args.cooldown_seconds}",
                f"perf_stat_requested={args.perf_stat}",
            ]
            if scenario == "corpus_replay" and corpus is not None:
                manifest.extend(corpus_manifest_lines(corpus, run_dir))
            (run_dir / "manifest.txt").write_text("\n".join(manifest) + "\n", encoding="utf-8")

            process = launch_mars(args.mars_binary, config_home, workload, run_dir)
            time.sleep(args.startup_delay)
            if process.poll() is not None:
                exit_code = 1
                suite_lines.extend(
                    [
                        f"{run_label}: failed to start, exit_status={process.returncode}",
                        f"  stderr={run_dir / 'mars_stderr.log'}",
                        "",
                    ]
                )
                terminate_process_group(process)
                continue

            try:
                summary = sample_process(
                    run_label,
                    sample_seconds,
                    process.pid,
                    run_dir,
                    manifest,
                    phase_file,
                    perf_stat=args.perf_stat,
                )
                phase = read_phase(phase_file)
                if process.poll() is not None:
                    exit_code = 1
                    status = f"ended early, exit_status={process.returncode}"
                else:
                    status = "ok"
                suite_lines.extend(
                    [
                        f"{run_label}: {status}",
                        f"  summary={summary}",
                        f"  workload_phase={phase}",
                        "",
                    ]
                )
            finally:
                terminate_process_group(process)

    suite_summary.write_text("\n".join(suite_lines), encoding="utf-8")
    print(suite_summary.read_text(encoding="utf-8"), end="")
    return exit_code


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Sample Mars resources or run reproducible Mars performance scenarios."
    )
    parser.add_argument("--label", default="manual")
    parser.add_argument("--seconds", type=int, default=15)
    parser.add_argument("--pid", type=int)
    parser.add_argument(
        "--suite",
        action="store_true",
        help="launch Mars and run deterministic performance scenarios",
    )
    parser.add_argument(
        "--repeat",
        type=int,
        default=1,
        help="run each suite scenario this many times",
    )
    parser.add_argument(
        "--perf-stat",
        action="store_true",
        help="also run perf stat for the sampled Mars process when perf is available",
    )
    parser.add_argument(
        "--generate-corpus",
        help="write a deterministic terminal input corpus and exit",
    )
    parser.add_argument(
        "--corpus",
        help="existing terminal input corpus to replay with --suite --scenario corpus_replay",
    )
    parser.add_argument(
        "--corpus-kind",
        choices=CORPUS_KINDS,
        default="pty_flood",
        help="corpus kind for --generate-corpus",
    )
    parser.add_argument("--corpus-seed", type=int, default=1)
    parser.add_argument("--corpus-lines", type=int, default=10000)
    parser.add_argument("--corpus-rows", type=int, default=44)
    parser.add_argument("--corpus-columns", type=int, default=132)
    parser.add_argument(
        "--scenario",
        action="append",
        choices=sorted(SCENARIOS),
        help="scenario to run in --suite mode; repeat for multiple scenarios",
    )
    parser.add_argument(
        "--mars-binary",
        default=os.environ.get("MARS_BINARY", "mars"),
        help="Mars command to launch in --suite mode",
    )
    parser.add_argument("--startup-delay", type=float, default=1.5)
    parser.add_argument("--cooldown-seconds", type=int, default=5)
    parser.add_argument("--pty-lines", type=int, default=120000)
    parser.add_argument("--scroll-lines", type=int, default=80000)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.generate_corpus:
        if args.corpus_lines < 0:
            print("corpus-lines must be >= 0", file=sys.stderr)
            return 2
        if args.corpus_rows < 1 or args.corpus_columns < 1:
            print("corpus rows/columns must be >= 1", file=sys.stderr)
            return 2
        return generate_corpus(args)
    if args.seconds < 1:
        print("seconds must be >= 1", file=sys.stderr)
        return 2
    if args.startup_delay < 0:
        print("startup-delay must be >= 0", file=sys.stderr)
        return 2
    if args.cooldown_seconds < 0:
        print("cooldown-seconds must be >= 0", file=sys.stderr)
        return 2
    if args.pty_lines < 0 or args.scroll_lines < 0:
        print("pty-lines and scroll-lines must be >= 0", file=sys.stderr)
        return 2
    if args.repeat < 1:
        print("repeat must be >= 1", file=sys.stderr)
        return 2

    if args.suite:
        return run_repro_suite(args)
    if args.repeat != 1:
        print("repeat is only supported with --suite", file=sys.stderr)
        return 2

    pid = args.pid if args.pid is not None else find_mars_pid()
    if pid is None:
        print("no running Mars process found; pass --pid PID or use --suite", file=sys.stderr)
        return 1

    root = Path(os.environ.get("MARS_PERF_ARTIFACT_DIR", "artifacts/dogfooding"))
    run_dir = root / f"{utc_timestamp()}_{safe_label(args.label)}"
    run_dir.mkdir(parents=True, exist_ok=False)

    summary = sample_process(args.label, args.seconds, pid, run_dir, [], perf_stat=args.perf_stat)
    print(summary.read_text(encoding="utf-8"), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

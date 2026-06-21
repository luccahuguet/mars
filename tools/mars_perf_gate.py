#!/usr/bin/env python3
import argparse
import os
import re
import shutil
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


PROCESS_PATTERN = re.compile(r"(^|/)(mars|mars-desktop)( |$)")
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


def utc_timestamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def sample_timestamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def run_capture(argv: list[str]) -> str:
    result = subprocess.run(argv, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return result.stdout


def command_exists(command: str) -> bool:
    return shutil.which(command) is not None


def find_mars_pid() -> int | None:
    output = run_capture(["pgrep", "-af", "mars|mars-desktop"])
    matches: list[int] = []
    for line in output.splitlines():
        fields = line.split(maxsplit=1)
        if not fields:
            continue
        command = fields[1] if len(fields) > 1 else ""
        if PROCESS_PATTERN.search(command):
            matches.append(int(fields[0]))
    return matches[-1] if matches else None


def assert_process(pid: int) -> None:
    try:
        os.kill(pid, 0)
    except OSError as exc:
        raise SystemExit(f"process is not running: {pid}") from exc


def safe_label(raw: str) -> str:
    value = re.sub(r"[^A-Za-z0-9_.-]+", "_", raw).strip("_")
    return value or "manual"


def write_context(path: Path, label: str, seconds: int, pid: int) -> None:
    lines = [
        f"timestamp_utc={utc_timestamp()}",
        f"label={label}",
        f"seconds={seconds}",
        f"pid={pid}",
        f"repo={Path.cwd()}",
        f"git_branch={run_capture(['git', 'branch', '--show-current']).strip()}",
        f"git_head={run_capture(['git', 'rev-parse', 'HEAD']).strip()}",
        f"uname={run_capture(['uname', '-a']).strip()}",
        f"pidstat_available={'yes' if command_exists('pidstat') else 'no'}",
    ]

    for key, value in sorted(os.environ.items()):
        if any(key == prefix or key.startswith(f"{prefix}_") for prefix in ENV_PREFIXES):
            lines.append(f"{key}={value}")

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


def start_pidstat(pid: int, seconds: int, run_dir: Path) -> list[subprocess.Popen[str]]:
    if not command_exists("pidstat"):
        return []

    jobs: list[subprocess.Popen[str]] = []
    specs = [
        (["pidstat", "-u", "-r", "-h", "-p", str(pid), "1", str(seconds)], "pidstat.txt"),
        (["pidstat", "-t", "-u", "-h", "-p", str(pid), "1", str(seconds)], "pidstat_threads.txt"),
    ]
    for argv, filename in specs:
        target = (run_dir / filename).open("w", encoding="utf-8")
        jobs.append(subprocess.Popen(argv, stdout=target, stderr=subprocess.STDOUT, text=True))
    return jobs


def write_summary(path: Path, run_dir: Path, label: str, seconds: int, pid: int, samples: Path, threads: Path) -> None:
    last_sample = ""
    sample_lines = samples.read_text(encoding="utf-8").splitlines()
    if len(sample_lines) > 1:
        last_sample = sample_lines[-1]

    thread_rows = []
    for line in threads.read_text(encoding="utf-8").splitlines()[1:]:
        fields = line.split()
        if len(fields) >= 5:
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
        "",
        "last_process_sample:",
        last_sample,
        "",
        "top_threads_last_sample:",
    ]
    lines.extend(row for _, row in thread_rows[:10])
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Sample one running Mars process and write dogfooding artifacts."
    )
    parser.add_argument("--label", default="manual")
    parser.add_argument("--seconds", type=int, default=15)
    parser.add_argument("--pid", type=int)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.seconds < 1:
        print("seconds must be >= 1", file=sys.stderr)
        return 2

    pid = args.pid if args.pid is not None else find_mars_pid()
    if pid is None:
        print("no running Mars process found; pass --pid PID after launching Mars", file=sys.stderr)
        return 1
    assert_process(pid)

    root = Path(os.environ.get("MARS_PERF_ARTIFACT_DIR", "artifacts/dogfooding"))
    run_dir = root / f"{utc_timestamp()}_{safe_label(args.label)}"
    run_dir.mkdir(parents=True, exist_ok=False)

    context = run_dir / "context.txt"
    samples = run_dir / "process_samples.tsv"
    threads = run_dir / "thread_samples.tsv"
    summary = run_dir / "summary.txt"

    write_context(context, args.label, args.seconds, pid)
    samples.write_text("sample_utc\tpid\tppid\tpcpu\tpmem\trss\tvsz\tstat\tcomm\targs\n", encoding="utf-8")
    threads.write_text("sample_utc\tpid\ttid\tpcpu\tpmem\tcomm\n", encoding="utf-8")

    jobs = start_pidstat(pid, args.seconds, run_dir)
    for _ in range(args.seconds):
        append_process_sample(samples, pid)
        append_thread_sample(threads, pid)
        time.sleep(1)

    for job in jobs:
        try:
            job.wait(timeout=2)
        except subprocess.TimeoutExpired:
            job.send_signal(signal.SIGTERM)
            job.wait(timeout=2)

    write_summary(summary, run_dir, args.label, args.seconds, pid, samples, threads)
    print(summary.read_text(encoding="utf-8"), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

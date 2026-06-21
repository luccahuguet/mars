#!/usr/bin/env python3
import argparse
import datetime as dt
import os
import shlex
import signal
import subprocess
import time
from pathlib import Path


WATCH_ENV = [
    "MARS_CONFIG_HOME",
    "MARS_YAZELIX_CONFIG_HOME",
    "MARS_BINARY",
    "YAZELIX_SESSION_TERMINAL",
    "MARS",
    "ZELLIJ_SESSION_NAME",
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "XDG_CURRENT_DESKTOP",
    "XDG_RUNTIME_DIR",
    "VK_ICD_FILENAMES",
    "VK_LAYER_PATH",
    "DRI_PRIME",
]

SURVIVOR_NAMES = {"zellij", "yzx", "yzx_control", "mars", "mars-desktop", "rio", "rioterm"}


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).astimezone().isoformat(timespec="seconds")


def quote_argv(argv: list[str]) -> str:
    return " ".join(shlex.quote(arg) for arg in argv)


def proc_cmdline(pid: int) -> str:
    try:
        raw = Path(f"/proc/{pid}/cmdline").read_bytes()
    except OSError:
        return ""
    return raw.replace(b"\0", b" ").decode("utf-8", "replace").strip()


def proc_stat(pid: int) -> dict[str, str]:
    try:
        text = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return {}

    close = text.rfind(")")
    if close == -1:
        return {}
    comm = text[text.find("(") + 1 : close]
    fields = text[close + 2 :].split()
    if len(fields) < 6:
        return {}
    return {
        "pid": str(pid),
        "comm": comm,
        "state": fields[0],
        "ppid": fields[1],
        "pgrp": fields[2],
        "session": fields[3],
        "tty_nr": fields[4],
        "tpgid": fields[5],
    }


def scan_processes() -> list[dict[str, str]]:
    processes = []
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        pid = int(entry.name)
        stat = proc_stat(pid)
        if not stat:
            continue
        stat["cmd"] = proc_cmdline(pid)
        processes.append(stat)
    return processes


def process_tree(root_pid: int) -> list[dict[str, str]]:
    processes = scan_processes()
    by_parent: dict[str, list[dict[str, str]]] = {}
    for proc in processes:
        by_parent.setdefault(proc["ppid"], []).append(proc)

    tree = []
    stack = [str(root_pid)]
    seen = set(stack)
    while stack:
        parent = stack.pop()
        for child in by_parent.get(parent, []):
            pid = child["pid"]
            if pid in seen:
                continue
            seen.add(pid)
            tree.append(child)
            stack.append(pid)
    return tree


def survivor_snapshot() -> list[dict[str, str]]:
    matches = []
    self_pid = str(os.getpid())
    for proc in scan_processes():
        if proc["pid"] == self_pid:
            continue
        argv0 = proc.get("cmd", "").split(" ", 1)[0]
        executable = Path(argv0).name if argv0 else ""
        if proc.get("comm", "") in SURVIVOR_NAMES or executable in SURVIVOR_NAMES:
            matches.append(proc)
    return matches[:80]


def format_proc(proc: dict[str, str]) -> str:
    cmd = proc.get("cmd") or proc.get("comm", "")
    if len(cmd) > 180:
        cmd = f"{cmd[:177]}..."
    return (
        f'pid={proc.get("pid", "?")} ppid={proc.get("ppid", "?")} '
        f'pgrp={proc.get("pgrp", "?")} session={proc.get("session", "?")} '
        f'state={proc.get("state", "?")} comm={proc.get("comm", "?")} cmd={shlex.quote(cmd)}'
    )


def log_process_list(log, prefix: str, processes: list[dict[str, str]]) -> None:
    if not processes:
        print(f"{prefix}=none", file=log, flush=True)
        return
    for proc in processes:
        print(f"{prefix} {format_proc(proc)}", file=log, flush=True)


def status_from_returncode(returncode: int) -> int:
    if returncode < 0:
        return 128 + abs(returncode)
    return returncode


def signal_name(signum: int) -> str:
    try:
        return signal.Signals(signum).name
    except ValueError:
        return str(signum)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run Mars with dogfooding launch breadcrumbs."
    )
    parser.add_argument(
        "--log-file",
        default=os.environ.get(
            "MARS_LAUNCH_LOG",
            str(Path.home() / ".cache" / "mars-desktop-launch.log"),
        ),
        help="append breadcrumbs to this file",
    )
    parser.add_argument("--label", default="mars-desktop")
    parser.add_argument("--poll-interval", type=float, default=1.0)
    parser.add_argument("--heartbeat-seconds", type=float, default=10.0)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command and args.command[0] == "--":
        args.command = args.command[1:]
    if not args.command:
        parser.error("missing command after --")
    if args.poll_interval <= 0:
        parser.error("--poll-interval must be positive")
    if args.heartbeat_seconds <= 0:
        parser.error("--heartbeat-seconds must be positive")
    return args


def main() -> int:
    args = parse_args()
    log_path = Path(args.log_file).expanduser()
    log_path.parent.mkdir(parents=True, exist_ok=True)
    received_signal: int | None = None
    child: subprocess.Popen | None = None
    last_child_stat: dict[str, str] = {}
    last_tree: list[dict[str, str]] = []

    with log_path.open("a", buffering=1, encoding="utf-8") as log:
        def handle_signal(signum, _frame):
            nonlocal received_signal
            received_signal = signum
            print(
                f"wrapper_signal time={now()} signal={signal_name(signum)} child_pid={child.pid if child else 'none'}",
                file=log,
                flush=True,
            )
            if child and child.poll() is None:
                try:
                    child.send_signal(signum)
                except ProcessLookupError:
                    pass

        old_handlers = {}
        for signum in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            old_handlers[signum] = signal.getsignal(signum)
            signal.signal(signum, handle_signal)

        try:
            start_time = now()
            print(f"--- {start_time} ---", file=log)
            print(f"start_time={start_time}", file=log)
            print(f"label={args.label}", file=log)
            print(f"wrapper_pid={os.getpid()}", file=log)
            print(f"wrapper_ppid={os.getppid()}", file=log)
            print(f"cwd={os.getcwd()}", file=log)
            print(f"argv={quote_argv(args.command)}", file=log)
            print(
                f"selected_config_path={os.environ.get('MARS_CONFIG_HOME') or str(Path.home() / '.config' / 'mars')}",
                file=log,
            )
            for name in WATCH_ENV:
                print(f"env.{name}={os.environ.get(name, '')}", file=log)

            try:
                child = subprocess.Popen(args.command, stdout=log, stderr=log)
            except OSError as exc:
                print(f"spawn_error={type(exc).__name__}: {exc}", file=log)
                print("mars exit status=127", file=log)
                print(f"end_time={now()}", file=log)
                return 127
            print(f"terminal_child_pid={child.pid}", file=log, flush=True)

            next_heartbeat = time.monotonic()
            while True:
                returncode = child.poll()
                current_stat = proc_stat(child.pid)
                if current_stat:
                    last_child_stat = current_stat

                if time.monotonic() >= next_heartbeat:
                    if current_stat:
                        last_tree = process_tree(child.pid)
                        print(f"watch child_alive=true {format_proc(current_stat)}", file=log)
                        log_process_list(log, "watch_descendant", last_tree)
                    else:
                        print(
                            f"watch child_alive=false child_pid={child.pid} last_seen={format_proc(last_child_stat) if last_child_stat else 'none'}",
                            file=log,
                        )
                    next_heartbeat = time.monotonic() + args.heartbeat_seconds

                if returncode is not None:
                    break
                time.sleep(args.poll_interval)

            exit_status = status_from_returncode(returncode)
            print(f"child_returncode={returncode}", file=log)
            if returncode < 0:
                print(f"child_signal={signal_name(abs(returncode))}", file=log)
            print(f"wrapper_received_signal={signal_name(received_signal) if received_signal else 'none'}", file=log)
            print(
                f"last_child_state={format_proc(last_child_stat) if last_child_stat else 'none'}",
                file=log,
            )
            survivors = survivor_snapshot()
            log_process_list(log, "last_descendant", last_tree)
            log_process_list(log, "survivor_candidate", survivors)
            print(
                "zellij_survivor_count="
                f"{sum(1 for proc in survivors if proc.get('comm') == 'zellij')}",
                file=log,
            )
            print(f"mars exit status={exit_status}", file=log)
            print(f"end_time={now()}", file=log)
            return exit_status
        finally:
            for signum, handler in old_handlers.items():
                signal.signal(signum, handler)


if __name__ == "__main__":
    raise SystemExit(main())

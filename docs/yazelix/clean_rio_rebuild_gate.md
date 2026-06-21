# Clean Rio Rebuild Gate

Mars stays private until this gate passes on the clean Rio-based branch.

## Branch Rule

- Start from `rio-upstream/main`
- Keep the first Mars delta limited to wrapper packaging, app identity, icon, private test config, and dogfooding gates
- Do not add shaders, background integration, renderer scheduling changes, PTY changes, or Yazelix-specific render behavior before this gate passes

## Private Yazelix Launcher

The private Mars test config is `misc/private_yazelix/config.toml`.
The matching Rio comparison config is `misc/private_rio/config.toml`.

Keep those files identical until Mars has measured evidence for a
terminal-specific config difference. Terminal identity belongs in the
launcher and environment boundary, not in the TOML.

After a local Mars package or binary exists, launch it with:

```sh
tools/mars_private_yazelix.py
```

The launcher sets `MARS_CONFIG_HOME`. The config owns startup through
`[shell] program = "yzx"` and `args = ["start"]`. Use
`MARS_BINARY=/path/to/mars` when testing a build artifact directly.

## Artifact Rule

Every reproducible run leaves logs under:

```text
artifacts/dogfooding/<timestamp>_repro_suite/
```

Minimum saved files:

- `suite_summary.txt`
- `context.txt`
- `manifest.txt`
- `workload.py`
- `workload_phase.txt`
- `process_samples.tsv`
- `thread_samples.tsv`
- `summary.txt`
- `pidstat.txt` and `pidstat_threads.txt` when `pidstat` is available; these are the primary CPU/resource artifacts
- `perf_stat.txt` when `--perf-stat` is requested and `perf` is available, otherwise `perf_stat_skipped.txt`

## Reproducible Gate

Run the gate with:

```sh
tools/mars_perf_gate.py --suite --seconds 20
```

The suite launches Mars itself with an isolated generated config and fixed workloads. Default scenarios:

- `idle`: quiet Python process inside Mars
- `pty_flood`: fixed high-volume output stream
- `scroll_render`: fixed long scrollback stream with varied line widths
- `yzx_screen_mandelbrot`: bounded `yzx screen mandelbrot` run when `yzx` is available

Run one scenario with:

```sh
tools/mars_perf_gate.py --suite --scenario pty_flood --seconds 20
```

Run repeated samples with:

```sh
tools/mars_perf_gate.py --suite --seconds 20 --repeat 3
```

The runner does not compute its own statistics. Compare repeated run directories and use the `pidstat` summaries as the primary measurement source.

Add hardware-counter evidence when the host has `perf`:

```sh
tools/mars_perf_gate.py --suite --scenario pty_flood --seconds 20 --perf-stat
```

`--seconds` is the active workload duration for bounded workloads. Non-idle scenarios also sample `--cooldown-seconds` after the workload window so sustained CPU after output or animation can be seen in the same artifact.

Documented variables are written to `suite_summary.txt` and each scenario `manifest.txt`: Mars binary, workload seconds, sample seconds, startup delay, line counts, cooldown, repeat count, generated config path, workload path, phase file, primary sampler, and whether `perf stat` was requested.

Use a specific binary with:

```sh
MARS_BINARY=mars-desktop tools/mars_perf_gate.py --suite --seconds 20
```

## Supplemental Sampling

Manual dogfooding can add evidence, but it is not the gate. To sample an already running Mars process:

```sh
tools/mars_perf_gate.py --label live_dogfood --seconds 20
```

## Pass Bar

- Idle Mars does not sit on a busy core
- PTY flood does not leave the reader thread pegged after output stops
- `yzx screen` does not cause sustained CPU after the bounded workload ends
- Scenario summaries include `workload_phase`; a PTY or scroll row that never reaches `output_done` did not complete the intended cooldown portion
- Artifacts identify whether CPU is in Mars, Zellij, Codex, or helpers
- 2-4 active Codex sessions remain a supplemental dogfooding row until a synthetic Codex-load harness exists

If any row fails, keep Mars private and create a focused follow-up Bead from the artifact evidence.

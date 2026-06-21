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

The launcher sets `MARS_CONFIG_HOME` and starts Yazelix with
`mars -e yzx enter`. The config stays terminal-only. Use
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
- `mars_perf_metrics.jsonl` and `mars_perf_metrics_summary.json` when `--internal-metrics` is requested

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

The runner only computes small `ps` fallback summaries. Compare repeated run directories and use `pidstat`/`perf` artifacts as the primary measurement source when those tools are available.

Generate a deterministic terminal input corpus separately from measurement:

```sh
tools/mars_perf_gate.py --generate-corpus /tmp/mars_scroll.bin --corpus-kind scroll_render --corpus-seed 7 --corpus-lines 80000
```

Replay that exact corpus through Mars with normal artifacts:

```sh
tools/mars_perf_gate.py --suite --scenario corpus_replay --corpus /tmp/mars_scroll.bin --seconds 20
```

Corpus replay manifests include corpus path, size, SHA-256, seed,
generator version, terminal rows/columns, line count, and copied metadata.
Corpus generation is not part of timed measurement.

## Parser Throughput Benchmark

Run the parser-only benchmark without starting Mars, a PTY, Zellij, or the renderer:

```sh
cargo run -p rio-backend --bin mars-parser-bench --release \
  --features rio-window/x11,rio-window/wayland -- \
  --corpus rio-backend/fixtures/parser_smoke.txt \
  --rows 24 \
  --columns 80 \
  --chunk-size 4096 \
  --iterations 10
```

Debug builds are not representative. Use `--release` or a benchmark profile for
numbers you intend to compare.

For a larger escape-heavy corpus, generate input once and then benchmark that
same file:

```sh
tools/mars_perf_gate.py --generate-corpus /tmp/mars_parser_osc.bin \
  --corpus-kind osc_control \
  --corpus-seed 7 \
  --corpus-lines 80000 \
  --corpus-rows 44 \
  --corpus-columns 132

cargo run -p rio-backend --bin mars-parser-bench --release \
  --features rio-window/x11,rio-window/wayland -- \
  --corpus /tmp/mars_parser_osc.bin \
  --rows 44 \
  --columns 132 \
  --chunk-size 4096 \
  --iterations 5
```

The output records corpus path, corpus bytes, rows, columns, chunk size,
iterations, elapsed nanoseconds, bytes per second, parser action counts, and
printed UTF-8 byte counts.
Rows and columns are recorded for corpus comparability; this benchmark measures
only the escape parser and does not allocate a terminal grid.

To compare Mars against upstream Rio, apply the parser-benchmark patch series
to a `rio-upstream/main` worktree, generate one corpus file, and run the exact
same `mars-parser-bench` command in both worktrees. Compare only release-mode
output from the same machine and same corpus file; use the generator metadata
SHA-256 when the corpus was produced by `mars_perf_gate.py`.

## Terminal Stream State Benchmark

Run the parser plus terminal/grid state benchmark without starting Mars, a PTY,
Zellij, or the renderer:

```sh
cargo run -p rio-backend --bin mars-terminal-stream-bench --release \
  --features rio-window/x11,rio-window/wayland -- \
  --corpus rio-backend/fixtures/terminal_stream_smoke.txt \
  --rows 24 \
  --columns 80 \
  --scrollback 10000 \
  --chunk-size 4096 \
  --iterations 10
```

Use this benchmark when parser-only throughput is fine but CPU could be spent
mutating terminal state: wrapping, scrollback, cursor movement, styles, and
grid changes. It still excludes PTY scheduling, Zellij, renderer, GPU, shaders,
and live Codex load.

For a larger corpus, generate input once and replay the exact same file:

```sh
tools/mars_perf_gate.py --generate-corpus /tmp/mars_stream_scroll.bin \
  --corpus-kind scroll_render \
  --corpus-seed 7 \
  --corpus-lines 80000 \
  --corpus-rows 44 \
  --corpus-columns 132

cargo run -p rio-backend --bin mars-terminal-stream-bench --release \
  --features rio-window/x11,rio-window/wayland -- \
  --corpus /tmp/mars_stream_scroll.bin \
  --rows 44 \
  --columns 132 \
  --scrollback 10000 \
  --chunk-size 4096 \
  --iterations 5
```

The output records corpus path/bytes, detected sidecar metadata path/size, the
sidecar JSON metadata on one line when present, terminal dimensions, scrollback
limit, chunk size, iterations, elapsed nanoseconds, bytes per second, final
cursor position, final scrollback size, display offset, total grid lines, and
synchronized-update buffer bytes.

To compare Mars against upstream Rio, apply the benchmark patch series to a
`rio-upstream/main` worktree, generate one corpus file, and run the exact same
`mars-terminal-stream-bench` command in both worktrees. Compare only release
output from the same machine, corpus, dimensions, scrollback, chunk size, and
iteration count. Use the generator metadata SHA-256 when the corpus was
produced by `mars_perf_gate.py`.

Add hardware-counter evidence when the host has `perf`:

```sh
tools/mars_perf_gate.py --suite --scenario pty_flood --seconds 20 --perf-stat
```

Add Mars-owned PTY/render histograms for a focused traced run:

```sh
tools/mars_perf_gate.py --suite --scenario pty_flood --seconds 20 --internal-metrics
```

Internal metrics are off by default in Mars. The suite enables them by
setting `MARS_PERF_METRICS=1` and `MARS_PERF_METRICS_FILE` only when
`--internal-metrics` is present. Manual dogfooding can also enable them
with `MARS_PERF_METRICS=1` or `MARS_PERF_TRACE=metrics`.

`--seconds` is the active workload duration for bounded workloads. Non-idle scenarios also sample `--cooldown-seconds` after the workload window so sustained CPU after output or animation can be seen in the same artifact.

Documented variables are written to `suite_summary.txt` and each scenario `manifest.txt`: Mars binary, workload seconds, sample seconds, startup delay, line counts, cooldown, repeat count, generated config path, workload path, phase file, primary sampler, whether `perf stat` was requested, and whether internal Mars metrics were requested.

Use a specific binary with:

```sh
MARS_BINARY=mars-desktop tools/mars_perf_gate.py --suite --seconds 20
```

## Supplemental Sampling

Manual dogfooding can add evidence, but it is not the gate. To sample an already running Mars process:

```sh
tools/mars_perf_gate.py --label live_dogfood --seconds 20
```

## Launcher Breadcrumbs

Private dogfooding launchers should route desktop starts through:

```sh
mars-launch-trace --log-file ~/.cache/mars-desktop-launch.log -- mars ...
```

The trace helper records wrapper PID, Mars frontend PID, argv, selected
config path, key environment variables, periodic child state, descendant
processes, survivor candidates, final status, and end time. It is for
diagnostics only and does not replace the reproducible performance gate.

## Yazelix Cursors V1

`yzc materialize rio-compatible-config` materializes a launch-local
Rio-compatible config from the current cursor selection. V1 only writes
the selected color to `[colors].cursor`; split, multicolor, and shader
cursor geometry stay out of scope until separate cursor beads implement
them.

## Pass Bar

- Idle Mars does not sit on a busy core
- PTY flood does not leave the reader thread pegged after output stops
- `yzx screen` does not cause sustained CPU after the bounded workload ends
- Scenario summaries include `workload_phase`; a PTY or scroll row that never reaches `output_done` did not complete the intended cooldown portion
- Artifacts identify whether CPU is in Mars, Zellij, Codex, or helpers
- Focused traced runs can show PTY read batches, parser/state batches, render snapshots, grid row/cell emission, and frame present timing
- 2-4 active Codex sessions remain a supplemental dogfooding row until a synthetic Codex-load harness exists

If any row fails, keep Mars private and create a focused follow-up Bead from the artifact evidence.

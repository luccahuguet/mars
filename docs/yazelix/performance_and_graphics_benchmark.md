# Performance And Graphics Benchmark

Status: local benchmark evidence for `yzt-7p3.15`.

Date: 2026-05-31.

## Environment

- Host: COSMIC Wayland session
- Kernel: Linux `6.18.7-76061807-generic`
- Rio: `rioterm 0.4.6`, built with `nix develop -c cargo build -p rioterm --release`
- Rio benchmark binary: `target/release/rio`
- Ghostty: `Ghostty 1.3.1`, stable channel, GTK runtime, OpenGL renderer
- Ghostty config isolation: `--config-default-files=false`
- Rio WGPU backend: `WGPU_BACKEND=vulkan`

## Startup/Exit Timing

Contract measured: start the terminal, spawn `bash --noprofile --norc -c exit`,
and wait for the terminal process to exit after the child process exits.

This is a process startup/shutdown benchmark. It does not claim first-frame
latency, frame pacing, throughput, memory, or power results.

Raw data:

- `artifacts/benchmarks/startup_exit_2026_05_31.csv`

Results after 3 warmups and 20 measured runs:

| Case | Min | Mean | Median | Max |
| --- | ---: | ---: | ---: | ---: |
| Rio WGPU/Vulkan, no shader | 0.026s | 0.032s | 0.032s | 0.042s |
| Rio WGPU/Vulkan, Ghostty probe shader | 0.027s | 0.031s | 0.031s | 0.039s |
| Rio CPU renderer | 0.027s | 0.032s | 0.032s | 0.038s |
| Ghostty OpenGL, default config disabled | 0.281s | 0.302s | 0.301s | 0.328s |
| Ghostty OpenGL, Ghostty probe shader | 0.326s | 0.341s | 0.340s | 0.358s |

Interpretation:

- Rio has much lower process startup/exit time in this local harness
- The minimal Ghostty-compatible cursor probe does not measurably increase Rio
  startup/exit time in this test
- Ghostty shows a visible startup cost for the same probe shader, but this is
  still only process startup timing

## Scrollback Stress Smoke

Contract measured: start the terminal, print 20,000 simple lines through a PTY,
sleep for `0.2s`, and wait for the terminal process to exit.

This catches obvious PTY/render-loop instability and rough process completion
time. It is not a frame-time histogram and does not prove every line was
presented to the display before exit.

Raw data:

- `artifacts/benchmarks/scroll_stress_2026_05_31.csv`

Results after 2 warmups and 10 measured runs:

| Case | Min | Mean | Median | Max |
| --- | ---: | ---: | ---: | ---: |
| Rio WGPU/Vulkan, no shader | 0.375s | 0.403s | 0.401s | 0.438s |
| Ghostty OpenGL, default config disabled | 0.576s | 0.597s | 0.592s | 0.625s |

Interpretation:

- Rio completed this PTY scroll stress faster on this host
- No crashes or hangs were observed in either terminal
- This is useful as a smoke test, not as a substitute for real scrollback frame
  pacing instrumentation

## Graphics Evidence

Rio WGPU/Vulkan shader probe:

- command:
  `python3 tools/yazelix_conformance.py launch-wgpu-shader-screenshot`
- screenshot:
  `artifacts/shader_probe/screenshots/wgpu_shader_probe_vulkan.png`
- result: WGPU/Vulkan surface creation succeeded and the cursor probe rendered

Ghostty OpenGL shader probe:

- command:
  `ghostty --config-default-files=false --gtk-single-instance=false --window-decoration=false --custom-shader=$PWD/conformance/shaders/ghostty_cursor_probe.glsl -e bash --noprofile --norc -c 'printf "ghostty shader probe\nGhostty cursor uniforms\nPID $$\n"; sleep 4'`
- screenshot:
  `artifacts/benchmarks/screenshots/ghostty_shader_probe.png`
- result: Ghostty loaded the same shader source and rendered normally

## Frame-Time Harness

Status: implemented for `yzt-7p3.39`.

Rio now has an opt-in frame event log for benchmark runs. Set
`YAZELIX_TERMINAL_FRAME_LOG=/path/to/frame_log.jsonl` and each
`RedrawRequested` pass writes a JSON event with:

- route and window id
- whether the pass presented a frame or only drained queued render state
- first-redraw and per-redraw elapsed timestamps
- render duration
- frame-to-frame delta
- target vblank interval
- whether the renderer is in game mode

Normal terminal sessions do not create this log. If the env var is present and
the log cannot be opened or written, Rio fails fast because benchmark evidence
must not silently degrade.

The stdlib harness wraps the frame log with process sampling and artifact
generation:

```bash
python3 tools/yazelix_benchmark.py frame-run \
  --terminal target/release/rio \
  --config-template wgpu \
  --workload scroll
```

Artifacts are written under `artifacts/benchmarks/frame_time/<timestamp>/`:

- `frame_log.jsonl`: raw Rio frame events
- `proc_samples.csv`: `/proc/<pid>` CPU tick and RSS samples for the Rio process
- `summary.json`: first-redraw latency, first-presented-frame latency,
  redraw/presented frame delta histograms, render duration histogram, target
  vblank histogram, and process CPU/RSS summary
- `summary.csv`: flattened summary for spreadsheet comparison
- `rio_config_home/config.toml`: isolated Rio config used by the run, unless
  `--config-template host` is selected
- `stdout.log` and `stderr.log`: terminal process output

Built-in workloads:

```bash
python3 tools/yazelix_benchmark.py frame-run --workload scroll
python3 tools/yazelix_benchmark.py frame-run --workload idle
python3 tools/yazelix_benchmark.py frame-run --workload kitty-graphics
python3 tools/yazelix_benchmark.py frame-run --workload sixel
python3 tools/yazelix_benchmark.py frame-run --config-template wgpu-shader --workload idle --hold-seconds 10
```

The `wgpu-shader` template enables the Ghostty cursor shader probe and Rio's
game render strategy, which makes long-running shader animation stability
measurable from the same frame log.

Existing frame logs can be summarized without launching a terminal:

```bash
python3 tools/yazelix_benchmark.py frame-summary \
  artifacts/benchmarks/frame_time/<run>/frame_log.jsonl \
  --samples artifacts/benchmarks/frame_time/<run>/proc_samples.csv
```

## Remaining Gaps

The next useful benchmark work should add:

- comparable Ghostty runs that use the same workload definitions where possible
- GPU utilization sampling through optional host tools when available
- release-package benchmarks instead of only local `target/release/rio`

Tracked as Bead `yzt-7p3.41`.

# Performance And Graphics Benchmark

Status: local benchmark evidence for `yzt-7p3.15`, `yzt-7p3.39`, and
`yzt-7p3.41`.

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
  `nix run .#yazelix-protocol-conformance -- launch-wgpu-shader-screenshot`
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
`MARS_FRAME_LOG=/path/to/frame_log.jsonl` and each
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
python3 tools/yazelix_benchmark.py frame-run --config-template yzt-default --workload helix-jk --lines 4000 --hold-seconds 1
python3 tools/yazelix_benchmark.py frame-run --config-template yzt-default --workload helix-viewport --lines 400 --hold-seconds 1
python3 tools/yazelix_benchmark.py frame-run --config-template yzt-shaders --workload helix-viewport --lines 400 --hold-seconds 1
```

The `wgpu-shader` template enables the Ghostty-compatible cursor shader probe
and Rio's game render strategy, which makes long-running shader animation
stability measurable from the same frame log.

The Helix-style workloads are deterministic proxies for editor movement without
GUI key injection:

- `helix-jk` moves the terminal cursor inside a stable viewport.
- `helix-viewport` models Helix scroll-margin behavior: once the cursor reaches
  the top or bottom five visible lines and movement continues in that
  direction, the viewport follows and rows are repainted.

Use `helix-viewport` as the primary workload for latency that appears near the
top or bottom of the editor viewport. Use this matrix when investigating
interactive cursor latency:

```bash
python3 tools/yazelix_benchmark.py frame-run --config-template wgpu-no-effects --workload helix-viewport --lines 400 --hold-seconds 1
python3 tools/yazelix_benchmark.py frame-run --config-template wgpu-shader-event --workload helix-viewport --lines 400 --hold-seconds 1
python3 tools/yazelix_benchmark.py frame-run --config-template yzt-default --workload helix-viewport --lines 400 --hold-seconds 1
python3 tools/yazelix_benchmark.py frame-run --config-template yzt-default-game --workload helix-viewport --lines 400 --hold-seconds 1
python3 tools/yazelix_benchmark.py frame-run --config-template yzt-shaders --workload helix-viewport --lines 400 --hold-seconds 1
python3 tools/yazelix_benchmark.py frame-run --config-template yzt-shaders-game --workload helix-viewport --lines 400 --hold-seconds 1
```

For packaged `mars`, the runtime no-effects equivalent is
`MARS_PROFILE=baseline`. That profile keeps the packaged WebGPU,
font, and window settings while removing custom shaders and trail cursor
effects, so it is the first comparison to run before blaming event scheduling.
The packaged default equivalent is `MARS_PROFILE=full`, which keeps
Rio's native trail cursor and does not load custom shaders. Use
`MARS_PROFILE=shaders` only when measuring the opt-in
Ghostty-compatible shader stack.

Platform coverage caveat: the Helix viewport latency work touches shared
PTY/render-lock code, so Windows and macOS inherit the same scheduling behavior.
The current evidence is Linux-local, collected on a COSMIC Wayland session.
Before claiming that the shared changes do not regress Windows or macOS, native
smoke tests should cover shell startup, heavy scroll output, repeated
resize/viewport changes, and the `helix-viewport` workload on those platforms.

For Ghostty or another terminal binary, pass terminal arguments before the
child `-e` command with repeated `--terminal-arg=...` values:

```bash
python3 tools/yazelix_benchmark.py frame-run \
  --terminal "$(command -v ghostty)" \
  --config-template host \
  --terminal-arg=--config-default-files=false \
  --terminal-arg=--gtk-single-instance=false \
  --terminal-arg=--window-decoration=false \
  --workload scroll
```

When `nvidia-smi` is available, the harness samples host GPU utilization into
`gpu_samples.csv` by default. Use `--gpu-sampler none` to disable that optional
probe.

Existing frame logs can be summarized without launching a terminal:

```bash
python3 tools/yazelix_benchmark.py frame-summary \
  artifacts/benchmarks/frame_time/<run>/frame_log.jsonl \
  --samples artifacts/benchmarks/frame_time/<run>/proc_samples.csv
```

## Comparable Rio/Ghostty Runs

Status: one local release-binary sample per case, collected on 2026-05-31.

Raw summary and sample artifacts:

- `artifacts/benchmarks/frame_time/2026_05_31_rio_wgpu_scroll/`
- `artifacts/benchmarks/frame_time/2026_05_31_ghostty_opengl_scroll/`
- `artifacts/benchmarks/frame_time/2026_05_31_rio_wgpu_shader_idle/`
- `artifacts/benchmarks/frame_time/2026_05_31_rio_wgpu_shader_idle_throttled/`
- `artifacts/benchmarks/frame_time/2026_05_31_ghostty_opengl_shader_idle/`

The Rio runs use `target/release/rio` under `nix develop`. The Ghostty runs use
Ghostty `1.3.1` with `--config-default-files=false`,
`--gtk-single-instance=false`, and `--window-decoration=false`. Shader runs use
the same `conformance/shaders/ghostty_cursor_probe.glsl` source.

| Case | Workload | Wall | First Rio Frame | Rio Frames | Process CPU | Max RSS | NVIDIA GPU Max |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Rio WGPU/Vulkan | 20k-line scroll + 0.5s hold | 0.869s | 12.4ms | 5 redraw / 4 presented | 19.9% | 38.8 MiB | 0% |
| Ghostty OpenGL | 20k-line scroll + 0.5s hold | 1.072s | n/a | n/a | 44.3% | 149.8 MiB | 0% |
| Rio WGPU/Vulkan + shader, before `yzt-7p3.46` | idle + 4s hold | 4.239s | 13.0ms | 69,688 presented | 96.9% | 25.0 MiB | 0% |
| Rio WGPU/Vulkan + shader, after `yzt-7p3.46` | idle + 4s hold | 4.253s | 12.8ms | 237 presented | 2.5% | 25.4 MiB | 0% |
| Ghostty OpenGL + shader | idle + 4s hold | 4.459s | n/a | n/a | 18.2% | 164.5 MiB | 0% |

Interpretation:

- The scroll sample favors Rio on local wall time, process CPU, and RSS. Rio
  also produces internal frame timing while Ghostty does not expose comparable
  frame events through this external harness.
- The original shader idle sample revealed a Rio problem: the game render
  strategy spun far above display cadence and consumed almost a full CPU core.
  `yzt-7p3.46` fixed that by scheduling the next game redraw at the window
  vblank interval instead of immediately requesting another redraw. The
  post-fix run presented 237 frames over four seconds with a 16.99ms median
  frame delta and 2.5% sampled process CPU.
- The `nvidia-smi` samples saw 0% utilization and 2 MiB memory on the discrete
  NVIDIA GPU for every run. On this host that likely means the sampled GPU is
  not the compositor/render path for these windows; it is not proof of zero GPU
  work.

## Remaining Gaps

The next useful benchmark work should add:

- repeated samples with aggregation instead of one local sample per case
- direct Ghostty frame pacing instrumentation if an external or source-level
  hook is found
- longer shader stability runs after the throttled game loop fix
- real Helix key-injection captures to complement the deterministic
  `helix-jk` and `helix-viewport` proxies
- native Windows and macOS smoke runs for shared PTY/render-lock changes before
  making no-regression claims on those platforms

Tracked as Bead `yzt-7p3.41`.

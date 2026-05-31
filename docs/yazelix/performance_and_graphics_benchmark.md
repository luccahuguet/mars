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

## Gaps

The next useful benchmark work should add:

- first-frame latency measurement
- frame-time histograms during scrolling and shader animation
- CPU/GPU utilization and memory measurements
- long-running shader animation stability
- graphics workloads for Kitty graphics and Sixel images
- release-package benchmarks instead of only local `target/release/rio`

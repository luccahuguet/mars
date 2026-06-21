# Clean Rio Rebuild Gate

Mars stays private until this gate passes on the clean Rio-based branch.

## Branch Rule

- Start from `rio-upstream/main`
- Keep the first Mars delta limited to wrapper packaging, app identity, icon, private test config, and dogfooding gates
- Do not add shaders, background integration, renderer scheduling changes, PTY changes, or Yazelix-specific render behavior before this gate passes

## Private Yazelix Launcher

The private test config is `misc/private_yazelix/config.toml`.

It runs:

```toml
[shell]
program = "yzx"
args = ["start"]
```

After a local Mars package or binary exists, launch it with:

```sh
tools/mars_private_yazelix.py
```

Use `MARS_BINARY=/path/to/mars` when testing a build artifact directly.

## Artifact Rule

Every run should leave logs under:

```text
artifacts/dogfooding/<timestamp>_<label>/
```

Minimum saved files:

- `context.txt`
- `process_samples.tsv`
- `thread_samples.tsv`
- `summary.txt`
- `pidstat.txt` and `pidstat_threads.txt` when `pidstat` is available

## Rows

Run each row with a fresh Mars window and sample the Mars process:

```sh
tools/mars_perf_gate.py --label idle --seconds 20
```

Required rows:

- `idle`: Mars open, no active workload
- `pty_flood`: sustained terminal output without Codex
- `scroll_render`: long scrollback/render pass
- `welcome_screen`: Yazelix welcome or `yzx screen` animation
- `codex_2`: two Codex sessions active
- `codex_3`: three Codex sessions active
- `codex_4`: four Codex sessions active

Suggested proxy commands inside Mars:

```sh
python3 - <<'PY'
for i in range(200000):
    print(f"pty flood line {i:06d} " + "abcdef0123456789" * 8)
PY
```

```sh
seq 1 100000
```

## Pass Bar

- Idle Mars does not sit on a busy core
- PTY flood does not leave the reader thread pegged after output stops
- Welcome and `yzx screen` animations do not show cursor artifacts or repeated top-down rerenders
- 2-4 active Codex sessions remain usable enough for normal Yazelix work
- Artifacts identify whether CPU is in Mars, Zellij, Codex, or helpers

If any row fails, keep Mars private and create a focused follow-up Bead from the artifact evidence.

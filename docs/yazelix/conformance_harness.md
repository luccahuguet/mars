# Conformance Harness

The local harness lives at `tools/yazelix_conformance.py`. It is intentionally
small and dependency-free so protocol research can move without changing Rio's
Rust workspace.

## Commands

List checked-in fixture streams:

```text
python3 tools/yazelix_conformance.py list
```

Validate fixture manifest bytes and the Ghostty cursor shader probe:

```text
python3 tools/yazelix_conformance.py verify
```

Emit one fixture byte stream to a terminal or file:

```text
python3 tools/yazelix_conformance.py emit osc133_semantic_prompt
```

Record local source/version evidence:

```text
python3 tools/yazelix_conformance.py record-env
```

Launch the built Rio binary with CPU renderer and capture a COSMIC screenshot:

```text
python3 tools/yazelix_conformance.py launch-cpu-screenshot
```

## Current Fixture Scope

The first manifest covers:

- OSC 8 hyperlinks
- OSC 52 clipboard query
- OSC 133 semantic prompt regions
- Kitty OSC 66 text sizing
- Kitty keyboard mode query
- Kitty graphics 1x1 RGBA transmit/place
- minimal Sixel DCS path
- synchronized output DECSET 2026
- XTVERSION
- XTGETTCAP RGB query

The fixtures are not proof that Rio supports every protocol correctly. They are
stable byte streams that future beads can feed into Rio, Ghostty, WezTerm, or
black-box probes and compare against expected behavior.

## Shader Probe

`conformance/shaders/ghostty_cursor_probe.glsl` is a minimal shader that reads
Ghostty's cursor uniforms. It is not a visual parity target by itself. Its job is
to fail early when a renderer path cannot compile or populate the names Yazelix
cursor presets depend on.

CPU renderer screenshots prove process launch and window rendering only. They do
not prove shader parity.

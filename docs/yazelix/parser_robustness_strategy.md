# Parser Robustness Strategy

This fork treats terminal parser robustness as release-critical. Modern terminal
features are escape-sequence heavy, and Yazelix needs failures to be isolated to
one rejected sequence instead of turning into panics, parser desynchronization, or
renderer corruption.

## Current Strategy

The default test lane owns a deterministic parser noise smoke test in
`rio-backend/src/performer/parser/mod.rs`.

That test feeds pseudo-random byte chunks through the existing parser and test
dispatcher. It is intentionally small, deterministic, and dependency-free so it
can run in normal Rust checks without a special fuzzing toolchain.

This does not replace feature-specific parser tests. Every implemented protocol
still needs direct tests for valid inputs, malformed inputs, chunk boundaries,
and maximum-size behavior.

## Coverage Layers

- parser unit tests defend escape-sequence decoding and malformed input handling
- `tools/yazelix_conformance.py` fixtures defend protocol-level request/response
  behavior through a real PTY stream
- screenshot or framebuffer probes defend visual protocols and cursor shaders
- manual Yazelix sessions defend Zellij, Yazi, Helix, and shell behavior together

The deterministic noise test is a guardrail for accidental panics. It is not a
parity claim by itself.

## When To Add Heavy Fuzzing

Add a dedicated fuzz target, likely `cargo-fuzz`/libFuzzer, when one of these
conditions is true:

- parser bugs start depending on long or hard-to-minimize byte streams
- OSC, DCS, APC, CSI, or Kitty graphics parsing grows enough shared state that
  deterministic smoke coverage is too shallow
- fuzz corpora from Ghostty, WezTerm, terminal specs, or Yazelix bug reports need
  minimization and regression preservation
- release validation needs sanitizer-backed parser coverage outside the default
  Rust test lane

The first fuzz target should drive Rio's parser boundary directly and reuse
small protocol corpora from the conformance harness. It should not shell out to a
terminal binary.

## Source And License Rules

Ghostty and WezTerm are permissive references for behavior, fixtures, and test
shape when attribution is recorded. Kitty remains a public spec and black-box
behavior source unless a separate GPL decision changes that policy.

Do not copy Kitty implementation code into fuzz harnesses, corpora, tests, or
comments.

## Promotion Criteria

Promote a protocol from "smoke-covered" to "robust" only after it has:

- valid input tests
- malformed input tests
- split-chunk tests
- maximum-size or policy-limit tests when applicable
- conformance fixture coverage when the behavior crosses the PTY boundary

For visual protocols, add screenshot or framebuffer evidence before claiming
Ghostty parity.

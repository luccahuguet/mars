Yazelix Terminal Cursor Shaders
===============================

These shaders are the packaged Yazelix Terminal opt-in Ghostty-style cursor
stack for Rio's WGPU custom-shader runtime. The default Yazelix Terminal
profile uses Rio's native `trail-cursor` without stacking custom cursor
shaders.

- `cursor_trail_dusk.glsl` is generated for Yazelix Terminal's cursor trail
  shader profile with medium glow. In Yazelix Terminal it decorates Rio's
  `trail-cursor` geometry through the guarded `YAZELIX_TERMINAL_RIO_TRAIL`
  uniform extension; in Ghostty it keeps the normal Ghostty cursor-uniform path
- `generated_effects/sweep.glsl` and `generated_effects/rectangle_boom.glsl`
  are generated from vendored Ghostty cursor effect templates

The effect templates are from `https://github.com/sahaj-b/ghostty-cursor-shaders`.
Keep this directory as terminal-owned generated shader assets, not as the
long-term cursor configuration source of truth. Main Yazelix should select
published yzxterm profiles instead of generating or editing these Rio-aware
assets.

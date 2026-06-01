Yazelix Terminal Cursor Shaders
===============================

These shaders are the packaged Yazelix default Ghostty-style cursor stack for
Rio's WGPU custom-shader runtime.

- `cursor_trail_dusk.glsl` is generated from Yazelix's cursor trail shader
  sources with medium glow
- `generated_effects/sweep.glsl` and `generated_effects/rectangle_boom.glsl`
  are generated from vendored Ghostty cursor effect templates

The effect templates are from `https://github.com/sahaj-b/ghostty-cursor-shaders`.
Keep this directory as generated shader assets, not as the long-term cursor
configuration source of truth.

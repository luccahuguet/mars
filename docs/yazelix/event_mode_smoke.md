# Event-Mode Smoke Checklist

Use this after a packaged Mars Terminal build when changing renderer event
delivery, cursor animation, WGPU backend selection, or the desktop wrapper.

Automated package checks:

```sh
tools/yazelix_event_mode_smoke.sh ./result_yazelix_terminal_package
```

The script verifies that the packaged config does not default to
`renderer.strategy = "game"`, that the default profile enables Rio
`trail-cursor` without custom shaders, that the opt-in shader profile starts,
and that `YAZELIX_TERMINAL_RENDER_STRATEGY=game` still materializes a valid
escape-hatch config. It also verifies that `YAZELIX_TERMINAL_PROFILE=baseline`
starts a no-effects config and composes with the game-mode diagnostic path.

Manual session smoke:

1. Open the Mars Terminal desktop launcher.
2. Type after the terminal has been idle for a few seconds.
3. Run `yzx enter`.
4. Create and walk Zellij panes.
5. Resize the window repeatedly.
6. Preview an image and a PDF in Yazi.
7. Move the editor cursor quickly and confirm Rio's trail cursor animation
   renders smoothly.
8. Relaunch with `YAZELIX_TERMINAL_PROFILE=shaders` only when validating the
   Ghostty-compatible shader stack.

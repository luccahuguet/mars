# Event-Mode Smoke Checklist

Use this after a packaged Yazelix Terminal build when changing renderer event
delivery, cursor animation, WGPU backend selection, or the desktop wrapper.

Automated package checks:

```sh
tools/yazelix_event_mode_smoke.sh ./result_yazelix_terminal_package
```

The script verifies that the packaged config does not default to
`renderer.strategy = "game"`, that the desktop wrapper starts with the packaged
config, and that `YAZELIX_TERMINAL_RENDER_STRATEGY=game` still materializes a
valid escape-hatch config.

Manual session smoke:

1. Open the `Yazelix Terminal` desktop launcher.
2. Type after the terminal has been idle for a few seconds.
3. Run `yzx enter`.
4. Create and walk Zellij panes.
5. Resize the window repeatedly.
6. Preview an image and a PDF in Yazi.
7. Move the editor cursor quickly and confirm the Yazelix cursor shader/trail
   animation still renders.

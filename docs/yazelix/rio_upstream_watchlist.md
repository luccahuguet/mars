# Rio Upstream Watchlist

Review date: 2026-06-21

These upstream Rio issues affect Mars dogfooding decisions. Treat them as context before creating Mars-specific Beads. Create a Mars Bead only when local evidence shows the behavior still reproduces in Mars after the clean rebuild baseline.

Before editing Rio-owned source, refresh this context for the current bug instead of relying only on the table below:

```sh
gh issue list --repo raphamorim/rio --state all --search '<symptom or protocol keywords>'
git fetch rio-upstream main
base=$(git merge-base HEAD rio-upstream/main)
git log --oneline "$base"..rio-upstream/main -- <suspect-rio-paths>
git log --oneline --grep='<keyword>' "$base"..rio-upstream/main
```

Record the issue query, result, and upstream commit range in the Bead or `docs/yazelix/change_scorecard.md`.

## Startup, GPU, And Vulkan

| Issue | Relevance To Mars |
| --- | --- |
| [#1641 rio panics in wgpu_core on RasPi 4 with latest Mesa and Vulkan drivers](https://github.com/raphamorim/rio/issues/1641) | Open upstream wgpu/Vulkan panic report. The reported panic differs from local `vkCreateInstance failed: ERROR_INCOMPATIBLE_DRIVER`, but it confirms current Rio has active Vulkan/wgpu crash reports. |
| [#542 Crashes on nix on Fedora](https://github.com/raphamorim/rio/issues/542) | Older Nix/Linux crash context. Useful when separating Nix wrapper or library path problems from terminal code. |
| [#559 Crash on startup in ubuntu 24.04](https://github.com/raphamorim/rio/issues/559) | Startup crash context on Linux. Check before assuming Mars-specific startup behavior. |

## COSMIC, Wayland, And Window Behavior

| Issue | Relevance To Mars |
| --- | --- |
| [#1644 Background opacity does not work anymore (cosmic/wayland)](https://github.com/raphamorim/rio/issues/1644) | Directly matches the local transparency problem on COSMIC/Wayland. Treat opacity failure as likely upstream until a Mars-only difference is proven. |
| [#1658 Bug Report: Rio terminal close button freezes under Labwc (Wayland)](https://github.com/raphamorim/rio/issues/1658) | Wayland close/freeze issue. Relevant to Mars crash and shutdown dogfooding. |
| [#1620 Wayland: mouse selection is not exported to PRIMARY selection](https://github.com/raphamorim/rio/issues/1620) | Wayland integration issue. Not a blocker for Mars startup, but relevant to daily terminal quality. |
| [#1622 Window title on Wayland CSD drops non-Cantarell glyphs](https://github.com/raphamorim/rio/issues/1622) | Wayland CSD/titlebar font fallback issue. Relevant only if Mars keeps Rio CSD behavior visible. |

## Redraw, Resize, And Compositor Cost

| Issue | Relevance To Mars |
| --- | --- |
| [#1603 Terminal window stops responding to resize requests if a process is running under Rio without output](https://github.com/raphamorim/rio/issues/1603) | Matches stale resize/event-loop symptoms. Include resize stalls in the Mars performance gate. |
| [#1604 Rio does not redraw the window until manual key input](https://github.com/raphamorim/rio/issues/1604) | Stale redraw context. Useful when investigating welcome-screen or focus-return rendering bugs. |
| [#1570 Renderer: present with damage to the compositor](https://github.com/raphamorim/rio/issues/1570) | Performance/compositor repaint concern. Relevant to frame-time and CPU measurement work. |

## CPU And Shutdown

| Issue | Relevance To Mars |
| --- | --- |
| [#1589 100% CPU usage when closing window on linux](https://github.com/raphamorim/rio/issues/1589) | Directly relevant to CPU spike and close/shutdown behavior. Include close-window sampling in the performance gate before shipping Mars. |

## Local Interpretation

- If latest Rio and Mars both fail the same way, keep the fix upstream-shaped or document it as an upstream Rio runtime issue.
- If latest Rio works and Mars fails, inspect Mars packaging, app id, config path, and Home Manager wiring before touching renderer code.
- Avoid forcing Vulkan ICD paths in Mars unless the same requirement is proven for Rio on the same host.
- Prefer local reproducible artifacts over assumptions from issue titles.

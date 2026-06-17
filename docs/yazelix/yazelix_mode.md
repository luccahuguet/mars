# Yazelix Mode

`mars --yazelix -e <command> [args...]` runs Mars as a Yazelix terminal
host instead of a standalone workspace terminal.

Current behavior:

- `--yazelix` requires `-e/--command`
- Mars starts exactly the requested child command
- Rio native split keybindings are disabled through `navigation.use_split = false`
- config-editor split opening is disabled through `navigation.open_config_with_split = false`
- the native tab/island UI stays hidden for a single child through `hide_if_single = true`
- the default Wayland app id / X11 class becomes `yazelix-terminal`; the desktop wrapper also honors `YAZELIX_TERMINAL_APP_ID` for parent-owned launcher identity
- `TERM_PROGRAM` is `mars` for product identity
- terminfo prefers `xterm-mars` and `mars`, with `xterm-rio` and `rio` aliases kept for Rio-compatible capability detection
- `MARS_TERMINAL_HOST` becomes `mars` for fork-specific detection

The intended launch shape is:

```text
mars --yazelix -e yzx launch
```

Zellij remains the owner of panes, tabs, sessions, layouts, and focus policy.
Rio's split and tab code can still exist for standalone Rio usage, but Yazelix
mode must keep it out of the default Yazelix workspace contract.

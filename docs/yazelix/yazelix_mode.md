# Yazelix Mode

`rioterm --yazelix -e <command> [args...]` runs Rio as a Yazelix terminal
host instead of a standalone workspace terminal.

Current behavior:

- `--yazelix` requires `-e/--command`
- Rio starts exactly the requested child command
- Rio native split keybindings are disabled through `navigation.use_split = false`
- config-editor split opening is disabled through `navigation.open_config_with_split = false`
- the native tab/island UI stays hidden for a single child through `hide_if_single = true`
- the default Wayland app id / X11 class becomes `yazelix-terminal` unless explicitly overridden
- `TERM_PROGRAM` remains `rio` so child applications detect the Rio protocol surface
- `YAZELIX_TERMINAL_HOST` becomes `yazelix-terminal` for fork-specific detection

The intended launch shape is:

```text
rioterm --yazelix -e yzx launch
```

Zellij remains the owner of panes, tabs, sessions, layouts, and focus policy.
Rio's split and tab code can still exist for standalone Rio usage, but Yazelix
mode must keep it out of the default Yazelix workspace contract.

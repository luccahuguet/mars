# Clipboard And Identity Audit

Current protocol state:

- OSC 52 read/write is implemented for the clipboard designator `c` and the
  primary/selection designators `p` and `s`
- OSC 52 stores require valid base64 and valid UTF-8
- OSC 52 stores reject encoded payloads above 2 MiB and decoded payloads above
  1 MiB
- XTVERSION replies as `Rio <version>` because the terminal core is still Rio
- Yazelix host mode keeps `TERM_PROGRAM=rio` so child applications detect the
  Rio protocol surface
- Yazelix host mode sets `YAZELIX_TERMINAL_HOST=yazelix-terminal` for
  fork-specific detection
- Yazelix host mode defaults the Wayland app id / X11 class to
  `yazelix-terminal`
- `TERM` still follows Rio's packaged terminfo discovery:
  `xterm-rio`, `rio`, then `xterm-256color`

Open audit items:

- packaged terminfo should be reviewed after every protocol milestone so it
  does not advertise capabilities that are missing or hide capabilities that
  are implemented
- OSC 52 read/write should grow an explicit user policy surface before this
  fork is used as a daily-driver terminal
- XTVERSION/DA naming should stay conservative until the fork has its own
  packaged terminfo and release identity

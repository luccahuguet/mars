# Clipboard And Identity Audit

Current protocol state:

- OSC 52 read/write is implemented for the clipboard designator `c` and the
  primary/selection designators `p` and `s`
- OSC 52 stores require valid base64 and valid UTF-8
- OSC 52 stores reject encoded payloads above 2 MiB and decoded payloads above
  1 MiB
- XTVERSION replies as `Mars <version>`
- Yazelix host mode sets `TERM_PROGRAM=mars` for product identity
- Yazelix host mode sets `MARS_TERMINAL_HOST=mars` for fork-specific detection
- Yazelix host mode defaults the Wayland app id / X11 class to
  `yazelix-terminal`
- `TERM` prefers Mars packaged terminfo while keeping Rio aliases:
  `xterm-mars`, `mars`, `xterm-rio`, `rio`, then `xterm-256color`

Open audit items:

- packaged terminfo should be reviewed after every protocol milestone so it
  does not advertise capabilities that are missing or hide capabilities that
  are implemented
- OSC 52 read/write should grow an explicit user policy surface before this
  fork is used as a daily-driver terminal
- DA naming should stay conservative until Mars has broader ecosystem
  registration evidence beyond the packaged terminfo aliases

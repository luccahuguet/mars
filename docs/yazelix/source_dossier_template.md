# Source Dossier Template

Copy this file to `docs/yazelix/dossiers/<feature_id>.md` before implementing a
non-trivial Ghostty-parity feature.

## Feature

- Feature:
- Bead:
- Parity tier: must-have | should-have | frontier
- Status: research | implementation | validation | complete | pivoted

## Source Inventory

| Project | Commit | License posture | Files/specs/probes |
| --- | --- | --- | --- |
| Rio |  | fork base |  |
| Ghostty |  | MIT |  |
| WezTerm |  | MIT |  |
| Kitty docs/specs |  | spec/black-box |  |
| Other |  |  |  |

## Behavior Target

Describe the terminal behavior in user-visible terms. Include protocol sequence
forms, response bytes, visual state, error handling, and security policy where
relevant.

## Current Rio State

List the Rio modules that currently own the behavior and summarize what exists,
what is missing, and what must not be duplicated.

## Candidate Implementation

Describe the smallest Rio-native implementation path. Name the expected owner
files and the data/control flow.

## License And Attribution Decision

Record whether the work is:

- in-house Rio/Yazelix code
- source-derived from Ghostty or WezTerm with attribution
- spec/black-box behavior from Kitty
- blocked on a license decision

## Validation Plan

List the exact evidence required:

- unit/fixture tests:
- PTY/conformance smoke:
- screenshot/framebuffer evidence:
- manual Yazelix session:
- benchmark:

## Risks

List renderer, parser, platform, security, config, or performance risks.

## Pivot Criteria

State what would make this implementation path the wrong one.

## Outcome

Fill this in before closing the bead:

- Implemented:
- Evidence:
- Remaining gaps:
- Follow-up beads:

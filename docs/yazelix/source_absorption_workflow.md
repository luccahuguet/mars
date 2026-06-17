# Source Absorption Workflow

This workflow is for implementing Ghostty-parity features in
`mars` by studying existing terminal emulators without losing
license discipline or Rio ownership clarity.

The aim is to absorb behavior, not blindly transplant architecture.

## Source Policy

| Source | License posture | Allowed use |
| --- | --- | --- |
| Rio | Fork base | Direct modification |
| Ghostty | MIT reference | Source reading, behavior porting, tests/fixtures where useful, attribution required |
| WezTerm | MIT reference | Source reading, alternate Rust design reference, tests/fixtures where useful, attribution required |
| Kitty specs | Public protocol reference | Spec-driven implementation and black-box behavior probes |
| Kitty implementation | GPL reference | No implementation copying unless a separate explicit GPL decision is made |
| Unknown license | Restricted | Treat as spec/black-box only until license is resolved |

When in doubt, record the source and do not copy implementation code.

## Workflow

1. Start from a bead and identify the parity tier

   Use the Ghostty parity contract to classify the work as must-have, should-have,
   or frontier. The first implementation question is always whether this feature
   is required for Yazelix workflows or only interesting terminal completeness.

2. Create or update a dossier

   Copy `docs/yazelix/source_dossier_template.md` into a feature-specific file
   under `docs/yazelix/dossiers/`. Fill in the source commits, license posture,
   Rio owner files, behavior target, validation plan, and pivot criteria before
   making implementation edits.

3. Inspect Rio first

   Find the current Rio owner path and the smallest local seam. Prefer extending
   existing parser, renderer, config, or frontend boundaries over creating a
   parallel implementation path.

4. Compare Ghostty and WezTerm

   Use Ghostty as the primary parity target. Use WezTerm to cross-check whether
   the behavior is terminal consensus or a Ghostty-specific choice. For permissive
   source-derived behavior, record upstream files and commits in the dossier.

5. Treat Kitty as spec and behavior

   For Kitty protocols, use the public protocol docs and black-box probes. Do not
   copy GPL implementation structure or code. If the public spec is ambiguous,
   record the observed behavior and the probe command.

6. Implement against behavior

   The accepted result is user-visible behavior, not a source shape. It is fine
   if Rio's implementation differs from Ghostty when protocol output, visuals,
   and Yazelix workflow behavior match.

7. Validate at the right level

   Parser changes need unit/fixture tests. Renderer changes need screenshots or
   framebuffer evidence. Yazelix-facing changes need a manual session through
   Zellij/Yazi/Helix/shell before they are considered real.

8. Commit each finished bead

   Close the bead, flush Beads, and commit code/docs/evidence together. Keep hard
   problems documented instead of forcing a weak partial implementation.

## Evidence Ladder

Parity claims should climb as high as the feature requires:

- Level 0: unsupported claim, not accepted
- Level 1: source references and behavior notes
- Level 2: parser/unit tests
- Level 3: PTY/conformance smoke test or screenshot/framebuffer evidence
- Level 4: manual Yazelix session evidence through the real stack

Must-have features should reach Level 4 before release-quality claims. Exploratory
frontier features may stop earlier if the hard problem is documented.

## Attribution Rules

When code or tests are derived from Ghostty or WezTerm:

- name the upstream project and file in the dossier
- preserve or add license headers when moving non-trivial code
- prefer a focused code comment only when it helps future maintainers trace the
  behavior
- do not hide source-derived logic inside large unrelated refactors

For Kitty:

- cite public protocol documentation or probe output
- do not paste GPL implementation snippets into docs, tests, comments, or code
- if a future decision accepts GPL code, record that decision before any copy

## AI-Agent Use

AI agents may help inspect upstream projects, but their output must be treated as
analysis until verified locally.

Good agent task:

```text
Read Ghostty and WezTerm source for OSC 133. Return a behavior summary, file
paths, commit ids, test names, and edge cases. Do not propose copied Kitty code.
```

Bad agent task:

```text
Copy Kitty's implementation of OSC 133 into Rio.
```

Agent output should be condensed into a dossier. Do not commit long raw agent
transcripts.

## Hard Problem Rule

If a feature looks much larger than expected, stop and document:

- the exact technical blocker
- the owner boundary that made it hard
- whether the problem is renderer, parser, platform, config, or license related
- the smallest next bead that can make progress
- whether the parity contract should change

Then move to another unblocked bead rather than forcing a brittle solution.

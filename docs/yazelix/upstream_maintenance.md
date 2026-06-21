# Upstream Maintenance

Mars should stay easy to refresh from upstream Rio.

## Rule

Prefer additive Mars-owned files over edits to Rio-owned files.

Good first choices:

- Add a small wrapper package around the upstream Rio package
- Add Mars desktop/app/icon metadata beside Rio metadata
- Add private maintainer test config under Mars-owned paths
- Add dogfooding scripts and docs that do not alter terminal behavior

Avoid by default:

- Renaming Rio crates, modules, or source directories
- Editing renderer, PTY, event-loop, parser, or window code without measured evidence
- Replacing Rio config behavior just to brand Mars
- Carrying generated shader/background/runtime assets in the clean baseline
- Broad search-and-replace changes from `rio` to `mars`

## Exception Bar

Editing a Rio-owned source file is allowed only when:

- The behavior is required for a current Mars feature
- The change is smaller than an additive alternative
- The reason is recorded in a Bead
- A focused test or dogfooding artifact protects the behavior

## Clean Rebuild Baseline

The clean rebuild branch starts from `rio-upstream/main`.

The first milestone is intentionally boring:

- Rio source behavior remains unchanged
- Mars package output wraps the Rio binary
- Mars desktop metadata and icon are additive
- Private Yazelix testing uses Mars-owned config and scripts
- Performance gates run before shaders, background integration, PTY pacing, or event scheduling work returns

When pulling from upstream Rio, review fork delta with:

```sh
git diff --stat rio-upstream/main
git diff --name-status rio-upstream/main
```

The expected early diff should be Mars-owned wrapper, docs, config, icon, and gate files. Any Rio-owned source file in that diff needs a current Bead explaining why it exists.

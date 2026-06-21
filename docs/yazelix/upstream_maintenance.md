# Upstream Maintenance

Mars should stay easy to refresh from upstream Rio.

## Rule

Prefer additive Mars-owned files over edits to Rio-owned files.

Use this change ladder:

1. Configuration, wrapper, package, desktop metadata, icons, docs, and dogfooding tools
2. Existing upstream Rio extension points
3. Tiny Rio call-site hooks into Mars-owned modules
4. Direct Rio source edits when a hook is larger or less clear than the direct change
5. Upstreamable Rio changes when the behavior is useful beyond Mars

Good first choices:

- Add a small wrapper package around the upstream Rio package
- Add Mars desktop/app/icon metadata beside Rio metadata
- Add private maintainer test config under Mars-owned paths
- Use the same Rio-compatible config content for Rio and Mars while isolating them by config directory
- Add dogfooding scripts and docs that do not alter terminal behavior

Avoid by default:

- Renaming Rio crates, modules, or source directories
- Editing renderer, PTY, event-loop, parser, or window code without measured evidence
- Replacing Rio config behavior just to brand Mars
- Carrying generated shader/background/runtime assets in the clean baseline
- Broad search-and-replace changes from `rio` to `mars`
- Scattering Yazelix-specific conditionals through Rio source files

## Exception Bar

Editing a Rio-owned source file is allowed only when:

- The behavior is required for a current Mars feature
- The change is smaller than an additive alternative
- The reason is recorded in a Bead
- Ghostty and WezTerm equivalent behavior has been inspected before editing
- Rio GitHub issues have been searched for the same symptom or protocol area
- Newer upstream Rio commits have been checked for an existing fix
- A focused test or dogfooding artifact protects the behavior

Before editing, record the exact checks in the Bead or scorecard. Prefer:

```sh
gh issue list --repo raphamorim/rio --state all --search '<symptom or protocol keywords>'
git fetch rio-upstream main
base=$(git merge-base HEAD rio-upstream/main)
git log --oneline "$base"..rio-upstream/main -- <suspect-rio-paths>
git log --oneline --grep='<keyword>' "$base"..rio-upstream/main
```

If GitHub or network access is unavailable, record that explicitly and keep the change blocked unless the maintainer accepts the risk.

## Mars-Owned Hooks

Mars-owned helper modules are acceptable when they reduce upstream merge churn.

Good hooks:

- Touch a stable Rio call site with one or two obvious lines
- Pass explicit inputs and return explicit values
- Keep one feature per helper module
- Use `mars::...` or `fork::...` ownership naming inside terminal code
- Stay easy to delete if the feature is rejected or upstreamed

Bad hooks:

- Hide a large fork behind a small function call
- Read global Yazelix state from deep terminal code
- Spread the same feature across many Rio files
- Create a generic helper module that becomes a catch-all
- Use `yazelix::...` naming inside terminal code unless the behavior is truly about launching or integrating with Yazelix

When a direct Rio edit is smaller and clearer than an abstraction, prefer the direct edit and document why.

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

Every Mars-owned code or runtime-behavior commit also needs a row in `docs/yazelix/change_scorecard.md` explaining why the commit exists, which Rio-owned files it touches, how it was verified, the expected upstream merge cost, and whether it is the smallest, simplest, surgical, non-invasive path. Pure documentation-only commits are exempt.

For Rio-owned source edits, the verification notes must include the Ghostty and WezTerm paths or docs checked before the edit, the Rio GitHub issue query/result, and the upstream commit range checked. If a terminal has no equivalent path, Rio has no matching issue, or newer Rio has no matching fix, say so in the row.

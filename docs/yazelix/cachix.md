# Cachix Cache

Yazelix Terminal uses the Cachix cache `luccahuguet-yazelix-terminal` for
Linux package builds.

## Trust Model

The cache is intended to be public and project-scoped:

- Cache name: `luccahuguet-yazelix-terminal`
- Substituter: `https://luccahuguet-yazelix-terminal.cachix.org`
- Public key: `luccahuguet-yazelix-terminal.cachix.org-1:NwYldFPOxjg4cjLoU9jZW9rrd/Jj60PzksvRXhDy574=`
- Signing: Cachix-managed signing key
- Writers: GitHub Actions on this repository's `main` branch and manual
  `workflow_dispatch` runs
- Pull requests: no cache-publishing workflow runs for `pull_request`, so forked
  or untrusted PR code does not receive the write token

By using this cache, users trust binaries built by the repository CI with write
access to this Cachix cache.

## Maintainer Setup

Create the cache on Cachix if it does not exist:

1. Open <https://app.cachix.org/>.
2. Create `luccahuguet-yazelix-terminal`.
3. Keep it public if the goal is speeding up normal Home Manager/runtime users.
4. Use Cachix-managed signing unless there is a specific reason to own the
   signing key locally.

Create a per-cache write token. A read token or a token for another cache will
let the workflow start but will fail when it tries to upload packages.

1. Open the cache settings.
2. Open access tokens.
3. Generate a write token for CI.
4. Copy it immediately; generate a new token if it is lost.

Install it as a GitHub Actions secret:

```sh
gh secret set CACHIX_AUTH_TOKEN --repo luccahuguet/yazelix-terminal
```

Verify GitHub has the secret name:

```sh
gh secret list --repo luccahuguet/yazelix-terminal
```

The workflow `.github/workflows/cachix.yml` publishes these x86_64-linux
outputs on pushes to `main` and manual runs:

```sh
nix build \
  .#packages.x86_64-linux.yazelix-terminal \
  .#packages.x86_64-linux.yazelix-terminal-fast
```

To push from a local machine for testing:

```sh
export CACHIX_AUTH_TOKEN='...'
nix build .#yazelix-terminal -o result_yazelix_terminal_package
cachix push luccahuguet-yazelix-terminal result_yazelix_terminal_package
```

## User Setup

If the cache is public, users do not need a token. Configure Nix with:

```sh
cachix use luccahuguet-yazelix-terminal
```

For declarative Nix or Home Manager setups, add the substituter and public key
that Cachix prints for this cache. The shape is:

```nix
{
  nix.settings.extra-substituters = [
    "https://luccahuguet-yazelix-terminal.cachix.org?priority=30"
  ];
  nix.settings.extra-trusted-public-keys = [
    "luccahuguet-yazelix-terminal.cachix.org-1:NwYldFPOxjg4cjLoU9jZW9rrd/Jj60PzksvRXhDy574="
  ];
}
```

If the cache is private, users also need a read token:

```sh
cachix authtoken '...'
cachix use luccahuguet-yazelix-terminal
```

## Substitution Check

After CI has pushed a build for the current revision, verify substitution from a
machine that has the cache configured:

```sh
nix build .#yazelix-terminal \
  --option substituters 'https://cache.nixos.org https://luccahuguet-yazelix-terminal.cachix.org' \
  --option trusted-public-keys 'cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY= luccahuguet-yazelix-terminal.cachix.org-1:NwYldFPOxjg4cjLoU9jZW9rrd/Jj60PzksvRXhDy574=' \
  --print-build-logs
```

Expected result: Nix downloads the `yazelix-terminal` and
`yazelix-terminal-unwrapped` paths instead of compiling `rioterm` locally.

If Nix still builds locally, check:

- the GitHub secret is a write token for `luccahuguet-yazelix-terminal`
- the CI workflow completed for the same commit and system
- the cache is public or the read token is configured
- the trusted public key matches Cachix
- the local Nix daemon has picked up config changes
- Nix has not cached a previous negative lookup for the same store path

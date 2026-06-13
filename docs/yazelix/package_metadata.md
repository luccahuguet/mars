# Yzxterm Package Metadata

Yazelix Terminal exposes explicit metadata so main Yazelix can select packages
and profiles without parsing yzxterm config files or shader assets.

The metadata is available in two forms:

- `passthru.yzxtermPackageMetadata` on the Nix package
- `share/yazelix-terminal/package-metadata.json` in the package output

Both forms use the same schema. Field names in the JSON output are stable for
main Yazelix consumption.

## Schema

| Field | Meaning |
| --- | --- |
| `schema_version` | Metadata schema version. Current value: `1` |
| `terminal` | Terminal identity. Current value: `yazelix-terminal` |
| `package_name` | Package derivation name, such as `yazelix-terminal` or `yazelix-terminal-fast` |
| `package_profile` | Package profile. This flake emits `release` and `fast`; explicitly local wrappers may use `local` |
| `checked_package` | Whether the package is the checked release-style package |
| `metadata_path` | Relative path to the package-output JSON metadata |
| `supported_profiles` | Stable profile names that the wrapper accepts |
| `default_profile` | Profile selected by default, currently `full` |
| `baseline_profile` | No-effects profile name, currently `baseline` |
| `shader_profile` | Opt-in shader profile name, currently `shaders` |
| `supported_appearance_modes` | Yazelix `appearance.mode` values supported by the packaged yzxterm configs. Current value: `["dark", "light", "auto"]` |
| `default_appearance_mode` | Appearance mode selected when the parent runtime does not override it. Current value: `dark` |
| `supported_emoji_fonts` | Stable emoji fallback preset names that the wrapper accepts |
| `default_emoji_font` | Emoji fallback selected by default, currently `noto` |
| `emoji_fonts` | Family names and profile config roots for each supported emoji fallback preset |
| `shader_asset_root` | Relative directory for terminal-owned shader assets |
| `config_roots` | Relative config roots for each stable profile |
| `wrapper_commands` | Relative package commands for terminal, desktop wrapper, and Rio compatibility |
| `wrapper_env` | Environment variables understood by the wrapper |
| `main_yazelix_boundary` | Human-readable boundary reminder |

`wrapper_env` currently advertises `profile`, `effects`, `config`, `app_id`,
`render_strategy`, `graphics_wrapper`, `appearance`, and `emoji_font`. `app_id` lets an
integrated parent runtime associate the terminal window with its own desktop
entry while the standalone yzxterm package keeps `yazelix-terminal`.
`appearance` selects `dark`, `light`, or `auto`; the wrapper default is `dark`
for continuity with earlier yzxterm packages.
`emoji_font` selects one of `noto`, `twitter`, or `serenityos`.

## Release And Fast Distinction

The normal package exposes:

```json
{
  "package_profile": "release",
  "checked_package": true
}
```

The fast package exposes:

```json
{
  "package_profile": "fast",
  "checked_package": false
}
```

Main Yazelix should use these fields to distinguish release evidence from local
iteration packages. It should not infer that distinction from package names,
store paths, Cargo profiles, or config contents.

For explicit local experiments, a wrapper or downstream package may set
`package_profile = "local"` and `checked_package = false`. Main Yazelix should
treat that as an explicit metadata value, not as something inferred from a
filesystem path.

## Consumption Boundary

Main Yazelix may:

- select a yzxterm package by metadata
- select one of the advertised `supported_profiles`
- select one of the advertised `supported_appearance_modes`
- select one of the advertised `supported_emoji_fonts`
- use `wrapper_commands.terminal` or `wrapper_commands.desktop` to launch the
  package
- locate yzxterm-owned shader assets through `shader_asset_root` for diagnostics

Main Yazelix may pass an advertised value through
`wrapper_env.appearance`. The yzxterm package owns the Rio `adaptive-theme`
pair, dark/light palettes, and `force-theme` wiring. Main Yazelix must not
synthesize yzxterm light or adaptive Rio config by parsing or editing terminal
config files.

Main Yazelix must stay terminal-agnostic for Ghostty, Kitty, WezTerm, Ratty, and
host terminal choices. Those terminal variants should expose their own metadata
or config boundaries instead of borrowing yzxterm-specific fields.

## Cheap Validation

`python3 tools/yazelix_conformance.py verify` checks that the metadata source
defines a package-output JSON file, exposes `passthru.yzxtermPackageMetadata`,
advertises the packaged appearance modes, validates the terminal-owned theme
files, and sets distinct release/fast profile fields in `flake.nix`.

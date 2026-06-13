use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, String>;

const DEFAULT_ENV_OUTPUT: &[&str] = &["artifacts", "conformance", "env.json"];
const DEFAULT_SCREENSHOT_DIR: &[&str] = &["artifacts", "conformance", "screenshots"];
const DEFAULT_CPU_CONFIG: &[&str] = &["artifacts", "conformance", "rio_cpu_config"];
const DEFAULT_SHADER_SCREENSHOT_DIR: &[&str] =
    &["artifacts", "shader_probe", "screenshots"];
const DEFAULT_SHADER_CONFIG: &[&str] = &["artifacts", "shader_probe", "rio_wgpu_config"];
const ALLOWED_FIXTURE_KINDS: &[&str] = &["protocol", "visual-probe", "comparison"];
const ALLOWED_FIXTURE_SOURCES: &[&str] = &[
    "kitty-spec",
    "kitty-behavior",
    "ghostty-behavior",
    "wezterm-behavior",
    "xterm",
    "iterm2",
    "de-facto",
    "rio-implementation",
];
const ALLOWED_COMPARISON_TARGETS: &[&str] = &["kitty", "ghostty", "wezterm"];

#[derive(Parser)]
#[command(about = "Protocol conformance harness for the Yazelix terminal experiment.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List fixture ids and hashes.
    List,
    /// Write one fixture byte stream.
    Emit { fixture: String },
    /// Validate fixtures, profile config, metadata, and keyboard manifests.
    Verify,
    /// List Kitty keyboard black-box comparison cases.
    KeyboardList,
    /// Capture Kitty keyboard bytes from the terminal running this command.
    KeyboardCapture {
        #[arg(long)]
        terminal: String,
        #[arg(long = "case")]
        case_ids: Vec<String>,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_t = 4.0)]
        timeout: f64,
        #[arg(long = "idle-timeout", default_value_t = 0.35)]
        idle_timeout: f64,
    },
    /// Verify a keyboard capture JSON report against checked-in expectations.
    KeyboardVerifyCapture {
        capture: PathBuf,
        #[arg(long)]
        require_all: bool,
    },
    /// Record local version/source evidence.
    RecordEnv {
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value = "target/debug/rio")]
        rio_bin: String,
    },
    /// Launch Rio with CPU renderer and capture a COSMIC screenshot.
    LaunchCpuScreenshot {
        #[arg(long)]
        output_dir: Option<PathBuf>,
        #[arg(long)]
        config_dir: Option<PathBuf>,
        #[arg(long, default_value = "target/debug/rio")]
        rio_bin: String,
        #[arg(long, default_value_t = 8)]
        sleep_seconds: u64,
        #[arg(long, default_value_t = 2)]
        settle_seconds: u64,
    },
    /// Launch Rio with WGPU custom shader probe and capture a COSMIC screenshot.
    LaunchWgpuShaderScreenshot {
        #[arg(long)]
        output_dir: Option<PathBuf>,
        #[arg(long)]
        config_dir: Option<PathBuf>,
        #[arg(long, default_value = "target/debug/rio")]
        rio_bin: String,
        #[arg(long, default_value = "vulkan")]
        wgpu_backend: String,
        #[arg(long)]
        shader: Vec<String>,
        #[arg(long, default_value_t = 8)]
        sleep_seconds: u64,
        #[arg(long, default_value_t = 2)]
        settle_seconds: u64,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = repo_root()?;
    match cli.command {
        Commands::List => command_list(&root),
        Commands::Emit { fixture } => command_emit(&root, &fixture),
        Commands::Verify => command_verify(&root),
        Commands::KeyboardList => command_keyboard_list(&root),
        Commands::KeyboardCapture {
            terminal,
            case_ids,
            output,
            timeout,
            idle_timeout,
        } => command_keyboard_capture(
            &root,
            &terminal,
            &case_ids,
            output,
            timeout,
            idle_timeout,
        ),
        Commands::KeyboardVerifyCapture {
            capture,
            require_all,
        } => command_keyboard_verify_capture(&root, &capture, require_all),
        Commands::RecordEnv { output, rio_bin } => {
            command_record_env(&root, output, &rio_bin)
        }
        Commands::LaunchCpuScreenshot {
            output_dir,
            config_dir,
            rio_bin,
            sleep_seconds,
            settle_seconds,
        } => command_launch_cpu_screenshot(
            &root,
            output_dir,
            config_dir,
            &rio_bin,
            sleep_seconds,
            settle_seconds,
        ),
        Commands::LaunchWgpuShaderScreenshot {
            output_dir,
            config_dir,
            rio_bin,
            wgpu_backend,
            shader,
            sleep_seconds,
            settle_seconds,
        } => command_launch_wgpu_shader_screenshot(
            &root,
            output_dir,
            config_dir,
            &rio_bin,
            &wgpu_backend,
            &shader,
            sleep_seconds,
            settle_seconds,
        ),
    }
}

fn repo_root() -> Result<PathBuf> {
    if let Ok(root) = env::var("YAZELIX_CONFORMANCE_ROOT") {
        return Ok(PathBuf::from(root));
    }
    let current_dir = env::current_dir().map_err(|err| err.to_string())?;
    if manifest_path(&current_dir).is_file() {
        return Ok(current_dir);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to derive repository root".to_string())
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join("conformance")
        .join("fixtures")
        .join("manifest.json")
}

fn keyboard_manifest_path(root: &Path) -> PathBuf {
    root.join("conformance")
        .join("fixtures")
        .join("kitty_keyboard_blackbox.json")
}

fn load_json(path: &Path) -> Result<Value> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

fn load_manifest(root: &Path) -> Result<Value> {
    let path = manifest_path(root);
    let manifest = load_json(&path)?;
    if manifest.get("version").and_then(Value::as_i64) != Some(1) {
        return Err(format!(
            "unsupported manifest version in {}",
            path.display()
        ));
    }
    Ok(manifest)
}

fn load_keyboard_manifest(root: &Path) -> Result<Value> {
    let path = keyboard_manifest_path(root);
    let manifest = load_json(&path)?;
    if manifest.get("version").and_then(Value::as_i64) != Some(1) {
        return Err(format!(
            "unsupported keyboard manifest version in {}",
            path.display()
        ));
    }
    Ok(manifest)
}

fn array_field<'a>(value: &'a Value, key: &str, context: &str) -> Result<&'a Vec<Value>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{context} missing {key}"))
}

fn object_field<'a>(
    value: &'a Value,
    key: &str,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{context} missing {key}"))
}

fn str_field<'a>(value: &'a Value, key: &str, context: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{context} missing {key}"))
}

fn fixture_bytes(fixture: &Value) -> Result<Vec<u8>> {
    let id = fixture
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>");
    let hex_text = str_field(fixture, "hex", "fixture")?;
    hex::decode(hex_text).map_err(|err| format!("fixture {id} has invalid hex: {err}"))
}

fn hex_bytes(value: &str, context: &str) -> Result<Vec<u8>> {
    hex::decode(value).map_err(|err| format!("{context} has invalid hex: {err}"))
}

fn command_list(root: &Path) -> Result<()> {
    let manifest = load_manifest(root)?;
    for fixture in array_field(&manifest, "fixtures", "manifest")? {
        let data = fixture_bytes(fixture)?;
        println!(
            "{}\t{}\t{}\t{} bytes\tsha256={}",
            str_field(fixture, "id", "fixture")?,
            str_field(fixture, "tier", "fixture")?,
            str_field(fixture, "protocol", "fixture")?,
            data.len(),
            sha256_hex(&data)
        );
    }
    Ok(())
}

fn command_emit(root: &Path, fixture_id: &str) -> Result<()> {
    let manifest = load_manifest(root)?;
    let fixtures = array_field(&manifest, "fixtures", "manifest")?;
    for fixture in fixtures {
        if fixture.get("id").and_then(Value::as_str) == Some(fixture_id) {
            io::stdout()
                .write_all(&fixture_bytes(fixture)?)
                .map_err(|err| err.to_string())?;
            return Ok(());
        }
    }
    let known = fixtures
        .iter()
        .filter_map(|fixture| fixture.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "unknown fixture {fixture_id:?}; known fixtures: {known}"
    ))
}

fn command_verify(root: &Path) -> Result<()> {
    let manifest = load_manifest(root)?;
    let mut seen = HashSet::new();
    for fixture in array_field(&manifest, "fixtures", "manifest")? {
        let fixture_id = fixture
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| "fixture missing id".to_string())?;
        if !seen.insert(fixture_id.to_string()) {
            return Err(format!("duplicate fixture id: {fixture_id}"));
        }
        validate_fixture_metadata(fixture, &format!("fixture {fixture_id}"))?;
        let data = fixture_bytes(fixture)?;
        if data.is_empty() {
            return Err(format!("fixture {fixture_id} is empty"));
        }
        println!(
            "ok {fixture_id} {} bytes sha256={}",
            data.len(),
            sha256_hex(&data)
        );
    }

    let shader = root
        .join("conformance")
        .join("shaders")
        .join("ghostty_cursor_probe.glsl");
    let shader_text = read_text(&shader)?;
    for required in [
        "iChannel0",
        "iResolution",
        "iCurrentCursor",
        "iCurrentCursorColor",
        "iCursorVisible",
    ] {
        ensure_contains(
            &shader_text,
            required,
            &format!("shader probe missing {required}"),
        )?;
    }
    println!("ok {}", rel(root, &shader));

    validate_yazelix_shader_assets(root)?;
    validate_yazelix_profile_configs(root)?;
    validate_yazelix_theme_configs(root)?;
    validate_yazelix_font_config(root)?;
    validate_package_metadata_sources(root)?;
    validate_keyboard_manifest(root)?;
    println!("ok {}", rel(root, &keyboard_manifest_path(root)));
    Ok(())
}

fn validate_yazelix_shader_assets(root: &Path) -> Result<()> {
    let shader_root = root.join("misc").join("yazelix_terminal_shaders");
    let cursor_trail = shader_root.join("cursor_trail_dusk.glsl");
    let cursor_trail_text = read_text(&cursor_trail)?;
    for required in [
        "YAZELIX_TRAIL_GLOW_STRENGTH",
        "YAZELIX_TRAIL_GLOW_WIDTH_SCALE",
        "trailGlowMask",
        "trailEdgeMask",
        "trailCoreMask",
        "cursorGlowMask",
        "cursorEdgeMask",
        "yazelixRioTrailSdf",
        "#if defined(YAZELIX_TERMINAL_RIO_TRAIL)",
    ] {
        ensure_contains(
            &cursor_trail_text,
            required,
            &format!("{} missing {required}", rel(root, &cursor_trail)),
        )?;
    }

    let generated_effects = [
        shader_root.join("generated_effects").join("sweep.glsl"),
        shader_root
            .join("generated_effects")
            .join("rectangle_boom.glsl"),
    ];
    for effect in &generated_effects {
        let text = read_text(effect)?;
        for required in ["mainImage", "iCurrentCursor", "iPreviousCursor"] {
            ensure_contains(
                &text,
                required,
                &format!("{} missing {required}", rel(root, effect)),
            )?;
        }
    }
    println!("ok {}", rel(root, &cursor_trail));
    for effect in generated_effects {
        println!("ok {}", rel(root, &effect));
    }
    Ok(())
}

fn validate_yazelix_profile_configs(root: &Path) -> Result<()> {
    let profile_configs = [
        (
            "full",
            root.join("misc").join("yazelix_terminal_config.toml"),
        ),
        (
            "baseline",
            root.join("misc")
                .join("yazelix_terminal_config_baseline.toml"),
        ),
        (
            "shaders",
            root.join("misc")
                .join("yazelix_terminal_config_shaders.toml"),
        ),
    ];
    for (profile, path) in profile_configs {
        let text = read_text(&path)?;
        for required in [
            r#"adaptive-theme = { dark = "yazelix-dark", light = "yazelix-light" }"#,
            "[cursor]",
            "blinking = true",
            "blinking-interval = 650",
        ] {
            ensure_contains(
                &text,
                required,
                &format!("{} missing {required}", rel(root, &path)),
            )?;
        }
        if profile == "baseline" {
            if text.contains("trail-cursor") || text.contains("custom-shader") {
                return Err(format!(
                    "{} must stay a no-effects profile",
                    rel(root, &path)
                ));
            }
        } else if !text.contains("trail-cursor = true") {
            return Err(format!("{} must keep Rio trail enabled", rel(root, &path)));
        }
        if profile == "shaders" && !text.contains("custom-shader") {
            return Err(format!(
                "{} must keep custom shader profile",
                rel(root, &path)
            ));
        }
        println!("ok {} {profile} profile", rel(root, &path));
    }
    Ok(())
}

fn validate_yazelix_theme_configs(root: &Path) -> Result<()> {
    let theme_files = [
        (
            "dark",
            root.join("misc").join("yazelix_terminal_theme_dark.toml"),
        ),
        (
            "light",
            root.join("misc").join("yazelix_terminal_theme_light.toml"),
        ),
    ];
    let required_colors = [
        "background",
        "foreground",
        "cursor",
        "vi-cursor",
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "white",
        "light-black",
        "light-red",
        "light-green",
        "light-yellow",
        "light-blue",
        "light-magenta",
        "light-cyan",
        "light-white",
        "tabs",
        "tab-border",
        "tabs-active",
        "bar",
        "split",
        "split-active",
        "selection-background",
        "selection-foreground",
        "search-match-background",
        "search-match-foreground",
        "search-focused-match-background",
        "search-focused-match-foreground",
        "hint-background",
        "hint-foreground",
    ];
    for (mode, path) in theme_files {
        let parsed = parse_toml(&path)?;
        let colors = parsed
            .get("colors")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{} missing [colors]", rel(root, &path)))?;
        for color in required_colors {
            if !colors.contains_key(color) {
                return Err(format!("{} missing {color}", rel(root, &path)));
            }
        }
        println!("ok {} {mode} theme", rel(root, &path));
    }
    Ok(())
}

fn validate_yazelix_font_config(root: &Path) -> Result<()> {
    let fonts = root.join("misc").join("yazelix_terminal_fonts.toml");
    let text = read_text(&fonts)?;
    let parsed = parse_toml(&fonts)?;
    let symbol_map = parsed
        .get("fonts")
        .and_then(|fonts| fonts.get("symbol-map"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{} missing fonts.symbol-map", rel(root, &fonts)))?;
    let font_size = parsed
        .get("fonts")
        .and_then(|fonts| fonts.get("size"))
        .and_then(toml::Value::as_float)
        .ok_or_else(|| format!("{} missing fonts.size", rel(root, &fonts)))?;
    if font_size != 17.0 {
        return Err(format!(
            "{} fonts.size is {font_size}, expected 17.0",
            rel(root, &fonts)
        ));
    }
    let emoji_family_placeholder = "@yazelix_terminal_emoji_font_family@";
    for required in [
        r#"font-family = "@yazelix_terminal_emoji_font_family@""#,
        r#"start = "2600", end = "2605""#,
        r#"start = "26A0", end = "26A2""#,
        r#"start = "2744", end = "2745""#,
        r#"start = "2B50", end = "2B51""#,
        r#"start = "1F000", end = "1FB00""#,
        r#"font-family = "Symbols Nerd Font Mono""#,
    ] {
        ensure_contains(
            &text,
            required,
            &format!("{} missing {required}", rel(root, &fonts)),
        )?;
    }
    for forbidden in [
        r#"font-family = "Noto Color Emoji""#,
        r#"font-family = "Noto Emoji""#,
        r#"start = "2600", end = "2800""#,
        r#"start = "2B00", end = "2C00""#,
    ] {
        if text.contains(forbidden) {
            return Err(format!("{} still contains {forbidden}", rel(root, &fonts)));
        }
    }

    for (name, codepoint) in [
        ("cloud", 0x2601),
        ("coffee", 0x2615),
        ("lightning", 0x26A1),
        ("snowflake", 0x2744),
        ("star", 0x2B50),
        ("package", 0x1F4E6),
        ("snake", 0x1F40D),
        ("crab", 0x1F980),
    ] {
        let family = mapped_family(symbol_map, codepoint)?;
        if family.as_deref() != Some(emoji_family_placeholder) {
            return Err(format!(
                "{} maps {name} U+{codepoint:04X} to {:?}, expected {:?}",
                rel(root, &fonts),
                family,
                emoji_family_placeholder
            ));
        }
    }
    if mapped_family(symbol_map, 0x276F)?.as_deref() == Some(emoji_family_placeholder) {
        return Err(format!(
            "{} maps prompt arrow U+276F to emoji",
            rel(root, &fonts)
        ));
    }

    let pkg = root.join("pkgRio.nix");
    let pkg_text = read_text(&pkg)?;
    for required_pkg_pattern in [
        "noto-fonts-color-emoji",
        "twitter-color-emoji",
        "serenityos-emoji-font",
        "supported_emoji_fonts",
        "YAZELIX_TERMINAL_EMOJI_FONT",
    ] {
        ensure_contains(
            &pkg_text,
            required_pkg_pattern,
            &format!("{} missing {required_pkg_pattern}", rel(root, &pkg)),
        )?;
    }
    if pkg_text.contains("noto-fonts-monochrome-emoji") {
        return Err(format!(
            "{} still packages monochrome emoji",
            rel(root, &pkg)
        ));
    }
    println!("ok {}", rel(root, &fonts));
    Ok(())
}

fn mapped_family(symbol_map: &[toml::Value], codepoint: u32) -> Result<Option<String>> {
    for entry in symbol_map {
        let start = entry
            .get("start")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "font symbol-map entry missing start".to_string())?;
        let end = entry
            .get("end")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "font symbol-map entry missing end".to_string())?;
        let family = entry
            .get("font-family")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "font symbol-map entry missing font-family".to_string())?;
        let start = u32::from_str_radix(start, 16).map_err(|err| {
            format!("font symbol-map start {start:?} is invalid: {err}")
        })?;
        let end = u32::from_str_radix(end, 16)
            .map_err(|err| format!("font symbol-map end {end:?} is invalid: {err}"))?;
        if start <= codepoint && codepoint < end {
            return Ok(Some(family.to_string()));
        }
    }
    Ok(None)
}

fn validate_package_metadata_sources(root: &Path) -> Result<()> {
    let sources = [
        (
            root.join("pkgRio.nix"),
            vec![
                r#"packageProfile ? "release""#,
                "packageChecked ? true",
                "yzxtermPackageMetadata",
                "package_profile = packageProfile",
                "checked_package = packageChecked",
                "supported_appearance_modes",
                r#""dark""#,
                r#""light""#,
                r#""auto""#,
                r#"default_appearance_mode = "dark""#,
                r#"appearance = "YAZELIX_TERMINAL_APPEARANCE""#,
                "install_yazelix_themes",
                "yazelix_terminal_theme_dark.toml",
                "yazelix_terminal_theme_light.toml",
                "yazelix-dark.toml",
                "yazelix-light.toml",
                "share/yazelix-terminal/package-metadata.json",
                "passthru",
            ],
        ),
        (
            root.join("misc").join("yazelix_terminal_desktop.sh"),
            vec![
                "select_appearance_mode",
                "YAZELIX_TERMINAL_APPEARANCE",
                "write_effective_config",
                "force-theme",
            ],
        ),
        (
            root.join("flake.nix"),
            vec![
                r#"packageProfile = "release";"#,
                "packageChecked = true;",
                r#"packageProfile = "fast";"#,
                "packageChecked = false;",
            ],
        ),
    ];
    for (path, required_patterns) in sources {
        let text = read_text(&path)?;
        for required in required_patterns {
            ensure_contains(
                &text,
                required,
                &format!("{} missing {required}", rel(root, &path)),
            )?;
        }
        println!("ok {} metadata source", rel(root, &path));
    }
    Ok(())
}

fn command_keyboard_list(root: &Path) -> Result<()> {
    let manifest = load_keyboard_manifest(root)?;
    for case in array_field(&manifest, "cases", "keyboard manifest")? {
        let expect = object_field(case, "expect", "keyboard case")?;
        let mode = expect
            .get("mode")
            .and_then(Value::as_str)
            .ok_or_else(|| "keyboard case expect missing mode".to_string())?;
        let expectation = if mode == "exact" {
            expect
                .get("hex")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        } else {
            expect
                .get("fragments")
                .and_then(Value::as_array)
                .map(|fragments| {
                    fragments
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default()
        };
        println!(
            "{}\t{}\t{}\t{}\t{}",
            str_field(case, "id", "keyboard case")?,
            str_field(case, "tier", "keyboard case")?,
            str_field(case, "keys", "keyboard case")?,
            mode,
            expectation
        );
    }
    Ok(())
}

fn selected_keyboard_cases(
    manifest: &Value,
    selected_ids: &[String],
) -> Result<Vec<Value>> {
    let manifest_cases = array_field(manifest, "cases", "keyboard manifest")?;
    if selected_ids.is_empty() {
        return Ok(manifest_cases.clone());
    }

    let cases = keyboard_cases_by_id(manifest)?;
    selected_ids
        .iter()
        .map(|case_id| {
            cases.get(case_id).cloned().ok_or_else(|| {
                let mut known = cases.keys().cloned().collect::<Vec<_>>();
                known.sort();
                format!(
                    "unknown keyboard case {case_id:?}; known cases: {}",
                    known.join(", ")
                )
            })
        })
        .collect()
}

fn command_keyboard_capture(
    root: &Path,
    terminal: &str,
    case_ids: &[String],
    output: Option<PathBuf>,
    timeout: f64,
    idle_timeout: f64,
) -> Result<()> {
    let manifest = load_keyboard_manifest(root)?;
    let cleanup = hex_bytes(
        manifest
            .get("cleanup_hex")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        "keyboard cleanup",
    )?;
    let cases = selected_keyboard_cases(&manifest, case_ids)?;
    let output = output.map(|path| expand_tilde(&path)).unwrap_or_else(|| {
        root.join("artifacts")
            .join("conformance")
            .join("keyboard_captures")
            .join(format!("{terminal}.json"))
    });
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("{}: {err}", parent.display()))?;
    }

    let mut captured_cases = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let case_id = str_field(case, "id", "keyboard case")?;
        eprintln!(
            "[{}/{}] {}: {}\n  {}\n  Press Enter to arm capture, then press the requested key sequence.",
            index + 1,
            cases.len(),
            case_id,
            str_field(case, "keys", "keyboard case")?,
            str_field(case, "workflow", "keyboard case")?
        );
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|err| err.to_string())?;

        let setup = hex_bytes(
            case.get("setup_hex")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            &format!("keyboard case {case_id} setup"),
        )?;
        io::stdout()
            .write_all(&setup)
            .map_err(|err| err.to_string())?;
        io::stdout().flush().map_err(|err| err.to_string())?;
        let captured = read_keyboard_capture(timeout, idle_timeout);
        let cleanup_result = io::stdout()
            .write_all(&cleanup)
            .and_then(|_| io::stdout().flush())
            .map_err(|err| err.to_string());
        let captured = captured?;
        cleanup_result?;

        let matched = keyboard_capture_matches(case, &captured)?;
        eprintln!(
            "  captured {} bytes: {} {}",
            captured.len(),
            hex::encode(&captured),
            if matched { "ok" } else { "mismatch" }
        );
        captured_cases.push(json!({
            "id": case_id,
            "captured_hex": hex::encode(captured),
            "matched": matched,
        }));
    }

    let report = json!({
        "version": 1,
        "terminal": terminal,
        "timestamp_unix": unix_timestamp()?,
        "spec": manifest.get("spec").cloned().unwrap_or(Value::Null),
        "cases": captured_cases,
    });
    let text = serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?;
    fs::write(&output, format!("{text}\n"))
        .map_err(|err| format!("{}: {err}", output.display()))?;
    println!("{}", output.display());
    if report
        .get("cases")
        .and_then(Value::as_array)
        .is_some_and(|cases| {
            cases
                .iter()
                .all(|case| case.get("matched").and_then(Value::as_bool) == Some(true))
        })
    {
        Ok(())
    } else {
        Err("one or more keyboard captures did not match".to_string())
    }
}

fn command_keyboard_verify_capture(
    root: &Path,
    capture: &Path,
    require_all: bool,
) -> Result<()> {
    let manifest = load_keyboard_manifest(root)?;
    let cases = keyboard_cases_by_id(&manifest)?;
    let capture_path = expand_tilde(capture);
    let report = load_json(&capture_path)?;
    if report.get("version").and_then(Value::as_i64) != Some(1) {
        return Err(format!(
            "unsupported capture version in {}",
            capture_path.display()
        ));
    }

    let mut captured_by_id: HashMap<String, &Value> = HashMap::new();
    let mut captured_order = Vec::new();
    for capture in report
        .get("cases")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
    {
        if let Some(id) = capture.get("id").and_then(Value::as_str) {
            captured_by_id.insert(id.to_string(), capture);
            captured_order.push((id.to_string(), capture));
        }
    }

    if require_all {
        let mut missing = cases
            .keys()
            .filter(|case_id| !captured_by_id.contains_key(*case_id))
            .cloned()
            .collect::<Vec<_>>();
        missing.sort();
        if !missing.is_empty() {
            return Err(format!(
                "capture missing keyboard cases: {}",
                missing.join(", ")
            ));
        }
    }

    let mut failed = false;
    for (case_id, capture) in captured_order {
        let case = cases
            .get(&case_id)
            .ok_or_else(|| format!("capture has unknown keyboard case: {case_id}"))?;
        let captured_hex = capture
            .get("captured_hex")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let data = hex_bytes(captured_hex, &format!("capture {case_id}"))?;
        let matched = keyboard_capture_matches(case, &data)?;
        println!(
            "{} {case_id} {} bytes",
            if matched { "ok" } else { "fail" },
            data.len()
        );
        failed |= !matched;
    }
    if failed {
        Err("one or more keyboard captures did not match".to_string())
    } else {
        Ok(())
    }
}

fn command_record_env(root: &Path, output: Option<PathBuf>, rio_bin: &str) -> Result<()> {
    let output = output
        .map(|path| expand_tilde(&path))
        .unwrap_or_else(|| path_from_components(root, DEFAULT_ENV_OUTPUT));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    let report = json!({
        "repo": root.to_string_lossy(),
        "timestamp_unix": unix_timestamp()?,
        "commands": {
            "git_head": run_capture(root, ["git", "rev-parse", "HEAD"]),
            "git_status": run_capture(root, ["git", "status", "--short", "--branch"]),
            "rio_version": run_capture(root, [rio_bin, "--version"]),
            "rustc": run_capture(root, ["rustc", "--version"]),
            "cargo": run_capture(root, ["cargo", "--version"]),
            "rustup_active_toolchain": run_capture(root, ["rustup", "show", "active-toolchain"]),
            "vulkaninfo_summary": run_capture(root, ["vulkaninfo", "--summary"]),
        },
    });
    let text = serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?;
    fs::write(&output, format!("{text}\n"))
        .map_err(|err| format!("{}: {err}", output.display()))?;
    println!("{}", output.display());
    Ok(())
}

fn command_launch_cpu_screenshot(
    root: &Path,
    output_dir: Option<PathBuf>,
    config_dir: Option<PathBuf>,
    rio_bin: &str,
    sleep_seconds: u64,
    settle_seconds: u64,
) -> Result<()> {
    let output_dir = output_dir
        .map(|path| expand_tilde(&path))
        .unwrap_or_else(|| path_from_components(root, DEFAULT_SCREENSHOT_DIR));
    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("{}: {err}", output_dir.display()))?;
    let config_dir = config_dir
        .map(|path| expand_tilde(&path))
        .unwrap_or_else(|| path_from_components(root, DEFAULT_CPU_CONFIG));
    ensure_cpu_config(&config_dir)?;

    let mut command = Command::new("nix");
    command
        .arg("develop")
        .arg("-c")
        .arg(rio_bin)
        .arg("--app-id")
        .arg("yazelix-terminal-protocol-conformance")
        .arg("--title-placeholder")
        .arg("Yazelix Terminal Protocol Conformance")
        .arg("-e")
        .arg("bash")
        .arg("--noprofile")
        .arg("--norc")
        .arg("-c")
        .arg(format!(
            "printf 'yazelix-terminal protocol conformance\nCPU renderer screenshot probe\nPID $$\n'; sleep {sleep_seconds}"
        ))
        .current_dir(root)
        .env("RIO_CONFIG_HOME", &config_dir);
    let child = command.spawn().map_err(|err| err.to_string())?;
    capture_cosmic_screenshot(output_dir, child, settle_seconds, sleep_seconds)
}

fn command_launch_wgpu_shader_screenshot(
    root: &Path,
    output_dir: Option<PathBuf>,
    config_dir: Option<PathBuf>,
    rio_bin: &str,
    wgpu_backend: &str,
    shader: &[String],
    sleep_seconds: u64,
    settle_seconds: u64,
) -> Result<()> {
    let output_dir = output_dir
        .map(|path| expand_tilde(&path))
        .unwrap_or_else(|| path_from_components(root, DEFAULT_SHADER_SCREENSHOT_DIR));
    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("{}: {err}", output_dir.display()))?;
    let config_dir = config_dir
        .map(|path| expand_tilde(&path))
        .unwrap_or_else(|| path_from_components(root, DEFAULT_SHADER_CONFIG));
    let shader_paths = if shader.is_empty() {
        vec!["conformance/shaders/ghostty_cursor_probe.glsl".to_string()]
    } else {
        shader.to_vec()
    };
    ensure_shader_config(&config_dir, &shader_paths)?;

    let mut command = Command::new("nix");
    command
        .arg("develop")
        .arg("-c")
        .arg(rio_bin)
        .arg("--app-id")
        .arg("yazelix-terminal-shader-probe")
        .arg("--title-placeholder")
        .arg("Yazelix Terminal Shader Probe")
        .arg("-e")
        .arg("bash")
        .arg("--noprofile")
        .arg("--norc")
        .arg("-c")
        .arg(format!(
            "printf 'yazelix-terminal shader probe\nGhostty cursor uniforms via WGPU\nPID $$\n'; sleep {sleep_seconds}"
        ))
        .current_dir(root)
        .env("RIO_CONFIG_HOME", &config_dir)
        .env("WGPU_BACKEND", wgpu_backend);
    let child = command.spawn().map_err(|err| err.to_string())?;
    capture_cosmic_screenshot(output_dir, child, settle_seconds, sleep_seconds)
}

fn ensure_cpu_config(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|err| format!("{}: {err}", path.display()))?;
    fs::write(path.join("config.toml"), "[renderer]\nuse-cpu = true\n")
        .map_err(|err| format!("{}: {err}", path.join("config.toml").display()))
}

fn ensure_shader_config(path: &Path, shader_paths: &[String]) -> Result<()> {
    fs::create_dir_all(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let shader_entries = shader_paths
        .iter()
        .map(|shader| serde_json::to_string(shader).map_err(|err| err.to_string()))
        .collect::<Result<Vec<_>>>()?
        .join(", ");
    fs::write(
        path.join("config.toml"),
        format!("[renderer]\nbackend = \"Webgpu\"\ncustom-shader = [{shader_entries}]\n"),
    )
    .map_err(|err| format!("{}: {err}", path.join("config.toml").display()))
}

fn capture_cosmic_screenshot(
    output_dir: PathBuf,
    mut process: Child,
    settle_seconds: u64,
    sleep_seconds: u64,
) -> Result<()> {
    let result = (|| -> Result<()> {
        let screenshot_tool = find_in_path("cosmic-screenshot")
            .ok_or_else(|| "cosmic-screenshot not found".to_string())?;
        std::thread::sleep(Duration::from_secs(settle_seconds.max(1)));
        if let Some(status) = process.try_wait().map_err(|err| err.to_string())? {
            return Err(format!(
                "launched terminal exited before screenshot: {status}"
            ));
        }

        let shot = Command::new(screenshot_tool)
            .arg("--interactive=false")
            .arg("--modal=false")
            .arg("--notify=false")
            .arg("--save-dir")
            .arg(&output_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|err| err.to_string())?;
        if !shot.status.success() {
            let stderr = trim_output(&shot.stderr);
            return Err(if stderr.is_empty() {
                "screenshot failed".to_string()
            } else {
                stderr
            });
        }

        let status = wait_with_timeout(
            &mut process,
            Duration::from_secs(sleep_seconds.max(1) + 3),
        )?
        .ok_or_else(|| "launched terminal did not exit after screenshot".to_string())?;
        if !status.success() {
            return Err(format!(
                "launched terminal exited after screenshot: {status}"
            ));
        }
        let stdout = trim_output(&shot.stdout);
        if !stdout.is_empty() {
            println!("{stdout}");
        }
        Ok(())
    })();

    if result.is_err() && process.try_wait().ok().flatten().is_none() {
        let _ = process.kill();
        let _ = process.wait();
    }
    result
}

fn run_capture<const N: usize>(root: &Path, argv: [&str; N]) -> Value {
    let argv_json = argv.iter().copied().collect::<Vec<_>>();
    let mut command = Command::new(argv[0]);
    command.args(&argv[1..]);
    match command
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => json!({
            "argv": argv_json,
            "ok": output.status.success(),
            "status": output.status.code().unwrap_or(-1),
            "stdout": trim_output(&output.stdout),
            "stderr": trim_output(&output.stderr),
        }),
        Err(err) if err.kind() == io::ErrorKind::NotFound => json!({
            "argv": argv_json,
            "ok": false,
            "status": "not_found",
            "stdout": "",
            "stderr": "",
        }),
        Err(err) => json!({
            "argv": argv_json,
            "ok": false,
            "status": "error",
            "stdout": "",
            "stderr": err.to_string(),
        }),
    }
}

#[cfg(unix)]
fn read_keyboard_capture(timeout_seconds: f64, idle_seconds: f64) -> Result<Vec<u8>> {
    use std::mem::MaybeUninit;
    use std::os::fd::AsRawFd;

    let stdin = io::stdin();
    let fd = stdin.as_raw_fd();
    let mut original = MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(fd, original.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    let original = unsafe { original.assume_init() };
    let mut raw = original;
    unsafe {
        libc::cfmakeraw(&mut raw);
    }
    if unsafe { libc::tcsetattr(fd, libc::TCSADRAIN, &raw) } != 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    let _guard = TerminalModeGuard { fd, original };

    let mut data = Vec::new();
    let start = Instant::now();
    let mut last_input: Option<Instant> = None;
    let timeout = Duration::from_secs_f64(timeout_seconds.max(0.0));
    let idle_timeout = Duration::from_secs_f64(idle_seconds.max(0.0));

    while start.elapsed() < timeout {
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let poll_result = unsafe { libc::poll(&mut poll_fd, 1, 50) };
        if poll_result < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err.to_string());
        }
        if poll_result > 0 && (poll_fd.revents & libc::POLLIN) != 0 {
            let mut buffer = [0u8; 4096];
            let read_count = unsafe {
                libc::read(fd, buffer.as_mut_ptr().cast::<libc::c_void>(), buffer.len())
            };
            if read_count < 0 {
                let err = io::Error::last_os_error();
                if matches!(
                    err.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) {
                    continue;
                }
                return Err(err.to_string());
            }
            if read_count == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..read_count as usize]);
            last_input = Some(Instant::now());
        } else if !data.is_empty()
            && last_input.is_some_and(|last| last.elapsed() >= idle_timeout)
        {
            break;
        }
    }

    Ok(data)
}

#[cfg(unix)]
struct TerminalModeGuard {
    fd: i32,
    original: libc::termios,
}

#[cfg(unix)]
impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSADRAIN, &self.original);
        }
    }
}

#[cfg(not(unix))]
fn read_keyboard_capture(_timeout_seconds: f64, _idle_seconds: f64) -> Result<Vec<u8>> {
    Err("keyboard-capture currently requires a Unix terminal".to_string())
}

fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> Result<Option<std::process::ExitStatus>> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            return Ok(Some(status));
        }
        if start.elapsed() >= timeout {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn trim_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn validate_keyboard_manifest(root: &Path) -> Result<()> {
    let manifest = load_keyboard_manifest(root)?;
    let cleanup = hex_bytes(
        manifest
            .get("cleanup_hex")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        "keyboard cleanup",
    )?;
    if cleanup.is_empty() {
        return Err("keyboard cleanup is empty".to_string());
    }
    for case in keyboard_cases_by_id(&manifest)?.values() {
        let case_id = str_field(case, "id", "keyboard case")?;
        let setup = hex_bytes(
            case.get("setup_hex")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            &format!("keyboard case {case_id} setup"),
        )?;
        if setup.is_empty() {
            return Err(format!("keyboard case {case_id} setup is empty"));
        }
        expected_keyboard_fragments(case)?;
        validate_fixture_metadata(case, &format!("keyboard case {case_id}"))?;
        for field in ["tier", "keys", "workflow", "reference"] {
            if case
                .get(field)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .is_empty()
            {
                return Err(format!("keyboard case {case_id} missing {field}"));
            }
        }
    }
    Ok(())
}

fn validate_fixture_metadata(fixture: &Value, context: &str) -> Result<()> {
    let kind = fixture.get("kind").and_then(Value::as_str);
    if !kind.is_some_and(|kind| ALLOWED_FIXTURE_KINDS.contains(&kind)) {
        return Err(format!(
            "{context} has unsupported kind {kind:?}; expected one of {ALLOWED_FIXTURE_KINDS:?}"
        ));
    }
    let source = fixture.get("source").and_then(Value::as_str);
    if !source.is_some_and(|source| ALLOWED_FIXTURE_SOURCES.contains(&source)) {
        return Err(format!(
            "{context} has unsupported source {source:?}; expected one of {ALLOWED_FIXTURE_SOURCES:?}"
        ));
    }
    if fixture
        .get("reference")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .is_empty()
    {
        return Err(format!("{context} missing reference"));
    }
    let targets = fixture
        .get("comparison_targets")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{context} missing comparison_targets"))?;
    if targets.is_empty() {
        return Err(format!("{context} missing comparison_targets"));
    }
    for target in targets {
        let target = target
            .as_str()
            .ok_or_else(|| format!("{context} has non-string comparison target"))?;
        if !ALLOWED_COMPARISON_TARGETS.contains(&target) {
            return Err(format!(
                "{context} has unsupported comparison target {target:?}; expected one of {ALLOWED_COMPARISON_TARGETS:?}"
            ));
        }
    }
    Ok(())
}

fn keyboard_cases_by_id(manifest: &Value) -> Result<HashMap<String, Value>> {
    let mut cases = HashMap::new();
    for case in array_field(manifest, "cases", "keyboard manifest")? {
        let case_id = case
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| "keyboard case missing id".to_string())?;
        if cases.insert(case_id.to_string(), case.clone()).is_some() {
            return Err(format!("duplicate keyboard case id: {case_id}"));
        }
    }
    Ok(cases)
}

fn expected_keyboard_fragments(case: &Value) -> Result<Vec<Vec<u8>>> {
    let case_id = str_field(case, "id", "keyboard case")?;
    let expect = object_field(case, "expect", "keyboard case")?;
    match expect.get("mode").and_then(Value::as_str) {
        Some("exact") => Ok(vec![hex_bytes(
            expect
                .get("hex")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            &format!("keyboard case {case_id} expected hex"),
        )?]),
        Some("contains") => {
            let fragments = expect
                .get("fragments")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    format!("keyboard case {case_id} has no expected fragments")
                })?;
            if fragments.is_empty() {
                return Err(format!("keyboard case {case_id} has no expected fragments"));
            }
            fragments
                .iter()
                .map(|fragment| {
                    hex_bytes(
                        fragment.as_str().unwrap_or_default(),
                        &format!("keyboard case {case_id} expected fragment"),
                    )
                })
                .collect()
        }
        Some(mode) => Err(format!(
            "keyboard case {case_id} has unsupported expectation mode: {mode}"
        )),
        None => Err(format!(
            "keyboard case {case_id} has unsupported expectation mode: "
        )),
    }
}

fn keyboard_capture_matches(case: &Value, captured: &[u8]) -> Result<bool> {
    let expect = object_field(case, "expect", "keyboard case")?;
    let fragments = expected_keyboard_fragments(case)?;
    if expect.get("mode").and_then(Value::as_str) == Some("exact") {
        return Ok(captured == fragments[0]);
    }

    let mut cursor = 0;
    for fragment in fragments {
        if let Some(offset) = find_bytes(&captured[cursor..], &fragment) {
            cursor += offset + fragment.len();
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_toml(path: &Path) -> Result<toml::Value> {
    let text = read_text(path)?;
    text.parse::<toml::Value>()
        .map_err(|err| format!("{}: {err}", path.display()))
}

fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))
}

fn ensure_contains(text: &str, pattern: &str, message: &str) -> Result<()> {
    if text.contains(pattern) {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn path_from_components(root: &Path, components: &[&str]) -> PathBuf {
    components
        .iter()
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

fn expand_tilde(path: &Path) -> PathBuf {
    let path_text = path.as_os_str().to_string_lossy();
    if path_text == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
    }
    if let Some(stripped) = path_text.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(stripped);
        }
    }
    path.to_path_buf()
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return program_path.is_file().then(|| program_path.to_path_buf());
    }
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
    })
}

fn unix_timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|err| err.to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha256(bytes);
    hex::encode(digest)
}

fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
        0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
        0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (input.len() as u64) * 8;
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (index, word) in w.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut output = [0u8; 32];
    for (index, word) in h.into_iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

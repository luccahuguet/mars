// With the default subsystem, 'console', windows creates an additional console
// window for the program.
// This is silently ignored on non-windows systems.
// See https://msdn.microsoft.com/en-us/library/4cc7ya5b.aspx for more details.
#![windows_subsystem = "windows"]

mod application;
mod bindings;
mod cli;
mod constants;
mod context;
mod frame_metrics;
mod graphics_namespace;
mod grid_emit;
mod hints;
mod ime;
mod layout;
mod messenger;
mod mouse;
#[cfg(windows)]
mod panic;
mod platform;
mod renderer;
mod router;
mod scheduler;
mod screen;
mod watcher;

use clap::Parser;
use rio_backend::config::config_dir_path;
use rio_backend::event::EventPayload;
use rio_backend::{ansi, crosswords, event, performer, selection};
use std::path::PathBuf;
use std::str::FromStr;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    self, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

#[cfg(windows)]
use windows_sys::Win32::System::Console::{
    AttachConsole, FreeConsole, ATTACH_PARENT_PROCESS,
};

const LOG_LEVEL_ENV: &str = "RIO_LOG_LEVEL";
const RIO_TERM_PROGRAM: &str = "rio";
const YAZELIX_TERMINAL_HOST_ENV: &str = "YAZELIX_TERMINAL_HOST";
const YAZELIX_TERMINAL_HOST: &str = "yazelix-terminal";
const INHERITED_TERMINAL_IDENTITY_ENV: &[&str] = &[
    "ALACRITTY_LOG",
    "ALACRITTY_SOCKET",
    "ALACRITTY_WINDOW_ID",
    "GHOSTTY_BIN_DIR",
    "GHOSTTY_RESOURCES_DIR",
    "GHOSTTY_SHELL_FEATURES",
    "ITERM_PROFILE",
    "ITERM_SESSION_ID",
    "KITTY_LISTEN_ON",
    "KITTY_PID",
    "KITTY_PUBLIC_KEY",
    "KITTY_WINDOW_ID",
    "KONSOLE_DBUS_SESSION",
    "KONSOLE_DBUS_SERVICE",
    "KONSOLE_VERSION",
    "TABBY_CONFIG_DIRECTORY",
    "TERMCAP",
    "TERMINFO",
    "VSCODE_INJECTION",
    "WARP_HONOR_PS1",
    "WEZTERM_EXECUTABLE",
    "WEZTERM_PANE",
    "WEZTERM_UNIX_SOCKET",
    "WT_PROFILE_ID",
    "WT_SESSION",
    "WT_Session",
];

#[derive(Debug, PartialEq, Eq)]
struct ChildTerminalIdentity {
    term_program: &'static str,
    yazelix_terminal_host: Option<&'static str>,
}

fn child_terminal_identity(yazelix_mode: bool) -> ChildTerminalIdentity {
    ChildTerminalIdentity {
        term_program: RIO_TERM_PROGRAM,
        yazelix_terminal_host: yazelix_mode.then_some(YAZELIX_TERMINAL_HOST),
    }
}

fn scrub_inherited_terminal_identity(yazelix_mode: bool) {
    if !yazelix_mode {
        return;
    }

    for name in INHERITED_TERMINAL_IDENTITY_ENV {
        std::env::remove_var(name);
    }
}

pub fn setup_environment_variables(
    config: &rio_backend::config::Config,
    yazelix_mode: bool,
) {
    scrub_inherited_terminal_identity(yazelix_mode);

    #[cfg(unix)]
    {
        let terminfo = match (
            teletypewriter::terminfo_exists("xterm-rio"),
            teletypewriter::terminfo_exists("rio"),
        ) {
            // In case `xterm-rio` exists we prioritize it
            (true, _) => "xterm-rio",
            // If is only `rio` installed (which was the default for versions under 0.2.27)
            (false, true) => "rio",
            // If none, then fallback to `xterm-256color`
            (false, false) => "xterm-256color",
        };

        let span = tracing::span!(tracing::Level::INFO, "setup_environment_variables");
        let _guard = span.enter();
        tracing::info!("terminfo: {terminfo}");
        std::env::set_var("TERM", terminfo);
    }

    // TERM/TERM_PROGRAM are capability signals consumed by tools like Yazi.
    // Yazelix host identity lives in its own marker so child tools still
    // detect the Rio protocol surface.
    let identity = child_terminal_identity(yazelix_mode);
    std::env::set_var("TERM_PROGRAM", identity.term_program);
    std::env::set_var("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
    if let Some(host) = identity.yazelix_terminal_host {
        std::env::set_var(YAZELIX_TERMINAL_HOST_ENV, host);
    } else {
        std::env::remove_var(YAZELIX_TERMINAL_HOST_ENV);
    }

    std::env::set_var("COLORTERM", "truecolor");
    // Keep startup-notification tokens available for RouteWindow::from_target,
    // which applies them to the first window and clears them before spawning
    // the child process.
    #[cfg(target_os = "macos")]
    {
        platform::macos::set_locale_environment();
        std::env::set_current_dir(dirs::home_dir().unwrap()).unwrap();
    }

    // Set env vars from config.
    for env_config in config.env_vars.iter() {
        let env_vec: Vec<&str> = env_config.split('=').collect();

        if env_vec.len() == 2 {
            std::env::set_var(env_vec[0], env_vec[1]);
        }
    }
}

fn apply_yazelix_mode(
    config: &mut rio_backend::config::Config,
    terminal_options: &cli::TerminalOptions,
    app_id: &mut Option<String>,
) -> Result<(), String> {
    if !terminal_options.yazelix {
        return Ok(());
    }

    if terminal_options.command().is_none() {
        return Err(
            "--yazelix requires --command/-e with the Yazelix runtime command".into(),
        );
    }

    config.use_fork = false;
    config.navigation.use_split = false;
    config.navigation.open_config_with_split = false;
    config.navigation.hide_if_single = true;
    app_id.get_or_insert_with(|| "yazelix-terminal".to_string());

    Ok(())
}

fn setup_logs_by_filter_level(
    log_level: &str,
    log_file: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut filter_level = LevelFilter::from_str(log_level).unwrap_or(LevelFilter::OFF);

    if let Ok(data) = std::env::var(LOG_LEVEL_ENV) {
        if !data.is_empty() {
            filter_level = LevelFilter::from_str(&data).unwrap_or(filter_level);
        }
    }

    let env_filter = EnvFilter::builder().with_default_directive(filter_level.into());
    let stdout_subscriber = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_filter(env_filter.parse("")?);
    let subscriber = tracing_subscriber::registry().with(stdout_subscriber);

    let mut log_file_path = PathBuf::new();
    if log_file {
        let log_dir_path = config_dir_path().join("log");
        log_file_path = log_dir_path.join("rio.log");
        std::fs::create_dir_all(&log_dir_path)?;
        let log_file = std::fs::File::create(&log_file_path)?;
        let file_subscriber = tracing_subscriber::fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_writer(log_file)
            .with_target(false)
            .with_ansi(false)
            .with_filter(env_filter.parse("")?);
        subscriber.with(file_subscriber).init();
    } else {
        subscriber.init();
    }

    let span = tracing::span!(tracing::Level::INFO, "logger");
    let _guard = span.enter();
    tracing::info!("logging level: {log_level}");
    if log_file {
        tracing::info!("logging to a file: {}", log_file_path.display());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    panic::attach_handler();

    // When linked with the windows subsystem windows won't automatically attach
    // to the console of the parent process, so we do it explicitly. This fails
    // silently if the parent has no console.
    #[cfg(windows)]
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    // Load command line options.
    let args = cli::Cli::parse();

    let write_config_path = args.window_options.terminal_options.write_config.clone();
    if let Some(config_path) = write_config_path {
        let _ = setup_logs_by_filter_level("TRACE", false);
        rio_backend::config::create_config_file(config_path);
        return Ok(());
    }

    let (mut config, config_error) = match rio_backend::config::Config::try_load() {
        Ok(config) => (config, None),
        Err(err) => (rio_backend::config::Config::default(), Some(err)),
    };

    // Read platform property and overwrite values per OS
    //
    // [shell]
    // # default (in this case will be used on MacOS/Linux)
    // program = "/bin/fish"
    // args = ["--login"]
    //
    // [platform]
    // # Microsoft Windows overwrite
    // windows.shell.program = "pwsh"
    // windows.shell.args = ["-l"]
    config.overwrite_based_on_platform();

    {
        let terminal_options = &args.window_options.terminal_options;
        let log_to_file = terminal_options.enable_log_file;
        if let Err(e) = setup_logs_by_filter_level(
            &config.developer.log_level,
            log_to_file || config.developer.enable_log_file,
        ) {
            eprintln!("unable to configure the logger: {e:?}");
        }

        if let Some(command) = terminal_options.command() {
            config.shell = command;
            config.use_fork = false;
        }

        if let Some(working_dir_cli) = terminal_options.working_dir.as_deref() {
            // Use dunce::canonicalize on Windows to avoid UNC paths (\\?\)
            // which break many tools like Neovim and Bun
            #[cfg(target_os = "windows")]
            let canonicalize_fn = dunce::canonicalize;
            #[cfg(not(target_os = "windows"))]
            let canonicalize_fn = std::fs::canonicalize;

            config.working_dir = match canonicalize_fn(&working_dir_cli).and_then(
                |path| {
                    if path.is_dir() {
                        path.into_os_string().into_string().map_err(|_| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Invalid UTF-8 in path",
                            )
                        })
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::NotADirectory,
                            "Path is not a directory",
                        ))
                    }
                },
            ) {
                Ok(canonical_path) => Some(canonical_path),
                Err(e) => {
                    tracing::warn!("Failed to set working directory '{}': {}. Using default instead.", working_dir_cli, e);
                    None
                }
            };
        }

        config.title.placeholder = terminal_options.title_placeholder.clone();
    }

    #[cfg(target_os = "linux")]
    {
        // If running inside a flatpak sandbox.
        // Rio will never use use_fork configuration as true
        if std::path::PathBuf::from("/.flatpak-info").exists() {
            config.use_fork = false;
        }
    }

    let terminal_options = &args.window_options.terminal_options;
    let mut app_id = terminal_options.app_id.clone();
    apply_yazelix_mode(&mut config, terminal_options, &mut app_id)
        .map_err(std::io::Error::other)?;

    setup_environment_variables(&config, terminal_options.yazelix);
    frame_metrics::init_from_env()?;

    let window_event_loop =
        rio_window::event_loop::EventLoop::<EventPayload>::with_user_event().build()?;

    let mut application = crate::application::Application::new(
        config,
        config_error,
        &window_event_loop,
        app_id,
    );
    let _ = application.run(window_event_loop);

    #[cfg(windows)]
    unsafe {
        FreeConsole();
    }

    Ok(())
}

#[cfg(test)]
// Test lane: default
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    // Defends: Yazelix mode is explicit and never silently falls back to a host shell.
    fn yazelix_mode_requires_command() {
        let mut config = rio_backend::config::Config::default();
        let mut app_id = None;
        let options = cli::TerminalOptions {
            yazelix: true,
            ..Default::default()
        };

        assert!(apply_yazelix_mode(&mut config, &options, &mut app_id).is_err());
    }

    #[test]
    // Defends: Yazelix mode disables Rio-owned workspace splits while preserving the single child command surface.
    fn yazelix_mode_disables_native_workspace_ownership() {
        let mut config = rio_backend::config::Config::default();
        let mut app_id = None;
        let options = cli::TerminalOptions {
            command: vec!["yzx".to_string(), "launch".to_string()],
            yazelix: true,
            ..Default::default()
        };

        apply_yazelix_mode(&mut config, &options, &mut app_id).unwrap();

        assert!(!config.navigation.use_split);
        assert!(!config.navigation.open_config_with_split);
        assert!(config.navigation.hide_if_single);
        assert!(!config.use_fork);
        assert_eq!(app_id.as_deref(), Some("yazelix-terminal"));
    }

    #[test]
    // Defends: Child applications detect Rio protocols while Yazelix host mode remains visible separately.
    fn yazelix_mode_keeps_rio_child_terminal_identity() {
        assert_eq!(
            child_terminal_identity(true),
            ChildTerminalIdentity {
                term_program: "rio",
                yazelix_terminal_host: Some("yazelix-terminal"),
            }
        );
    }

    #[test]
    // Invariant: Desktop startup tokens survive environment setup so the first Wayland/X11 window can be focused.
    fn setup_environment_preserves_startup_activation_tokens() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous_desktop_startup_id = std::env::var("DESKTOP_STARTUP_ID").ok();
        let previous_xdg_activation_token = std::env::var("XDG_ACTIVATION_TOKEN").ok();

        std::env::set_var("DESKTOP_STARTUP_ID", "x11-token");
        std::env::set_var("XDG_ACTIVATION_TOKEN", "wayland-token");

        setup_environment_variables(&rio_backend::config::Config::default(), false);

        assert_eq!(
            std::env::var("DESKTOP_STARTUP_ID").as_deref(),
            Ok("x11-token")
        );
        assert_eq!(
            std::env::var("XDG_ACTIVATION_TOKEN").as_deref(),
            Ok("wayland-token")
        );

        restore_env("DESKTOP_STARTUP_ID", previous_desktop_startup_id);
        restore_env("XDG_ACTIVATION_TOKEN", previous_xdg_activation_token);
    }

    fn restore_env(name: &str, value: Option<String>) {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}

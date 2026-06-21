#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::env;
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::process;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    static METRICS: OnceLock<Option<Metrics>> = OnceLock::new();

    struct Metrics {
        file: Mutex<File>,
        started: Instant,
    }

    fn env_enabled() -> bool {
        if env_truthy("MARS_PERF_METRICS") {
            return true;
        }

        env::var("MARS_PERF_TRACE")
            .ok()
            .map(|value| {
                value
                    .split(|ch| matches!(ch, ',' | ':' | ';' | ' ' | '\t'))
                    .any(|token| {
                        matches!(
                            token.trim().to_ascii_lowercase().as_str(),
                            "all" | "metrics" | "perf" | "pty" | "render"
                        )
                    })
            })
            .unwrap_or(false)
    }

    fn env_truthy(name: &str) -> bool {
        env::var(name)
            .ok()
            .map(|value| {
                let value = value.trim();
                !value.is_empty()
                    && !matches!(
                        value.to_ascii_lowercase().as_str(),
                        "0" | "false" | "off"
                    )
            })
            .unwrap_or(false)
    }

    fn metrics_path() -> PathBuf {
        if let Some(path) = env::var_os("MARS_PERF_METRICS_FILE") {
            return PathBuf::from(path);
        }

        let base = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
            })
            .unwrap_or_else(env::temp_dir);
        base.join("mars").join("perf_metrics.jsonl")
    }

    fn init_metrics() -> Option<Metrics> {
        if !env_enabled() {
            return None;
        }

        let path = metrics_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;
        let metrics = Metrics {
            file: Mutex::new(file),
            started: Instant::now(),
        };
        write_raw_event(
            &metrics,
            "metrics_start",
            &format!(",\"path\":\"{}\"", escape_json(&path.display().to_string())),
        );
        Some(metrics)
    }

    fn metrics() -> Option<&'static Metrics> {
        METRICS.get_or_init(init_metrics).as_ref()
    }

    pub fn start_timer() -> Option<Instant> {
        metrics().map(|_| Instant::now())
    }

    pub fn elapsed_us(start: Option<Instant>) -> Option<u64> {
        start.map(|started| micros_u64(started.elapsed().as_micros()))
    }

    pub fn record_pty_read(route_id: usize, bytes: usize, duration_us: Option<u64>) {
        write_event(
            "pty_read_batch",
            &format!(
                ",\"route_id\":{},\"bytes\":{},\"duration_us\":{}",
                route_id,
                bytes,
                json_opt_u64(duration_us)
            ),
        );
    }

    pub fn record_parser_batch(route_id: usize, bytes: usize, duration_us: Option<u64>) {
        write_event(
            "parser_state_batch",
            &format!(
                ",\"route_id\":{},\"bytes\":{},\"duration_us\":{}",
                route_id,
                bytes,
                json_opt_u64(duration_us)
            ),
        );
    }

    pub fn record_render_snapshot(
        route_id: usize,
        duration_us: Option<u64>,
        any_panel_dirty: bool,
    ) {
        write_event(
            "render_snapshot",
            &format!(
                ",\"route_id\":{},\"duration_us\":{},\"any_panel_dirty\":{}",
                route_id,
                json_opt_u64(duration_us),
                any_panel_dirty
            ),
        );
    }

    pub fn record_grid_emit(
        route_id: usize,
        damage: &str,
        rows: usize,
        cells: usize,
        duration_us: Option<u64>,
    ) {
        write_event(
            "grid_emit",
            &format!(
                ",\"route_id\":{},\"damage\":\"{}\",\"rows\":{},\"cells\":{},\"duration_us\":{}",
                route_id,
                escape_json(damage),
                rows,
                cells,
                json_opt_u64(duration_us)
            ),
        );
    }

    pub fn record_frame_render(
        route_id: usize,
        presented: bool,
        any_panel_dirty: bool,
        has_animation: bool,
        visual_bell: bool,
        total_us: Option<u64>,
        snapshot_us: Option<u64>,
        grid_emit_us: Option<u64>,
        present_us: Option<u64>,
    ) {
        write_event(
            "frame_render",
            &format!(
                concat!(
                    ",\"route_id\":{},\"presented\":{},\"any_panel_dirty\":{},",
                    "\"has_animation\":{},\"visual_bell\":{},\"total_us\":{},",
                    "\"snapshot_us\":{},\"grid_emit_us\":{},\"present_us\":{}"
                ),
                route_id,
                presented,
                any_panel_dirty,
                has_animation,
                visual_bell,
                json_opt_u64(total_us),
                json_opt_u64(snapshot_us),
                json_opt_u64(grid_emit_us),
                json_opt_u64(present_us)
            ),
        );
    }

    fn write_event(event: &str, fields: &str) {
        if let Some(metrics) = metrics() {
            write_raw_event(metrics, event, fields);
        }
    }

    fn write_raw_event(metrics: &Metrics, event: &str, fields: &str) {
        let elapsed_us = micros_u64(metrics.started.elapsed().as_micros());
        let unix_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| micros_u64(duration.as_micros()))
            .unwrap_or(0);

        let line = format!(
            "{{\"schema_version\":1,\"pid\":{},\"ts_unix_us\":{},\"elapsed_us\":{},\"event\":\"{}\"{}}}\n",
            process::id(),
            unix_us,
            elapsed_us,
            escape_json(event),
            fields
        );

        if let Ok(mut file) = metrics.file.lock() {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn micros_u64(value: u128) -> u64 {
        u64::try_from(value).unwrap_or(u64::MAX)
    }

    fn json_opt_u64(value: Option<u64>) -> String {
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string())
    }

    fn escape_json(value: &str) -> String {
        let mut escaped = String::with_capacity(value.len());
        for ch in value.chars() {
            match ch {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                ch if ch.is_control() => {
                    escaped.push_str(&format!("\\u{:04x}", ch as u32));
                }
                ch => escaped.push(ch),
            }
        }
        escaped
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::time::Instant;

    pub fn start_timer() -> Option<Instant> {
        None
    }

    pub fn elapsed_us(_: Option<Instant>) -> Option<u64> {
        None
    }

    pub fn record_pty_read(_: usize, _: usize, _: Option<u64>) {}

    pub fn record_parser_batch(_: usize, _: usize, _: Option<u64>) {}

    pub fn record_render_snapshot(_: usize, _: Option<u64>, _: bool) {}

    pub fn record_grid_emit(_: usize, _: &str, _: usize, _: usize, _: Option<u64>) {}

    #[allow(clippy::too_many_arguments)]
    pub fn record_frame_render(
        _: usize,
        _: bool,
        _: bool,
        _: bool,
        _: bool,
        _: Option<u64>,
        _: Option<u64>,
        _: Option<u64>,
        _: Option<u64>,
    ) {
    }
}

pub use imp::*;

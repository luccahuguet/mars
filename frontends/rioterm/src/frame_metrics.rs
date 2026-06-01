use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const FRAME_LOG_ENV: &str = "YAZELIX_TERMINAL_FRAME_LOG";

static FRAME_LOGGER: OnceLock<Option<Mutex<FrameLogger>>> = OnceLock::new();

struct FrameLogger {
    file: BufWriter<File>,
    start: Instant,
    frame_index: u64,
    last_frame_end: Option<Duration>,
}

pub(crate) struct RedrawMetrics<'a> {
    pub(crate) window_id: &'a str,
    pub(crate) route: &'a str,
    pub(crate) presented: bool,
    pub(crate) dirty_after: bool,
    pub(crate) game_mode: bool,
    pub(crate) vblank_interval: Duration,
    pub(crate) render_phases: RenderPhaseMetrics,
    pub(crate) render_start: Instant,
    pub(crate) render_end: Instant,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RenderPhaseMetrics {
    pub(crate) renderer_run_duration: Duration,
    pub(crate) terminal_lock_wait_duration: Duration,
    pub(crate) terminal_snapshot_duration: Duration,
    pub(crate) snapshot_visible_duration: Duration,
    pub(crate) panel_collect_duration: Duration,
    pub(crate) grid_emit_duration: Duration,
    pub(crate) sugarloaf_render_duration: Duration,
    pub(crate) panel_count: u64,
    pub(crate) visible_row_count: u64,
    pub(crate) row_rebuild_count: u64,
    pub(crate) full_row_rebuild_count: u64,
    pub(crate) dirty_row_rebuild_count: u64,
    pub(crate) terminal_lock_busy_count: u64,
}

pub(crate) fn init_from_env() -> io::Result<()> {
    let logger = match std::env::var_os(FRAME_LOG_ENV) {
        Some(path) if !path.is_empty() => {
            Some(Mutex::new(FrameLogger::new(path.into())?))
        }
        _ => None,
    };

    let _ = FRAME_LOGGER.set(logger);
    Ok(())
}

pub(crate) fn record_redraw(metrics: RedrawMetrics<'_>) {
    let Some(logger) = FRAME_LOGGER.get().and_then(Option::as_ref) else {
        return;
    };

    logger
        .lock()
        .expect("lock YAZELIX_TERMINAL_FRAME_LOG")
        .record_redraw(metrics)
        .expect("write YAZELIX_TERMINAL_FRAME_LOG frame event");
}

impl FrameLogger {
    fn new(path: PathBuf) -> io::Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;
        let start = Instant::now();
        writeln!(
            file,
            "{{\"event\":\"benchmark_start\",\"pid\":{},\"elapsed_ns\":0}}",
            std::process::id()
        )?;
        file.flush()?;
        Ok(Self {
            file: BufWriter::new(file),
            start,
            frame_index: 0,
            last_frame_end: None,
        })
    }

    fn record_redraw(&mut self, metrics: RedrawMetrics<'_>) -> io::Result<()> {
        let start_elapsed = metrics.render_start.duration_since(self.start);
        let end_elapsed = metrics.render_end.duration_since(self.start);
        let render_duration = metrics.render_end.duration_since(metrics.render_start);
        let delta = self
            .last_frame_end
            .map(|last_end| end_elapsed.saturating_sub(last_end))
            .unwrap_or_default();
        self.last_frame_end = Some(end_elapsed);

        write!(
            self.file,
            "{{\"event\":\"redraw\",\"frame_index\":{},",
            self.frame_index
        )?;
        self.frame_index += 1;
        write_json_string_field(&mut self.file, "window_id", metrics.window_id)?;
        write_json_string_field(&mut self.file, "route", metrics.route)?;
        writeln!(
            self.file,
            "\"presented\":{},\"dirty_after\":{},\"game_mode\":{},\
             \"start_elapsed_ns\":{},\"end_elapsed_ns\":{},\
             \"render_duration_ns\":{},\"delta_ns\":{},\"vblank_interval_ns\":{},\
             \"renderer_run_duration_ns\":{},\
             \"terminal_lock_wait_duration_ns\":{},\
             \"terminal_snapshot_duration_ns\":{},\
             \"snapshot_visible_duration_ns\":{},\
             \"panel_collect_duration_ns\":{},\
             \"grid_emit_duration_ns\":{},\
             \"sugarloaf_render_duration_ns\":{},\
             \"panel_count\":{},\"visible_row_count\":{},\
             \"row_rebuild_count\":{},\"full_row_rebuild_count\":{},\
             \"dirty_row_rebuild_count\":{},\
             \"terminal_lock_busy_count\":{}}}",
            metrics.presented,
            metrics.dirty_after,
            metrics.game_mode,
            start_elapsed.as_nanos(),
            end_elapsed.as_nanos(),
            render_duration.as_nanos(),
            delta.as_nanos(),
            metrics.vblank_interval.as_nanos(),
            metrics.render_phases.renderer_run_duration.as_nanos(),
            metrics.render_phases.terminal_lock_wait_duration.as_nanos(),
            metrics.render_phases.terminal_snapshot_duration.as_nanos(),
            metrics.render_phases.snapshot_visible_duration.as_nanos(),
            metrics.render_phases.panel_collect_duration.as_nanos(),
            metrics.render_phases.grid_emit_duration.as_nanos(),
            metrics.render_phases.sugarloaf_render_duration.as_nanos(),
            metrics.render_phases.panel_count,
            metrics.render_phases.visible_row_count,
            metrics.render_phases.row_rebuild_count,
            metrics.render_phases.full_row_rebuild_count,
            metrics.render_phases.dirty_row_rebuild_count,
            metrics.render_phases.terminal_lock_busy_count
        )?;
        self.file.flush()
    }
}

fn write_json_string_field(
    file: &mut impl Write,
    name: &str,
    value: &str,
) -> io::Result<()> {
    write!(file, "\"{name}\":\"")?;
    for ch in value.chars() {
        match ch {
            '"' => file.write_all(br#"\""#)?,
            '\\' => file.write_all(br#"\\"#)?,
            '\n' => file.write_all(br#"\n"#)?,
            '\r' => file.write_all(br#"\r"#)?,
            '\t' => file.write_all(br#"\t"#)?,
            ch if ch.is_control() => write!(file, "\\u{:04x}", ch as u32)?,
            ch => write!(file, "{ch}")?,
        }
    }
    file.write_all(b"\",")
}

use std::error::Error;
use std::fmt;
use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::ansi::CursorShape;
use crate::crosswords::grid::row::Row;
use crate::crosswords::grid::Dimensions as _;
use crate::crosswords::pos::{Column, Line};
use crate::crosswords::square::{Extras, Square};
use crate::crosswords::{Crosswords, CrosswordsSize};
use crate::event::{TerminalDamage, VoidListener, WindowId};
use crate::performer::handler::Processor;
use crate::performer::parser::{Params, Parser, Perform};
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, Debug)]
pub struct ParserBenchmarkConfig {
    pub chunk_size: usize,
    pub iterations: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ParserBenchmarkCounts {
    pub print_chars: u64,
    pub print_bytes: u64,
    pub execute: u64,
    pub hook: u64,
    pub put: u64,
    pub unhook: u64,
    pub osc_dispatch: u64,
    pub csi_dispatch: u64,
    pub esc_dispatch: u64,
    pub string_controls: u64,
}

impl ParserBenchmarkCounts {
    pub fn total_actions(self) -> u64 {
        self.print_chars
            + self.execute
            + self.hook
            + self.put
            + self.unhook
            + self.osc_dispatch
            + self.csi_dispatch
            + self.esc_dispatch
            + self.string_controls
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParserBenchmarkResult {
    pub corpus_bytes: usize,
    pub chunk_size: usize,
    pub iterations: usize,
    pub total_bytes: u128,
    pub elapsed: Duration,
    pub counts: ParserBenchmarkCounts,
}

impl ParserBenchmarkResult {
    pub fn bytes_per_second(self) -> f64 {
        let elapsed = self.elapsed.as_secs_f64();
        if elapsed == 0.0 {
            0.0
        } else {
            self.total_bytes as f64 / elapsed
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalStreamBenchmarkConfig {
    pub rows: usize,
    pub columns: usize,
    pub scrollback_history_limit: usize,
    pub chunk_size: usize,
    pub iterations: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalStreamBenchmarkResult {
    pub corpus_bytes: usize,
    pub rows: usize,
    pub columns: usize,
    pub scrollback_history_limit: usize,
    pub chunk_size: usize,
    pub iterations: usize,
    pub total_bytes: u128,
    pub elapsed: Duration,
    pub final_cursor_line: i32,
    pub final_cursor_column: usize,
    pub final_history_size: usize,
    pub final_display_offset: usize,
    pub final_total_lines: usize,
    pub final_sync_buffer_bytes: usize,
}

impl TerminalStreamBenchmarkResult {
    pub fn bytes_per_second(self) -> f64 {
        let elapsed = self.elapsed.as_secs_f64();
        if elapsed == 0.0 {
            0.0
        } else {
            self.total_bytes as f64 / elapsed
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderSnapshotBenchmarkMode {
    Noop,
    Full,
    Incremental,
}

impl RenderSnapshotBenchmarkMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::Full => "full",
            Self::Incremental => "incremental",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderSnapshotBenchmarkConfig {
    pub rows: usize,
    pub columns: usize,
    pub scrollback_history_limit: usize,
    pub chunk_size: usize,
    pub iterations: usize,
    pub dirty_rows_per_iteration: usize,
    pub mode: RenderSnapshotBenchmarkMode,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderSnapshotBenchmarkResult {
    pub corpus_bytes: usize,
    pub rows: usize,
    pub columns: usize,
    pub scrollback_history_limit: usize,
    pub chunk_size: usize,
    pub iterations: usize,
    pub dirty_rows_per_iteration: usize,
    pub mode: RenderSnapshotBenchmarkMode,
    pub elapsed: Duration,
    pub total_snapshots: usize,
    pub final_damage: TerminalDamage,
    pub final_cursor_line: i32,
    pub final_cursor_column: usize,
    pub final_history_size: usize,
    pub final_display_offset: usize,
    pub final_visible_rows: usize,
    pub final_dirty_rows: usize,
    pub final_style_count: usize,
    pub final_extras_count: usize,
    pub final_sync_buffer_bytes: usize,
}

impl RenderSnapshotBenchmarkResult {
    pub fn snapshots_per_second(self) -> f64 {
        let elapsed = self.elapsed.as_secs_f64();
        if elapsed == 0.0 {
            0.0
        } else {
            self.total_snapshots as f64 / elapsed
        }
    }
}

#[derive(Debug)]
pub enum ParserBenchmarkError {
    EmptyCorpus,
    ZeroChunkSize,
    ZeroIterations,
}

impl fmt::Display for ParserBenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCorpus => f.write_str("parser benchmark corpus is empty"),
            Self::ZeroChunkSize => {
                f.write_str("parser benchmark chunk size must be greater than zero")
            }
            Self::ZeroIterations => {
                f.write_str("parser benchmark iterations must be greater than zero")
            }
        }
    }
}

impl Error for ParserBenchmarkError {}

#[derive(Debug)]
pub enum TerminalStreamBenchmarkError {
    EmptyCorpus,
    ZeroRows,
    ZeroColumns,
    ZeroChunkSize,
    ZeroIterations,
}

impl fmt::Display for TerminalStreamBenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCorpus => f.write_str("terminal stream benchmark corpus is empty"),
            Self::ZeroRows => {
                f.write_str("terminal stream benchmark rows must be greater than zero")
            }
            Self::ZeroColumns => {
                f.write_str("terminal stream benchmark columns must be greater than zero")
            }
            Self::ZeroChunkSize => f.write_str(
                "terminal stream benchmark chunk size must be greater than zero",
            ),
            Self::ZeroIterations => f.write_str(
                "terminal stream benchmark iterations must be greater than zero",
            ),
        }
    }
}

impl Error for TerminalStreamBenchmarkError {}

#[derive(Debug)]
pub enum RenderSnapshotBenchmarkError {
    EmptyCorpus,
    ZeroRows,
    ZeroColumns,
    ZeroChunkSize,
    ZeroIterations,
    ZeroDirtyRows,
}

impl fmt::Display for RenderSnapshotBenchmarkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCorpus => f.write_str("render snapshot benchmark corpus is empty"),
            Self::ZeroRows => {
                f.write_str("render snapshot benchmark rows must be greater than zero")
            }
            Self::ZeroColumns => {
                f.write_str("render snapshot benchmark columns must be greater than zero")
            }
            Self::ZeroChunkSize => f.write_str(
                "render snapshot benchmark chunk size must be greater than zero",
            ),
            Self::ZeroIterations => f.write_str(
                "render snapshot benchmark iterations must be greater than zero",
            ),
            Self::ZeroDirtyRows => f.write_str(
                "render snapshot benchmark dirty rows must be greater than zero",
            ),
        }
    }
}

impl Error for RenderSnapshotBenchmarkError {}

pub fn run_parser_benchmark(
    corpus: &[u8],
    config: ParserBenchmarkConfig,
) -> Result<ParserBenchmarkResult, ParserBenchmarkError> {
    if corpus.is_empty() {
        return Err(ParserBenchmarkError::EmptyCorpus);
    }
    if config.chunk_size == 0 {
        return Err(ParserBenchmarkError::ZeroChunkSize);
    }
    if config.iterations == 0 {
        return Err(ParserBenchmarkError::ZeroIterations);
    }

    let mut counts = ParserBenchmarkCounts::default();
    let start = Instant::now();

    for _ in 0..config.iterations {
        let mut parser = Parser::default();
        let mut performer = CountingPerformer::default();
        for chunk in corpus.chunks(config.chunk_size) {
            parser.advance(&mut performer, black_box(chunk));
        }
        counts.add(black_box(performer.counts));
    }

    Ok(ParserBenchmarkResult {
        corpus_bytes: corpus.len(),
        chunk_size: config.chunk_size,
        iterations: config.iterations,
        total_bytes: corpus.len() as u128 * config.iterations as u128,
        elapsed: start.elapsed(),
        counts,
    })
}

pub fn run_terminal_stream_benchmark(
    corpus: &[u8],
    config: TerminalStreamBenchmarkConfig,
) -> Result<TerminalStreamBenchmarkResult, TerminalStreamBenchmarkError> {
    if corpus.is_empty() {
        return Err(TerminalStreamBenchmarkError::EmptyCorpus);
    }
    if config.rows == 0 {
        return Err(TerminalStreamBenchmarkError::ZeroRows);
    }
    if config.columns == 0 {
        return Err(TerminalStreamBenchmarkError::ZeroColumns);
    }
    if config.chunk_size == 0 {
        return Err(TerminalStreamBenchmarkError::ZeroChunkSize);
    }
    if config.iterations == 0 {
        return Err(TerminalStreamBenchmarkError::ZeroIterations);
    }

    let mut final_cursor_line = 0;
    let mut final_cursor_column = 0;
    let mut final_history_size = 0;
    let mut final_display_offset = 0;
    let mut final_total_lines = 0;
    let mut final_sync_buffer_bytes = 0;
    let mut elapsed = Duration::ZERO;

    for _ in 0..config.iterations {
        let mut processor = Processor::default();
        let mut terminal = Crosswords::new(
            CrosswordsSize::new(config.columns, config.rows),
            CursorShape::Block,
            VoidListener {},
            WindowId::from(0),
            0,
            config.scrollback_history_limit,
        );

        let start = Instant::now();
        for chunk in corpus.chunks(config.chunk_size) {
            processor.advance(&mut terminal, black_box(chunk));
        }
        elapsed += start.elapsed();

        final_cursor_line = terminal.grid.cursor.pos.row.0;
        final_cursor_column = terminal.grid.cursor.pos.col.0;
        final_history_size = terminal.grid.history_size();
        final_display_offset = terminal.grid.display_offset();
        final_total_lines = terminal.grid.total_lines();
        final_sync_buffer_bytes = processor.sync_bytes_count();
        black_box(&terminal);
        black_box(&processor);
    }

    Ok(TerminalStreamBenchmarkResult {
        corpus_bytes: corpus.len(),
        rows: config.rows,
        columns: config.columns,
        scrollback_history_limit: config.scrollback_history_limit,
        chunk_size: config.chunk_size,
        iterations: config.iterations,
        total_bytes: corpus.len() as u128 * config.iterations as u128,
        elapsed,
        final_cursor_line,
        final_cursor_column,
        final_history_size,
        final_display_offset,
        final_total_lines,
        final_sync_buffer_bytes,
    })
}

pub fn run_render_snapshot_benchmark(
    corpus: &[u8],
    config: RenderSnapshotBenchmarkConfig,
) -> Result<RenderSnapshotBenchmarkResult, RenderSnapshotBenchmarkError> {
    if corpus.is_empty() {
        return Err(RenderSnapshotBenchmarkError::EmptyCorpus);
    }
    if config.rows == 0 {
        return Err(RenderSnapshotBenchmarkError::ZeroRows);
    }
    if config.columns == 0 {
        return Err(RenderSnapshotBenchmarkError::ZeroColumns);
    }
    if config.chunk_size == 0 {
        return Err(RenderSnapshotBenchmarkError::ZeroChunkSize);
    }
    if config.iterations == 0 {
        return Err(RenderSnapshotBenchmarkError::ZeroIterations);
    }
    if config.mode == RenderSnapshotBenchmarkMode::Incremental
        && config.dirty_rows_per_iteration == 0
    {
        return Err(RenderSnapshotBenchmarkError::ZeroDirtyRows);
    }

    let mut processor = Processor::default();
    let mut terminal = Crosswords::new(
        CrosswordsSize::new(config.columns, config.rows),
        CursorShape::Block,
        VoidListener {},
        WindowId::from(0),
        0,
        config.scrollback_history_limit,
    );

    for chunk in corpus.chunks(config.chunk_size) {
        processor.advance(&mut terminal, black_box(chunk));
    }

    let mut visible_rows: Vec<Row<Square>> = Vec::new();
    let mut style_table = Vec::new();
    let mut extras: FxHashMap<u16, Extras> = FxHashMap::default();

    // Warm the reusable snapshot buffers once so the timed loop measures the
    // steady-state renderer extraction path rather than first-frame allocation.
    terminal.reset_damage();
    terminal.snapshot_visible(
        &TerminalDamage::Full,
        terminal.columns(),
        &mut visible_rows,
        &mut style_table,
        &mut extras,
    );
    clear_snapshot_dirty(&mut visible_rows);
    let _ = terminal.damage();
    terminal.reset_damage();

    let mut elapsed = Duration::ZERO;
    let mut final_damage = TerminalDamage::Noop;
    let mut final_cursor_line = 0;
    let mut final_cursor_column = 0;
    let mut final_history_size = 0;
    let mut final_display_offset = 0;
    let mut final_visible_rows = 0;
    let mut final_dirty_rows = 0;
    let mut final_style_count = 0;
    let mut final_extras_count = 0;
    let mut final_sync_buffer_bytes = processor.sync_bytes_count();

    for iteration in 0..config.iterations {
        match config.mode {
            RenderSnapshotBenchmarkMode::Full => terminal.mark_fully_damaged(),
            RenderSnapshotBenchmarkMode::Incremental => {
                prepare_incremental_snapshot_damage(
                    &mut terminal,
                    iteration,
                    config.dirty_rows_per_iteration,
                );
            }
            RenderSnapshotBenchmarkMode::Noop => {}
        }

        let start = Instant::now();
        let damage = terminal.peek_damage_event().unwrap_or(TerminalDamage::Noop);
        terminal.reset_damage();
        terminal.snapshot_visible(
            &damage,
            terminal.columns(),
            &mut visible_rows,
            &mut style_table,
            &mut extras,
        );

        let cursor = terminal.cursor();
        final_damage = damage;
        final_cursor_line = cursor.pos.row.0;
        final_cursor_column = cursor.pos.col.0;
        final_history_size = terminal.history_size();
        final_display_offset = terminal.display_offset();
        final_visible_rows = visible_rows.len();
        final_dirty_rows = count_snapshot_dirty(&visible_rows);
        final_style_count = style_table.len();
        final_extras_count = extras.len();
        final_sync_buffer_bytes = processor.sync_bytes_count();
        black_box((
            terminal.colors,
            final_cursor_line,
            final_cursor_column,
            final_history_size,
            final_display_offset,
            final_visible_rows,
            final_dirty_rows,
            final_style_count,
            final_extras_count,
            terminal.blinking_cursor,
            terminal.graphics.kitty_graphics_dirty,
        ));
        elapsed += start.elapsed();

        clear_snapshot_dirty(&mut visible_rows);
    }

    Ok(RenderSnapshotBenchmarkResult {
        corpus_bytes: corpus.len(),
        rows: config.rows,
        columns: config.columns,
        scrollback_history_limit: config.scrollback_history_limit,
        chunk_size: config.chunk_size,
        iterations: config.iterations,
        dirty_rows_per_iteration: config.dirty_rows_per_iteration,
        mode: config.mode,
        elapsed,
        total_snapshots: config.iterations,
        final_damage,
        final_cursor_line,
        final_cursor_column,
        final_history_size,
        final_display_offset,
        final_visible_rows,
        final_dirty_rows,
        final_style_count,
        final_extras_count,
        final_sync_buffer_bytes,
    })
}

fn prepare_incremental_snapshot_damage<U>(
    terminal: &mut Crosswords<U>,
    iteration: usize,
    dirty_rows_per_iteration: usize,
) where
    U: crate::event::EventListener,
{
    let rows = terminal.screen_lines().max(1);
    let columns = terminal.columns().max(1);
    let dirty_rows = dirty_rows_per_iteration.min(rows);

    for offset in 0..dirty_rows {
        let row = (iteration + offset) % rows;
        let col = Column((iteration + offset) % columns);
        let ch = char::from_u32(b'a' as u32 + ((iteration + offset) % 26) as u32)
            .unwrap_or('a');
        terminal.grid[Line(row as i32)][col].set_c(ch);
        terminal.damage_line(row);
    }
}

fn count_snapshot_dirty(rows: &[Row<Square>]) -> usize {
    rows.iter().filter(|row| row.dirty).count()
}

fn clear_snapshot_dirty(rows: &mut [Row<Square>]) {
    for row in rows {
        row.dirty = false;
    }
}

impl ParserBenchmarkCounts {
    fn add(&mut self, other: Self) {
        self.print_chars += other.print_chars;
        self.print_bytes += other.print_bytes;
        self.execute += other.execute;
        self.hook += other.hook;
        self.put += other.put;
        self.unhook += other.unhook;
        self.osc_dispatch += other.osc_dispatch;
        self.csi_dispatch += other.csi_dispatch;
        self.esc_dispatch += other.esc_dispatch;
        self.string_controls += other.string_controls;
    }
}

#[derive(Default)]
struct CountingPerformer {
    counts: ParserBenchmarkCounts,
}

impl Perform for CountingPerformer {
    fn print(&mut self, c: char) {
        self.counts.print_chars += 1;
        self.counts.print_bytes += c.len_utf8() as u64;
    }

    fn print_str(&mut self, s: &str) {
        self.counts.print_chars += s.chars().count() as u64;
        self.counts.print_bytes += s.len() as u64;
    }

    fn print_codepoints(&mut self, codepoints: &[u32]) {
        self.counts.print_chars += codepoints.len() as u64;
        self.counts.print_bytes += codepoints
            .iter()
            .map(|&cp| char::from_u32(cp).unwrap_or('\u{FFFD}').len_utf8() as u64)
            .sum::<u64>();
    }

    fn execute(&mut self, _byte: u8) {
        self.counts.execute += 1;
    }

    fn hook(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
        self.counts.hook += 1;
    }

    fn put(&mut self, _byte: u8) {
        self.counts.put += 1;
    }

    fn unhook(&mut self) {
        self.counts.unhook += 1;
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {
        self.counts.osc_dispatch += 1;
    }

    fn csi_dispatch(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
        self.counts.csi_dispatch += 1;
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {
        self.counts.esc_dispatch += 1;
    }

    fn sos_start(&mut self) {
        self.counts.string_controls += 1;
    }

    fn sos_put(&mut self, _byte: u8) {
        self.counts.string_controls += 1;
    }

    fn sos_end(&mut self) {
        self.counts.string_controls += 1;
    }

    fn pm_start(&mut self) {
        self.counts.string_controls += 1;
    }

    fn pm_put(&mut self, _byte: u8) {
        self.counts.string_controls += 1;
    }

    fn pm_end(&mut self) {
        self.counts.string_controls += 1;
    }

    fn apc_start(&mut self) {
        self.counts.string_controls += 1;
    }

    fn apc_put(&mut self, _byte: u8) {
        self.counts.string_controls += 1;
    }

    fn apc_end(&mut self) {
        self.counts.string_controls += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_benchmark_counts_control_sequences() {
        let corpus = b"hello\x1b[31mred\x1b[0m\x1b]0;title\x07\n";
        let result = run_parser_benchmark(
            corpus,
            ParserBenchmarkConfig {
                chunk_size: 4,
                iterations: 2,
            },
        )
        .unwrap();

        assert_eq!(result.corpus_bytes, corpus.len());
        assert_eq!(result.total_bytes, corpus.len() as u128 * 2);
        assert!(result.counts.print_chars >= 16);
        assert!(result.counts.csi_dispatch >= 4);
        assert!(result.counts.osc_dispatch >= 2);
    }

    #[test]
    fn parser_benchmark_counts_utf8_print_bytes() {
        let text = "acao ação 東京 λ ✓";
        let result = run_parser_benchmark(
            text.as_bytes(),
            ParserBenchmarkConfig {
                chunk_size: 3,
                iterations: 3,
            },
        )
        .unwrap();

        assert_eq!(result.counts.print_chars, text.chars().count() as u64 * 3);
        assert_eq!(result.counts.print_bytes, text.len() as u64 * 3);
    }

    #[test]
    fn terminal_stream_benchmark_updates_grid_state() {
        let corpus = b"\x1b[2J\x1b[Hhello\r\n\x1b[31mred\x1b[0m\r\n";
        let result = run_terminal_stream_benchmark(
            corpus,
            TerminalStreamBenchmarkConfig {
                rows: 4,
                columns: 20,
                scrollback_history_limit: 100,
                chunk_size: 3,
                iterations: 2,
            },
        )
        .unwrap();

        assert_eq!(result.corpus_bytes, corpus.len());
        assert_eq!(result.rows, 4);
        assert_eq!(result.columns, 20);
        assert_eq!(result.total_bytes, corpus.len() as u128 * 2);
        assert!(result.final_cursor_line >= 1);
        assert_eq!(result.final_sync_buffer_bytes, 0);
    }

    #[test]
    fn terminal_stream_benchmark_exercises_scrollback() {
        let corpus = b"one\r\ntwo\r\nthree\r\nfour\r\n";
        let result = run_terminal_stream_benchmark(
            corpus,
            TerminalStreamBenchmarkConfig {
                rows: 2,
                columns: 20,
                scrollback_history_limit: 100,
                chunk_size: 5,
                iterations: 1,
            },
        )
        .unwrap();

        assert!(result.final_history_size > 0);
        assert!(result.final_total_lines > result.rows);
    }

    #[test]
    fn render_snapshot_benchmark_measures_full_snapshot() {
        let corpus = b"\x1b[48;2;20;40;60mhello\x1b[0m\r\n\x1b]8;;https://example.test\x07link\x1b]8;;\x07\r\n";
        let result = run_render_snapshot_benchmark(
            corpus,
            RenderSnapshotBenchmarkConfig {
                rows: 4,
                columns: 20,
                scrollback_history_limit: 100,
                chunk_size: 4,
                iterations: 2,
                dirty_rows_per_iteration: 1,
                mode: RenderSnapshotBenchmarkMode::Full,
            },
        )
        .unwrap();

        assert_eq!(result.mode, RenderSnapshotBenchmarkMode::Full);
        assert_eq!(result.final_damage, TerminalDamage::Full);
        assert_eq!(result.final_visible_rows, 4);
        assert_eq!(result.final_dirty_rows, 4);
        assert!(result.final_style_count > 0);
        assert!(result.final_extras_count > 0);
    }

    #[test]
    fn render_snapshot_benchmark_measures_incremental_damage() {
        let corpus = b"one\r\ntwo\r\nthree\r\nfour\r\n";
        let result = run_render_snapshot_benchmark(
            corpus,
            RenderSnapshotBenchmarkConfig {
                rows: 4,
                columns: 20,
                scrollback_history_limit: 100,
                chunk_size: 4,
                iterations: 3,
                dirty_rows_per_iteration: 2,
                mode: RenderSnapshotBenchmarkMode::Incremental,
            },
        )
        .unwrap();

        assert_eq!(result.mode, RenderSnapshotBenchmarkMode::Incremental);
        assert_eq!(result.final_damage, TerminalDamage::Partial);
        assert_eq!(result.final_visible_rows, 4);
        assert_eq!(result.final_dirty_rows, 2);
        assert!(result.final_history_size > 0);
    }

    #[test]
    fn render_snapshot_benchmark_measures_noop_gate() {
        let result = run_render_snapshot_benchmark(
            b"stable\r\n",
            RenderSnapshotBenchmarkConfig {
                rows: 3,
                columns: 10,
                scrollback_history_limit: 10,
                chunk_size: 8,
                iterations: 2,
                dirty_rows_per_iteration: 1,
                mode: RenderSnapshotBenchmarkMode::Noop,
            },
        )
        .unwrap();

        assert_eq!(result.mode, RenderSnapshotBenchmarkMode::Noop);
        assert_eq!(result.final_damage, TerminalDamage::Noop);
        assert_eq!(result.final_dirty_rows, 0);
        assert_eq!(result.final_visible_rows, 3);
    }
}

use std::error::Error;
use std::fmt;
use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::performer::parser::{Params, Parser, Perform};

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
}

use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use rio_backend::benchmarks::{
    run_terminal_stream_benchmark, TerminalStreamBenchmarkConfig,
};

#[derive(Debug)]
struct Args {
    corpus: PathBuf,
    rows: usize,
    columns: usize,
    scrollback_history_limit: usize,
    chunk_size: usize,
    iterations: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("mars-terminal-stream-bench: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = parse_args(env::args_os().skip(1))?;
    let corpus = fs::read(&args.corpus)?;
    let metadata_path = corpus_metadata_path(&args.corpus);
    let metadata_source = fs::read_to_string(&metadata_path).ok();
    let metadata_bytes = metadata_source
        .as_ref()
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let metadata_json = metadata_source.as_deref().map(compact_metadata_json);
    let result = run_terminal_stream_benchmark(
        &corpus,
        TerminalStreamBenchmarkConfig {
            rows: args.rows,
            columns: args.columns,
            scrollback_history_limit: args.scrollback_history_limit,
            chunk_size: args.chunk_size,
            iterations: args.iterations,
        },
    )?;

    if cfg!(debug_assertions) {
        eprintln!("warning: debug builds are not representative; rerun with --release");
    }

    println!("benchmark=mars_terminal_stream_state_update");
    println!(
        "profile={}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!("corpus_path={}", args.corpus.display());
    println!("corpus_bytes={}", result.corpus_bytes);
    println!("corpus_metadata_path={}", metadata_path.display());
    println!("corpus_metadata_present={}", metadata_json.is_some());
    println!("corpus_metadata_bytes={metadata_bytes}");
    println!(
        "corpus_metadata_json={}",
        metadata_json.as_deref().unwrap_or("")
    );
    println!("rows={}", result.rows);
    println!("columns={}", result.columns);
    println!(
        "scrollback_history_limit={}",
        result.scrollback_history_limit
    );
    println!("chunk_size={}", result.chunk_size);
    println!("iterations={}", result.iterations);
    println!("total_bytes={}", result.total_bytes);
    println!("elapsed_ns={}", result.elapsed.as_nanos());
    println!("bytes_per_second={:.2}", result.bytes_per_second());
    println!("final_cursor_line={}", result.final_cursor_line);
    println!("final_cursor_column={}", result.final_cursor_column);
    println!("final_history_size={}", result.final_history_size);
    println!("final_display_offset={}", result.final_display_offset);
    println!("final_total_lines={}", result.final_total_lines);
    println!("final_sync_buffer_bytes={}", result.final_sync_buffer_bytes);

    Ok(())
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Args, Box<dyn Error>> {
    let mut corpus = None;
    let mut rows = None;
    let mut columns = None;
    let mut scrollback_history_limit = 10_000;
    let mut chunk_size = 4096;
    let mut iterations = 1;
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        let arg = arg
            .into_string()
            .map_err(|_| "arguments must be valid UTF-8")?;
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--corpus" => corpus = Some(next_path(&mut args, "--corpus")?),
            "--rows" => rows = Some(next_usize(&mut args, "--rows")?),
            "--columns" | "--cols" => columns = Some(next_usize(&mut args, "--columns")?),
            "--scrollback-history-limit" | "--scrollback" => {
                scrollback_history_limit =
                    next_usize(&mut args, "--scrollback-history-limit")?
            }
            "--chunk-size" => chunk_size = next_usize(&mut args, "--chunk-size")?,
            "--iterations" => iterations = next_usize(&mut args, "--iterations")?,
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }

    let corpus = corpus.ok_or("--corpus PATH is required")?;
    let rows = rows.ok_or("--rows N is required")?;
    let columns = columns.ok_or("--columns N is required")?;
    if rows == 0 {
        return Err("--rows must be greater than zero".into());
    }
    if columns == 0 {
        return Err("--columns must be greater than zero".into());
    }
    if chunk_size == 0 {
        return Err("--chunk-size must be greater than zero".into());
    }
    if iterations == 0 {
        return Err("--iterations must be greater than zero".into());
    }

    Ok(Args {
        corpus,
        rows,
        columns,
        scrollback_history_limit,
        chunk_size,
        iterations,
    })
}

fn next_path(
    args: &mut impl Iterator<Item = OsString>,
    name: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(next_string(args, name)?))
}

fn next_usize(
    args: &mut impl Iterator<Item = OsString>,
    name: &str,
) -> Result<usize, Box<dyn Error>> {
    let raw = next_string(args, name)?;
    raw.parse()
        .map_err(|_| format!("{name} expects a positive integer").into())
}

fn next_string(
    args: &mut impl Iterator<Item = OsString>,
    name: &str,
) -> Result<String, Box<dyn Error>> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value"))?
        .into_string()
        .map_err(|_| format!("{name} value must be valid UTF-8").into())
}

fn corpus_metadata_path(corpus: &Path) -> PathBuf {
    let mut metadata_name = corpus
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("corpus"));
    metadata_name.push(".json");
    corpus.with_file_name(metadata_name)
}

fn compact_metadata_json(metadata: &str) -> String {
    metadata
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn print_help() {
    println!(
        "\
mars-terminal-stream-bench

Replay a raw corpus through Rio/Mars parser plus terminal grid/state updates.
No GUI, PTY, Zellij, renderer, or GPU work is started. Run in release mode for
meaningful numbers.

Required:
  --corpus PATH       Raw corpus file to replay.
  --rows N           Terminal rows to allocate.
  --columns N        Terminal columns to allocate.

Optional:
  --scrollback N      Scrollback history limit. Default: 10000.
  --chunk-size N      Input chunk size in bytes. Default: 4096.
  --iterations N      Replay the corpus this many times. Default: 1.
"
    );
}

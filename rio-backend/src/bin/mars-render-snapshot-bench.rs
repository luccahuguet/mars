use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use rio_backend::benchmarks::{
    run_render_snapshot_benchmark, RenderSnapshotBenchmarkConfig,
    RenderSnapshotBenchmarkMode,
};

#[derive(Debug)]
struct Args {
    corpus: PathBuf,
    rows: usize,
    columns: usize,
    scrollback_history_limit: usize,
    chunk_size: usize,
    iterations: usize,
    dirty_rows_per_iteration: usize,
    modes: Vec<RenderSnapshotBenchmarkMode>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("mars-render-snapshot-bench: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = parse_args(env::args_os().skip(1))?;
    let corpus = fs::read(&args.corpus)?;
    let metadata_path = corpus_metadata_path(&args.corpus);
    let metadata_bytes = fs::metadata(&metadata_path)
        .map(|metadata| metadata.len())
        .ok();
    let metadata_json = fs::read_to_string(&metadata_path)
        .ok()
        .map(|metadata| compact_metadata_json(&metadata));

    if cfg!(debug_assertions) {
        eprintln!("warning: debug builds are not representative; rerun with --release");
    }

    for (index, mode) in args.modes.iter().copied().enumerate() {
        if index > 0 {
            println!();
        }

        let result = run_render_snapshot_benchmark(
            &corpus,
            RenderSnapshotBenchmarkConfig {
                rows: args.rows,
                columns: args.columns,
                scrollback_history_limit: args.scrollback_history_limit,
                chunk_size: args.chunk_size,
                iterations: args.iterations,
                dirty_rows_per_iteration: args.dirty_rows_per_iteration,
                mode,
            },
        )?;

        println!("benchmark=mars_render_snapshot_damage");
        println!(
            "profile={}",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        println!("mode={}", result.mode.as_str());
        println!("corpus_path={}", args.corpus.display());
        println!("corpus_bytes={}", result.corpus_bytes);
        println!("corpus_metadata_path={}", metadata_path.display());
        println!("corpus_metadata_present={}", metadata_bytes.is_some());
        println!("corpus_metadata_bytes={}", metadata_bytes.unwrap_or(0));
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
        println!(
            "dirty_rows_per_iteration={}",
            result.dirty_rows_per_iteration
        );
        println!("total_snapshots={}", result.total_snapshots);
        println!("elapsed_ns={}", result.elapsed.as_nanos());
        println!("snapshots_per_second={:.2}", result.snapshots_per_second());
        println!("final_damage={:?}", result.final_damage);
        println!("final_cursor_line={}", result.final_cursor_line);
        println!("final_cursor_column={}", result.final_cursor_column);
        println!("final_history_size={}", result.final_history_size);
        println!("final_display_offset={}", result.final_display_offset);
        println!("final_visible_rows={}", result.final_visible_rows);
        println!("final_dirty_rows={}", result.final_dirty_rows);
        println!("final_style_count={}", result.final_style_count);
        println!("final_extras_count={}", result.final_extras_count);
        println!("final_sync_buffer_bytes={}", result.final_sync_buffer_bytes);
    }

    Ok(())
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Args, Box<dyn Error>> {
    let mut corpus = None;
    let mut rows = None;
    let mut columns = None;
    let mut scrollback_history_limit = 10_000;
    let mut chunk_size = 4096;
    let mut iterations = 1;
    let mut dirty_rows_per_iteration = 1;
    let mut mode = String::from("all");
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
            "--dirty-rows" | "--dirty-rows-per-iteration" => {
                dirty_rows_per_iteration =
                    next_usize(&mut args, "--dirty-rows-per-iteration")?
            }
            "--mode" => mode = next_string(&mut args, "--mode")?,
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
    let modes = parse_modes(&mode)?;
    if dirty_rows_per_iteration == 0
        && modes.contains(&RenderSnapshotBenchmarkMode::Incremental)
    {
        return Err(
            "--dirty-rows-per-iteration must be greater than zero for incremental mode"
                .into(),
        );
    }

    Ok(Args {
        corpus,
        rows,
        columns,
        scrollback_history_limit,
        chunk_size,
        iterations,
        dirty_rows_per_iteration,
        modes,
    })
}

fn parse_modes(raw: &str) -> Result<Vec<RenderSnapshotBenchmarkMode>, Box<dyn Error>> {
    let modes = match raw {
        "all" => vec![
            RenderSnapshotBenchmarkMode::Noop,
            RenderSnapshotBenchmarkMode::Full,
            RenderSnapshotBenchmarkMode::Incremental,
        ],
        "noop" => vec![RenderSnapshotBenchmarkMode::Noop],
        "full" => vec![RenderSnapshotBenchmarkMode::Full],
        "incremental" | "partial" => vec![RenderSnapshotBenchmarkMode::Incremental],
        unknown => return Err(format!("unknown --mode: {unknown}").into()),
    };
    Ok(modes)
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
mars-render-snapshot-bench

Replay a raw corpus into Rio/Mars terminal state once, then benchmark the
renderer-facing visible-row snapshot and damage extraction path. No GUI, PTY,
Zellij, GPU, or Sugarloaf window is started. Run in release mode for meaningful
numbers. The noop mode measures damage extraction and the no-row-copy snapshot
path after warmup damage has been settled.

Required:
  --corpus PATH       Raw corpus file to initialize terminal state.
  --rows N           Terminal rows to allocate.
  --columns N        Terminal columns to allocate.

Optional:
  --mode MODE         all, noop, full, or incremental. Default: all.
  --scrollback N      Scrollback history limit. Default: 10000.
  --chunk-size N      Input chunk size in bytes. Default: 4096.
  --iterations N      Snapshot the terminal this many times. Default: 1.
  --dirty-rows N      Rows dirtied before each incremental snapshot. Default: 1.
"
    );
}

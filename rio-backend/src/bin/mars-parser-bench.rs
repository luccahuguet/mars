use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use rio_backend::benchmarks::{run_parser_benchmark, ParserBenchmarkConfig};

#[derive(Debug)]
struct Args {
    corpus: PathBuf,
    rows: usize,
    columns: usize,
    chunk_size: usize,
    iterations: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("mars-parser-bench: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = parse_args(env::args_os().skip(1))?;
    let corpus = fs::read(&args.corpus)?;
    let result = run_parser_benchmark(
        &corpus,
        ParserBenchmarkConfig {
            chunk_size: args.chunk_size,
            iterations: args.iterations,
        },
    )?;

    if cfg!(debug_assertions) {
        eprintln!("warning: debug builds are not representative; rerun with --release");
    }

    println!("benchmark=mars_parser_throughput");
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
    println!("rows={}", args.rows);
    println!("columns={}", args.columns);
    println!("chunk_size={}", result.chunk_size);
    println!("iterations={}", result.iterations);
    println!("total_bytes={}", result.total_bytes);
    println!("elapsed_ns={}", result.elapsed.as_nanos());
    println!("bytes_per_second={:.2}", result.bytes_per_second());
    println!("total_actions={}", result.counts.total_actions());
    println!("print_chars={}", result.counts.print_chars);
    println!("print_bytes={}", result.counts.print_bytes);
    println!("execute_callbacks={}", result.counts.execute);
    println!("csi_dispatch_callbacks={}", result.counts.csi_dispatch);
    println!("osc_dispatch_callbacks={}", result.counts.osc_dispatch);
    println!("esc_dispatch_callbacks={}", result.counts.esc_dispatch);
    println!("dcs_put_callbacks={}", result.counts.put);
    println!("string_control_callbacks={}", result.counts.string_controls);

    Ok(())
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Args, Box<dyn Error>> {
    let mut corpus = None;
    let mut rows = None;
    let mut columns = None;
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

fn print_help() {
    println!(
        "\
mars-parser-bench

Parser-only Rio/Mars escape throughput benchmark. No GUI, PTY, renderer, or
terminal state is started. Run in release mode for meaningful numbers.

Required:
  --corpus PATH       Raw corpus file to parse.
  --rows N           Terminal rows recorded with the result.
  --columns N        Terminal columns recorded with the result.

Optional:
  --chunk-size N     Input chunk size in bytes. Default: 4096.
  --iterations N     Parse the corpus this many times. Default: 1.
"
    );
}

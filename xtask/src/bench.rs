use std::{
    collections::HashMap,
    ffi::OsStr,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use clap::Args;

#[derive(Args)]
pub struct BenchArgs {
    /// Benchmark names to run (default: all *.c files)
    names: Vec<String>,

    /// Timed iterations per variant
    #[arg(short = 'n', long, default_value_t = 10)]
    runs: u32,

    /// Warmup iterations before timing
    #[arg(short, long, default_value_t = 1)]
    warmup: u32,

    /// CPU core to pin to via taskset
    #[arg(short, long, default_value_t = 2)]
    core: u32,

    /// Run perf stat after benchmarks
    #[arg(long)]
    perf: bool,
}

struct PhaseResult {
    name: String,
    iterations: u64,
    total_ns: u64,
}

struct PhaseStats {
    total_ns: f64,
    ns_per_op: f64,
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";

fn log(color: bool, tag: &str, message: &str) {
    if color {
        eprintln!("{DIM}[{tag}]{RESET} {message}");
    } else {
        eprintln!("[{tag}] {message}");
    }
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be one directory below the project root")
        .to_path_buf()
}

fn discover_benchmarks(bench_dir: &Path, names: &[String]) -> Vec<PathBuf> {
    if !names.is_empty() {
        return names
            .iter()
            .map(|name| {
                let source = bench_dir.join(format!("{name}.c"));
                if !source.exists() {
                    eprintln!("benchmark not found: {}", source.display());
                    std::process::exit(1);
                }
                source
            })
            .collect();
    }

    let mut sources: Vec<PathBuf> = std::fs::read_dir(bench_dir)
        .expect("failed to read benchmarks directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "c").then_some(path)
        })
        .collect();
    sources.sort();

    if sources.is_empty() {
        eprintln!("no benchmarks found in {}", bench_dir.display());
        std::process::exit(1);
    }

    sources
}

fn build_miros(root: &Path, color: bool) {
    log(color, "build", "miros (release)");
    let status = Command::new("just")
        .arg("build_release")
        .current_dir(root)
        .stdout(Stdio::null())
        .status()
        .expect("failed to run `just build_release`");
    if !status.success() {
        eprintln!("just build_release failed");
        std::process::exit(1);
    }
}

fn compile_benchmark(
    source: &Path,
    variant: &str,
    bin_dir: &Path,
    miros: &Path,
    bench_dir: &Path,
    color: bool,
) -> PathBuf {
    let name = source
        .file_stem()
        .and_then(OsStr::to_str)
        .expect("benchmark source must have a valid UTF-8 filename");
    let output = bin_dir.join(format!("{name}_{variant}"));

    log(color, "compile", &format!("{name} ({variant})"));

    let mut cmd = Command::new("gcc");
    cmd.args([
        "-O2",
        "-march=native",
        "-fno-stack-protector",
        "-fno-builtin-printf",
    ])
    .arg(format!("-I{}", bench_dir.display()));

    if variant == "miros" {
        cmd.arg(format!("-Wl,--dynamic-linker={}", miros.display()));
    }

    cmd.arg("-o").arg(&output).arg(source);

    let status = cmd.status().expect("failed to run gcc");
    if !status.success() {
        eprintln!("compilation failed: {name} ({variant})");
        std::process::exit(1);
    }

    output
}

fn wrapped_command(command: impl AsRef<OsStr>, wrapper: &[String]) -> Command {
    if wrapper.is_empty() {
        Command::new(command)
    } else {
        let mut cmd = Command::new(&wrapper[0]);
        cmd.args(&wrapper[1..]).arg(command);
        cmd
    }
}

/// Run a benchmark binary N times. Returns `None` if any run crashes.
fn execute(binary: &Path, wrapper: &[String], runs: u32) -> Option<Vec<Vec<PhaseResult>>> {
    let mut all_runs = Vec::with_capacity(runs as usize);

    for _ in 0..runs {
        let output = wrapped_command(binary, wrapper)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .expect("failed to execute benchmark");

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        all_runs.push(parse_csv_output(&stdout));
    }

    Some(all_runs)
}

fn parse_csv_output(stdout: &str) -> Vec<PhaseResult> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ',');
            let name = parts.next()?.to_string();
            let iterations = parts.next()?.parse().ok()?;
            let total_ns = parts.next()?.parse().ok()?;
            Some(PhaseResult {
                name,
                iterations,
                total_ns,
            })
        })
        .collect()
}

fn analyze(runs: &[Vec<PhaseResult>]) -> Vec<(String, PhaseStats)> {
    // Accumulate per-phase totals, preserving the order phases first appear.
    let mut phases: Vec<(String, u64, Vec<u64>)> = Vec::new();

    for run in runs {
        for result in run {
            match phases.iter_mut().find(|(name, _, _)| *name == result.name) {
                Some(phase) => phase.2.push(result.total_ns),
                None => phases.push((
                    result.name.clone(),
                    result.iterations,
                    vec![result.total_ns],
                )),
            }
        }
    }

    phases
        .into_iter()
        .map(|(name, iterations, mut totals)| {
            let total = trimmed_median(&mut totals);
            let ns_per_op = if iterations > 0 {
                total / iterations as f64
            } else {
                0.0
            };
            (
                name,
                PhaseStats {
                    total_ns: total,
                    ns_per_op,
                },
            )
        })
        .collect()
}

/// Median of the interior values after dropping the single lowest and highest.
fn trimmed_median(values: &mut [u64]) -> f64 {
    values.sort_unstable();
    if values.len() <= 2 {
        return values.iter().sum::<u64>() as f64 / values.len() as f64;
    }
    let trimmed = &values[1..values.len() - 1];
    let mid = trimmed.len() / 2;
    if trimmed.len() % 2 == 0 {
        (trimmed[mid - 1] as f64 + trimmed[mid] as f64) / 2.0
    } else {
        trimmed[mid] as f64
    }
}

fn format_time(ns: f64) -> String {
    if ns < 1_000_000.0 {
        format!("{:.0} µs", ns / 1_000.0)
    } else if ns < 1_000_000_000.0 {
        format!("{:.1} ms", ns / 1_000_000.0)
    } else {
        format!("{:.2} s", ns / 1_000_000_000.0)
    }
}

fn ratio_style(ratio: f64) -> &'static str {
    if ratio <= 0.95 {
        GREEN
    } else if ratio >= 1.05 {
        RED
    } else {
        YELLOW
    }
}

fn print_table(
    name: &str,
    glibc: &[(String, PhaseStats)],
    miros: &[(String, PhaseStats)],
    color: bool,
) {
    let miros_map: HashMap<&str, &PhaseStats> = miros
        .iter()
        .map(|(name, stats)| (name.as_str(), stats))
        .collect();

    let bold = if color { BOLD } else { "" };
    let dim = if color { DIM } else { "" };
    let reset = if color { RESET } else { "" };

    println!("\n{bold}=== {name} ==={reset}");
    println!(
        "{:<24} {:>12} {:>12} {:>14} {:>14} {:>9}",
        "phase", "glibc", "miros", "glibc ns/op", "miros ns/op", "ratio",
    );
    println!(
        "{dim}{} {} {} {} {} {}{reset}",
        "─".repeat(24),
        "─".repeat(12),
        "─".repeat(12),
        "─".repeat(14),
        "─".repeat(14),
        "─".repeat(9),
    );

    for (phase, glibc_stats) in glibc {
        let Some(miros_stats) = miros_map.get(phase.as_str()) else {
            continue;
        };

        let ratio = if glibc_stats.ns_per_op > 0.0 {
            miros_stats.ns_per_op / glibc_stats.ns_per_op
        } else {
            f64::INFINITY
        };

        let ratio_color = if color { ratio_style(ratio) } else { "" };

        println!(
            "{:<24} {:>12} {:>12} {:>14.2} {:>14.2} {ratio_color}{bold}{:>8.3}x{reset}",
            phase,
            format_time(glibc_stats.total_ns),
            format_time(miros_stats.total_ns),
            glibc_stats.ns_per_op,
            miros_stats.ns_per_op,
            ratio,
        );
    }
}

fn run_perf(
    name: &str,
    glibc_bin: &Path,
    miros_bin: &Path,
    wrapper: &[String],
    runs: u32,
    color: bool,
) {
    if !command_exists("perf") {
        log(color, "perf", "not found — skipping");
        return;
    }

    let dim = if color { DIM } else { "" };
    let reset = if color { RESET } else { "" };

    for (variant, binary) in [("glibc", glibc_bin), ("miros", miros_bin)] {
        println!("\n{dim}--- {name} ({variant}) ---{reset}");

        let status = wrapped_command("perf", wrapper)
            .args(["stat", "-r", &runs.to_string()])
            .args(["-e", "cycles,instructions,cache-misses,dTLB-load-misses"])
            .arg(binary)
            .stdout(Stdio::null())
            .status();

        if let Err(error) = status {
            log(color, "perf", &format!("failed: {error}"));
        }
    }
}

fn command_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn run(args: BenchArgs) {
    let root = project_root();
    let bench_dir = root.join("benchmarks");
    let bin_dir = bench_dir.join("bin");
    let miros = root.join("target/x86_64-unknown-linux-gnu/release/miros");

    let log_color = std::io::stderr().is_terminal();
    let table_color = std::io::stdout().is_terminal();

    std::fs::create_dir_all(&bin_dir).unwrap_or_else(|error| {
        eprintln!("failed to create {}: {error}", bin_dir.display());
        std::process::exit(1);
    });

    build_miros(&root, log_color);
    if !miros.exists() {
        eprintln!("miros binary not found at {}", miros.display());
        std::process::exit(1);
    }

    let sources = discover_benchmarks(&bench_dir, &args.names);

    let wrapper: Vec<String> = if command_exists("taskset") {
        log(log_color, "env", &format!("pinned to core {}", args.core));
        vec!["taskset".into(), "-c".into(), args.core.to_string()]
    } else {
        log(
            log_color,
            "env",
            "taskset unavailable — jitter may be higher",
        );
        Vec::new()
    };

    for source in &sources {
        let name = source
            .file_stem()
            .and_then(OsStr::to_str)
            .expect("benchmark source must have a valid UTF-8 filename");

        let glibc_bin = compile_benchmark(source, "glibc", &bin_dir, &miros, &bench_dir, log_color);
        let miros_bin = compile_benchmark(source, "miros", &bin_dir, &miros, &bench_dir, log_color);

        // Warmup: discard results, skip benchmark entirely if miros crashes.
        if args.warmup > 0 {
            log(log_color, "warmup", &format!("{name} x {}", args.warmup));
            let _ = execute(&glibc_bin, &wrapper, args.warmup);
            if execute(&miros_bin, &wrapper, args.warmup).is_none() {
                log(log_color, "error", "miros crashed during warmup — skipping");
                continue;
            }
        }

        log(log_color, "run", &format!("{name} — glibc x {}", args.runs));
        let Some(glibc_runs) = execute(&glibc_bin, &wrapper, args.runs) else {
            log(log_color, "error", "glibc variant crashed — skipping");
            continue;
        };

        log(log_color, "run", &format!("{name} — miros x {}", args.runs));
        let Some(miros_runs) = execute(&miros_bin, &wrapper, args.runs) else {
            log(log_color, "error", "miros variant crashed — skipping");
            continue;
        };

        let glibc_stats = analyze(&glibc_runs);
        let miros_stats = analyze(&miros_runs);

        print_table(name, &glibc_stats, &miros_stats, table_color);

        if args.perf {
            run_perf(
                name,
                &glibc_bin,
                &miros_bin,
                &wrapper,
                args.runs,
                table_color,
            );
        }
    }
}

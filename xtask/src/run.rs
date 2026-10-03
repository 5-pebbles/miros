use std::{os::unix::process::ExitStatusExt, path::PathBuf, process::Command};

use clap::Args;

use crate::build;

#[derive(Args)]
pub struct RunArgs {
    /// The target binary to run under miros.
    binary: PathBuf,
    /// Working directory to run the binary from (for programs that read relative paths).
    #[arg(long)]
    dir: Option<PathBuf>,
    /// Cargo features to pass through (like `cargo --features`).
    #[arg(long)]
    features: Option<String>,
    /// Arguments forwarded to the binary.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

/// Build miros, then run the target binary through direct invocation: `libmiros.so <binary>`.
pub fn run(
    RunArgs {
        binary,
        dir,
        features,
        args,
    }: RunArgs,
) {
    let miros = build::run(build::BuildArgs {
        features,
        target_cpu: None,
    });

    let mut command = Command::new(miros);
    command.arg(&binary).args(&args);
    if let Some(dir) = &dir {
        command.current_dir(dir);
    }
    let status = command
        .status()
        .expect("failed to spawn the binary under miros");
    let code = status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .expect("process neither exited nor was signaled");
    std::process::exit(code);
}

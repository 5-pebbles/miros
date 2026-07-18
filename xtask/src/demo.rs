use std::{fs, path::PathBuf, process::Command};

use clap::Args;

use crate::build;

#[derive(Args)]
pub struct DemoArgs {
    /// The target binary to run under miros (a non-destructive copy is patched).
    binary: PathBuf,
    /// Working directory to run the binary from (for programs that read relative paths).
    #[arg(long)]
    dir: Option<PathBuf>,
    /// Arguments forwarded to the binary.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

/// Build miros, copy the target binary, repoint its interpreter at miros via patchelf, and run it.
pub fn run(demo: DemoArgs) {
    let miros = build::run();

    let patched = std::env::temp_dir().join("miros-demo");
    fs::copy(&demo.binary, &patched).expect("copy target binary");

    let patchelf = Command::new("patchelf")
        .arg("--set-interpreter")
        .arg(&miros)
        .arg(&patched)
        .status();
    match patchelf {
        Ok(status) if status.success() => {}
        Ok(_) => panic!("patchelf failed"),
        Err(_) => panic!("patchelf not found on PATH — install it (e.g. `nix run nixpkgs#patchelf`)"),
    }

    let mut command = Command::new(&patched);
    command.args(&demo.args);
    if let Some(dir) = &demo.dir {
        command.current_dir(dir);
    }
    let status = command.status().expect("failed to spawn the patched binary");
    std::process::exit(status.code().unwrap_or(1));
}

use std::{fs, path::PathBuf, process::Command};

use crate::{
    build,
    test::case::{Error, TestCase},
};

pub fn run() -> Result<(PathBuf, Vec<TestCase>), Error> {
    let miros = build::run(build::BuildArgs {
        features: None,
        target_cpu: None,
    });
    let root = build::workspace_root();

    let bin_dir = root.join("examples/bin");
    fs::create_dir_all(&bin_dir).map_err(|error| Error::Io {
        action: "create directory",
        path: bin_dir,
        error,
    })?;

    let cases = TestCase::discover(&root.join("examples"))?;
    for case in &cases {
        case.compile(&root, &miros)?;
    }

    // The Rust example is its own cargo project.
    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "build",
            "--release",
            "--manifest-path",
            "examples/hello_world/Cargo.toml",
        ])
        .status()
        .map_err(|error| Error::Io {
            action: "spawn cargo in",
            path: root.clone(),
            error,
        })?;
    if !status.success() {
        return Err(Error::CommandFailed {
            command: "cargo build --release --manifest-path examples/hello_world/Cargo.toml".into(),
            status,
        });
    }

    Ok((miros, cases))
}

use std::{fs, path::PathBuf, process::Command};

use crate::{build, test::parser::Parser};

pub fn run() -> PathBuf {
    let miros = build::run(build::BuildArgs {
        features: None,
        target_cpu: None,
    });
    let root = build::workspace_root();
    let interpreter = format!("-Wl,--dynamic-linker={}", miros.display());

    let bin_dir = root.join("examples/bin");
    fs::create_dir_all(&bin_dir).expect("create examples/bin");

    let mut sources: Vec<PathBuf> = fs::read_dir(root.join("examples"))
        .expect("read examples directory")
        .map(|entry| entry.expect("read directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "c"))
        .collect();
    sources.sort();

    for source in sources {
        let stem = source
            .file_stem()
            .expect("c file has a stem")
            .to_string_lossy()
            .into_owned();
        let source_code = fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("reading {} failed: {error}", source.display()));
        let case = Parser::new(&source_code)
            .parse()
            .unwrap_or_else(|diagnostics| {
                for diagnostic in &diagnostics {
                    diagnostic.emit(&source_code, &source);
                }
                panic!("parsing {} failed", source.display());
            });
        let cflags = case.cflags.unwrap_or_default();

        let status = Command::new("gcc")
            .current_dir(&root)
            .arg("-o")
            .arg(bin_dir.join(stem))
            .arg(&source)
            .args(cflags.split_whitespace())
            .arg(&interpreter)
            .status()
            .expect("failed to spawn gcc");
        assert!(status.success(), "compiling {} failed", source.display());
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
        .expect("failed to spawn cargo");
    assert!(status.success(), "building hello_world failed");

    miros
}

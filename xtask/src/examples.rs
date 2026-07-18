use std::{fs, process::Command};

use crate::build;

/// A C example: source stem under `examples/`, plus its extra `gcc` flags (libs, builtins).
struct Example {
    stem: &'static str,
    flags: &'static [&'static str],
}

const EXAMPLES: &[Example] = &[
    Example {
        stem: "print_deadbeef",
        flags: &["-lm"],
    },
    Example {
        stem: "sqrt_with_libm",
        flags: &["-lm"],
    },
    Example {
        stem: "thread_local",
        flags: &[],
    },
    Example {
        stem: "pthread_basic",
        flags: &["-lpthread"],
    },
    Example {
        stem: "thread_dtors",
        flags: &["-fno-builtin", "-lpthread"],
    },
];

pub fn run() {
    let miros = build::run();
    let root = build::workspace_root();
    let interpreter = format!("-Wl,--dynamic-linker={}", miros.display());

    let bin_dir = root.join("examples/bin");
    fs::create_dir_all(&bin_dir).expect("create examples/bin");

    for example in EXAMPLES {
        let status = Command::new("gcc")
            .current_dir(&root)
            .arg("-o")
            .arg(bin_dir.join(example.stem))
            .arg(format!("examples/{}.c", example.stem))
            .args(example.flags)
            .arg(&interpreter)
            .status()
            .expect("failed to spawn gcc");
        assert!(status.success(), "compiling {} failed", example.stem);
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
}

use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub const TARGET: &str = "x86_64-unknown-linux-gnu";

/// The workspace root, `xtask`'s manifest lives one level under it, so this is invocation-independent.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace member")
        .to_path_buf()
}

/// Build `libmiros.so` (release) and return its path.
pub fn run() -> PathBuf {
    let root = workspace_root();
    let aliases_version_script = crate::aliases::generate();

    let status = Command::new("cargo")
        .current_dir(&root)
        .env(
            "RUSTFLAGS",
            "-C target-cpu=native -Z unstable-options -C panic=immediate-abort -Z tls-model=initial-exec --cfg miros_aliases",
        )
        .args([
            "rustc",
            "-Z",
            "build-std=core,alloc,std",
            "--target",
            TARGET,
            "--release",
            "--",
            "-C",
            "link-arg=-nostartfiles",
            // We define our own intrinsics & are libc, so drop the driver's implicit libc/libgcc_s DT_NEEDED.
            "-C",
            "link-arg=-Wl,--as-needed",
            "-C",
            "link-arg=-Wl,-Bsymbolic",
            "-C",
            "link-arg=-Wl,-e,_start",
        ])
        .arg("-C")
        .arg(format!(
            "link-arg=-Wl,--version-script,{}",
            aliases_version_script.display()
        ))
        .status()
        .expect("failed to spawn cargo");
    assert!(status.success(), "release build failed");

    let miros = root.join(format!("target/{TARGET}/release/libmiros.so"));
    assert!(
        miros.exists(),
        "libmiros.so not found at {}",
        miros.display()
    );
    miros
}

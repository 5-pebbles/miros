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

    let status = Command::new("cargo")
        .current_dir(&root)
        .env(
            "RUSTFLAGS",
            "-C target-cpu=native -Z unstable-options -C panic=immediate-abort -Z tls-model=initial-exec",
        )
        .args([
            "rustc",
            "-Z",
            "build-std=core,alloc,std",
            "-Z",
            "build-std-features=mangled-names",
            // Without this ^, ld's visibility merge of our GLOBAL DEFAULT def with their WEAK HIDDEN def yields HIDDEN,
            // dropping every hardware-satisfiable math symbol from our .dynsym.
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

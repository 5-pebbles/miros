use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub const TARGET: &str = "x86_64-unknown-linux-gnu";

/// The workspace root — `xtask`'s manifest lives one level under it, so this is invocation-independent.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace member")
        .to_path_buf()
}

/// Build `libmiros.so` (release) and return its path. The release recipe rebuilds core/alloc/std
/// with immediate-abort + `target-cpu=native`, and passes the freestanding link flags miros needs:
/// no start files, bind internal references at link time, and set the entry point to `_start`.
/// The final link is driven through our own linker wrapper (`-C linker`) so we own it.
pub fn run() -> PathBuf {
    let root = workspace_root();
    let linker = build_linker(&root);

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
            // Mangle compiler_builtins' intrinsic symbols so its weak-hidden bare-C libm twins
            // (sqrt, floor, fma, …) don't exist to collide with our exports. Without this, ld's
            // visibility merge of our GLOBAL DEFAULT def with their WEAK HIDDEN def yields HIDDEN,
            // silently dropping every hardware-satisfiable math symbol from our .dynsym.
            "-Z",
            "build-std-features=mangled-names",
            "--target",
            TARGET,
            "--release",
            "--",
            "-C",
            &format!("linker={}", linker.display()),
            "-C",
            "linker-flavor=gcc",
            "-C",
            "link-arg=-nostartfiles",
            "-C",
            "link-arg=-Wl,-Bsymbolic",
            "-C",
            "link-arg=-Wl,-e,_start",
        ])
        .status()
        .expect("failed to spawn cargo");
    assert!(status.success(), "release build failed");

    root.join(format!("target/{TARGET}/release/libmiros.so"))
}

/// Build the `miros-ld` linker wrapper and return its path.
fn build_linker(root: &Path) -> PathBuf {
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "-p", "xtask", "--bin", "miros-ld"])
        .status()
        .expect("failed to spawn cargo");
    assert!(status.success(), "building the linker wrapper failed");
    root.join("target/debug/miros-ld")
}

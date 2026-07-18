// Linker wrapper for the miros final link. cargo invokes this in place of `cc`, so we receive the
// full, computed link line and can own it. For now it is a transparent pass-through to the real
// `cc`; owning the link is the foothold for later splitting the LTO codegen from a plain second
// link — which is how we re-add the hardware-libfunc aliases (`sqrt` → `__miros_sqrt`) that LTO
// drops from any symbol matching a target-satisfiable libfunc name.
use std::{env, process::Command};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let status = Command::new("cc")
        .args(&args)
        .status()
        .expect("failed to exec cc");
    std::process::exit(status.code().unwrap_or(1));
}

use std::{
    process::{self, Command},
    time::Duration,
};

use colored::Colorize;

use self::runner::TestRunner;
use crate::{build, examples};

pub const DIRECTIVE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) mod case;
mod diagnostic;
mod directive;
mod parser;
mod pty;
mod runner;
mod span;
mod utils;

pub fn run(filter: Option<String>, direct: bool) {
    let root = build::workspace_root();

    let status = Command::new("cargo")
        .current_dir(&root)
        .arg("test")
        .status()
        .expect("failed to spawn cargo test");
    assert!(status.success(), "unit tests failed");

    let (miros, cases) = examples::run().unwrap_or_else(|error| {
        error.emit();
        process::exit(1);
    });
    // `direct` swaps the launch mode from PT_INTERP to the miros CLI.
    // Every example then exercises argv compaction and the auxv retarget.
    let interpreter = direct.then_some(miros.as_path());

    println!();

    let (mut passed, mut failed) = (0, 0);

    for case in cases {
        if filter
            .as_ref()
            .is_some_and(|filter| !case.stem.contains(filter))
        {
            continue;
        }

        let stem = case.stem.clone();
        let runner = TestRunner::new(case);
        match runner.run(interpreter) {
            Ok(()) => {
                passed += 1;
                let ok_text = "OK".green().bold();
                println!("test {stem} ... {ok_text}");
            }
            Err(error) => {
                failed += 1;
                let failed_text = "FAILED".red().bold();
                println!("test {stem} ... {failed_text}");
                error.emit();
                println!();
            }
        }
    }

    println!("\ntest result: {passed} passed; {failed} failed");
    if failed > 0 {
        process::exit(1);
    }
}

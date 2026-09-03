use std::{fs, path::PathBuf, process, time::Duration};

use colored::Colorize;

use self::runner::{LoadError, TestRunner};
use crate::{build, examples};

pub const DIRECTIVE_TIMEOUT: Duration = Duration::from_secs(10);

mod diagnostic;
mod directive;
mod parser;
mod pty;
mod runner;
mod span;
mod utils;

pub fn run(filter: Option<String>) {
    examples::run();

    let root = build::workspace_root();
    let mut sources: Vec<PathBuf> = fs::read_dir(root.join("examples"))
        .expect("read examples directory")
        .map(|entry| entry.expect("read directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "c"))
        .collect();
    sources.sort();

    let (mut passed, mut failed) = (0, 0);

    for source in sources {
        let stem = source
            .file_stem()
            .expect("c file has a stem")
            .to_string_lossy()
            .into_owned();
        if filter.as_ref().is_some_and(|filter| !stem.contains(filter)) {
            continue;
        }

        let runner = match TestRunner::new(source.clone()) {
            Ok(runner) => runner,
            Err(LoadError::Read(error)) => {
                failed += 1;
                println!("test {stem} ... read error: {error}\n");
                continue;
            }
            Err(LoadError::Parse {
                source_code,
                diagnostics,
            }) => {
                failed += 1;
                println!("test {stem} ... parse error");
                for diagnostic in &diagnostics {
                    diagnostic.emit(&source_code, &source);
                }
                println!();
                continue;
            }
        };

        match runner.run() {
            Ok(()) => {
                passed += 1;
                let ok_text = "OK".green().bold();
                println!("test {stem} ... {ok_text}",);
            }
            Err(error) => {
                failed += 1;
                let failed_text = "FAILED".red().bold();
                println!("test {stem} ... {failed_text}");
                error.emit();
            }
        }
        println!();
    }

    println!("\ntest result: {passed} passed; {failed} failed");
    if failed > 0 {
        process::exit(1);
    }
}

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use super::{
    diagnostic::Diagnostic,
    directive::{Directive, StatusExpectation},
    parser::Parser,
};

#[derive(Debug, Clone, Default)]
pub struct TestCase {
    pub source: PathBuf,
    pub stem: String,
    pub args: Option<String>,
    pub cflags: Option<String>,
    pub directives: Vec<Directive>,
    /// Shell commands run in the scratch directory after fixture staging, before launch.
    pub setup: Vec<String>,
    pub status: Option<StatusExpectation>,
    /// Indexed by `Stream::index()`; a set flag moves that stream to a pipe.
    pub pipes: [bool; 2],
}

pub enum Error {
    Io {
        action: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    Parse {
        source: PathBuf,
        source_code: String,
        diagnostics: Vec<Diagnostic>,
    },
    CommandFailed {
        command: String,
        status: ExitStatus,
    },
}

impl Error {
    pub fn emit(&self) {
        match self {
            Self::Io {
                action,
                path,
                error,
            } => eprintln!("{action} {} failed: {error}", path.display()),
            Self::Parse {
                source,
                source_code,
                diagnostics,
            } => {
                for diagnostic in diagnostics {
                    diagnostic.emit(source_code, source);
                }
            }
            Self::CommandFailed { command, status } => {
                eprintln!("{command} failed with {status}");
            }
        }
    }
}

impl TestCase {
    pub fn discover(examples: &Path) -> Result<Vec<Self>, Error> {
        let mut sources = fs::read_dir(examples)
            .map_err(|error| Error::Io {
                action: "read directory",
                path: examples.to_path_buf(),
                error,
            })?
            .map(|entry| {
                entry.map(|entry| entry.path()).map_err(|error| Error::Io {
                    action: "read directory entry in",
                    path: examples.to_path_buf(),
                    error,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        sources.retain(|path| path.extension().is_some_and(|extension| extension == "c"));
        sources.sort();
        sources.into_iter().map(Self::load).collect()
    }

    fn load(source: PathBuf) -> Result<Self, Error> {
        let source_code = fs::read_to_string(&source).map_err(|error| Error::Io {
            action: "read",
            path: source.clone(),
            error,
        })?;
        let mut case = Parser::new(&source_code)
            .parse()
            .map_err(|diagnostics| Error::Parse {
                source: source.clone(),
                source_code,
                diagnostics,
            })?;
        case.stem = source
            .file_stem()
            .expect("a discovered C file has a stem")
            .to_string_lossy()
            .into_owned();
        case.source = source;
        Ok(case)
    }

    pub fn compile(&self, root: &Path, miros: &Path) -> Result<(), Error> {
        let interpreter = format!("-Wl,--dynamic-linker={}", miros.display());
        let binary = root.join("examples/bin").join(&self.stem);
        let cflags = self.cflags.as_deref().unwrap_or_default();

        let mut command = Command::new("gcc");
        command
            .current_dir(root)
            .arg("-o")
            .arg(&binary)
            .arg(&self.source)
            .args(cflags.split_whitespace())
            .arg(&interpreter);
        let status = command.status().map_err(|error| Error::Io {
            action: "spawn gcc for",
            path: self.source.clone(),
            error,
        })?;
        if status.success() {
            return Ok(());
        }
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        Err(Error::CommandFailed {
            command: format!("gcc {arguments}"),
            status,
        })
    }
}

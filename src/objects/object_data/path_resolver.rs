use std::{cell::RefCell, env, fs::File, path::PathBuf};

use crate::error::MirosError;

const DEFAULT_SEARCH_PATHS: &[&str] = &[
    "/lib",
    "/usr/lib",
    "/lib/x86_64-linux-gnu",
    "/usr/lib/x86_64-linux-gnu",
];

/// Resolves DT_NEEDED library names to open file handles by searching the standard ld.so directory order.
///
/// The variant determines where ELF-embedded search paths are inserted relative to LD_LIBRARY_PATH:
///
/// | Variant | Search Order                                              |
/// |---------|-----------------------------------------------------------|
/// | Rpath   | RPATH → LD_LIBRARY_PATH → /etc/ld.so.cache → defaults    |
/// | Runpath | LD_LIBRARY_PATH → RUNPATH → /etc/ld.so.cache → defaults   |
/// | None    | LD_LIBRARY_PATH → /etc/ld.so.cache → defaults             |
///
pub enum PathResolver {
    Rpath(*const str),
    Runpath(*const str),
    None,
}

impl PathResolver {
    fn elf_search_dirs(&self) -> impl Iterator<Item = &str> {
        let path_string = match self {
            Self::Rpath(pointer) | Self::Runpath(pointer) => unsafe { &**pointer },
            Self::None => "",
        };
        path_string.split(':').filter(|path| !path.is_empty())
    }

    fn open_first_match<'a>(
        &self,
        mut search_directories: impl Iterator<Item = &'a str>,
        dependency_name: &str,
    ) -> Option<File> {
        // PERF: Reuse a single PathBuf across calls to avoid per-probe allocations.
        // LLVM can't hoist this — each iteration escapes into an opaque syscall with a different length.
        thread_local! {
            static CANDIDATE_BUFFER: RefCell<PathBuf> = RefCell::new(PathBuf::new());
        }
        CANDIDATE_BUFFER.with_borrow_mut(|candidate| {
            search_directories.find_map(|directory| {
                candidate.clear();
                candidate.push(directory);
                candidate.push(dependency_name);
                File::open(&*candidate).ok()
            })
        })
    }

    /// Resolves a dependency name to an open file handle by probing search directories. Names containing a slash are treated as literal paths.
    pub fn resolve(&self, dependency_name: &str) -> Result<File, MirosError> {
        if dependency_name.contains('/') {
            return File::open(dependency_name)
                .map_err(|_| MirosError::DependencyNotFound(dependency_name.to_string()));
        }

        // PERF: This allocates a string me thinks...
        let ld_library_path = env::var("LD_LIBRARY_PATH").ok();
        let ld_library_path_dirs = ld_library_path
            .iter()
            .flat_map(|paths| paths.split(':'))
            .filter(|path| !path.is_empty());

        // TODO: search /etc/ld.so.cache between LD_LIBRARY_PATH and defaults
        let default_dirs = DEFAULT_SEARCH_PATHS.iter().copied();

        match self {
            Self::Rpath(_) => self.open_first_match(
                self.elf_search_dirs()
                    .chain(ld_library_path_dirs)
                    .chain(default_dirs),
                dependency_name,
            ),
            Self::None | Self::Runpath(_) => self.open_first_match(
                ld_library_path_dirs
                    .chain(self.elf_search_dirs())
                    .chain(default_dirs),
                dependency_name,
            ),
        }
        .ok_or_else(|| MirosError::DependencyNotFound(dependency_name.to_string()))
    }
}

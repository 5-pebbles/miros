use std::{env, fs::File, path::PathBuf};

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
pub enum PathResolver {
    None,
    Rpath(*const str),
    Runpath(*const str),
}

impl PathResolver {
    fn elf_search_dirs(&self) -> impl Iterator<Item = &str> {
        let path_string = match self {
            Self::Rpath(pointer) | Self::Runpath(pointer) => unsafe { &**pointer },
            Self::None => "",
        };
        path_string.split(':').filter(|path| !path.is_empty())
    }

    /// Resolves a dependency name to an open file handle by probing search directories. Names containing a slash are treated as literal paths.
    pub fn resolve(&self, dependency_name: &str) -> Result<File, MirosError> {
        if dependency_name.contains('/') {
            return File::open(dependency_name)
                .map_err(|_| MirosError::DependencyNotFound(dependency_name.to_string()));
        }

        let ld_library_path = env::var("LD_LIBRARY_PATH").ok();
        let ld_library_path_dirs = ld_library_path
            .iter()
            .flat_map(|paths| paths.split(':'))
            .filter(|path| !path.is_empty());

        // TODO: search /etc/ld.so.cache between LD_LIBRARY_PATH and defaults (see #41)
        let default_dirs = DEFAULT_SEARCH_PATHS.iter().copied();

        let search_directories: Box<dyn Iterator<Item = &str>> = match self {
            Self::Rpath(_) => Box::new(
                self.elf_search_dirs()
                    .chain(ld_library_path_dirs)
                    .chain(default_dirs),
            ),
            Self::None | Self::Runpath(_) => Box::new(
                ld_library_path_dirs
                    .chain(self.elf_search_dirs())
                    .chain(default_dirs),
            ),
        };

        for directory in search_directories {
            let candidate = PathBuf::from(directory).join(dependency_name);
            if let Ok(file) = File::open(&candidate) {
                return Ok(file);
            }
        }

        Err(MirosError::DependencyNotFound(dependency_name.to_string()))
    }
}

use std::{ffi::CStr, fmt::Write as _};

use crate::{error::MirosError, start::config::ConfigOverrides, syscall::exit::exit};

macro_rules! flags {
    (
        |$state:ident: $state_type:ty|

        $(
            #[doc = $help:expr]
            $variant:ident ($long_name:expr) { $($body:tt)* }
        )+
    ) => {
        #[derive(Clone, Copy)]
        enum Flag {
            $(#[doc = $help] $variant,)+
        }

        impl Flag {
            const TABLE: &[Self] = &[$(Self::$variant),+];

            fn entry(self) -> (&'static str, &'static str) {
                match self {
                    $(Self::$variant => ($long_name, $help.trim()),)+
                }
            }

            fn from_long_name(bytes: &[u8]) -> Option<Self> {
                Self::TABLE
                    .iter()
                    .copied()
                    .find(|flag| flag.entry().0.as_bytes() == bytes)
            }

            fn apply(self, $state: $state_type) {
                match self {
                    $(Self::$variant => { $($body)* })+
                }
            }
        }
    };
}

flags! {
    |overrides: &mut ConfigOverrides|

    /// Bind unresolved strong symbols to null with a warning instead of aborting
    LenientUndefinedSymbols("--lenient-undefined-symbols") { overrides.lenient = Some(true); }
    /// Abort load on unresolved strong symbols (the default); overrides MIROS_LENIENT_UNDEFINED_SYMBOLS
    StrictUndefinedSymbols("--strict-undefined-symbols") { overrides.lenient = Some(false); }
    /// Print this message
    Help("--help") { print!("{}", usage()); exit(0); }
    /// Print version information
    Version("--version") { println!("miros {}", env!("CARGO_PKG_VERSION")); exit(0); }
}

pub unsafe fn parse(arguments: &[*const u8]) -> Result<(usize, ConfigOverrides), MirosError> {
    let mut overrides = ConfigOverrides::default();

    let mut index = 1;
    while let Some(&argument) = arguments.get(index) {
        let bytes = CStr::from_ptr(argument.cast()).to_bytes();
        if bytes == b"--" {
            index += 1;
            break;
        }
        if !bytes.starts_with(b"--") {
            break;
        }

        let flag = Flag::from_long_name(bytes).ok_or_else(|| {
            MirosError::UnrecognizedOption(String::from_utf8_lossy(bytes).into_owned())
        })?;
        flag.apply(&mut overrides);
        index += 1;
    }

    if arguments.get(index).is_none() {
        return Err(MirosError::MissingExecutable);
    }

    Ok((index, overrides))
}

pub fn usage() -> String {
    let longest_name = Flag::TABLE
        .iter()
        .map(|flag| flag.entry().0.len())
        .max()
        .unwrap_or(0);

    let mut usage = String::with_capacity(1024);
    usage.push_str(USAGE_HEADER);
    for flag in Flag::TABLE {
        let (name, help) = flag.entry();
        let _ = write!(usage, "      {name:<longest_name$}   {help}\n");
    }
    usage.push_str(USAGE_ENVIRONMENT);
    usage
}

const USAGE_HEADER: &str =
    "Usage: miros [OPTIONS] EXECUTABLE [ARGUMENTS...]\n\nRun EXECUTABLE with miros as the dynamic linker.\n\nOptions:\n";

const USAGE_ENVIRONMENT: &str =
    "\nEnvironment:\n  MIROS_LENIENT_UNDEFINED_SYMBOLS   1 or 0; command-line flags win\n";

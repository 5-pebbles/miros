use std::{ffi::CStr, fmt::Write as _};

use crate::{error::MirosError, start::config::ConfigOverrides};

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

            fn long_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $long_name,)+
                }
            }

            fn help(self) -> &'static str {
                match self {
                    $(Self::$variant => $help.trim()),+
                }
            }

            fn from_long_name(bytes: &[u8]) -> Option<Self> {
                Self::TABLE
                    .iter()
                    .copied()
                    .find(|flag| flag.long_name().as_bytes() == bytes)
            }

            /// A body that returns early ends parsing with that invocation.
            #[allow(unreachable_code)]
            fn apply(self, $state: $state_type) -> Option<Invocation> {
                match self {
                    $(Self::$variant => { $($body)* None })+
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
    Help("--help") { return Some(Invocation::Help); }
    /// Print version information
    Version("--version") { return Some(Invocation::Version); }
}

#[derive(Debug)]
pub enum Invocation {
    Run {
        executable_index: usize,
        overrides: ConfigOverrides,
    },
    Help,
    Version,
}

impl Invocation {
    pub unsafe fn parse(arguments: &[*const u8]) -> Result<Self, MirosError> {
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
            if let Some(invocation) = flag.apply(&mut overrides) {
                return Ok(invocation);
            }
            index += 1;
        }

        if arguments.get(index).is_none() {
            return Err(MirosError::MissingExecutable);
        }

        Ok(Self::Run {
            executable_index: index,
            overrides,
        })
    }

    pub unsafe fn parse_or_exit(arguments: &[*const u8]) -> (usize, ConfigOverrides) {
        match Self::parse(arguments) {
            Ok(Self::Run {
                executable_index,
                overrides,
            }) => (executable_index, overrides),
            Ok(Self::Help) => {
                print!("{}", Self::usage());
                crate::syscall::exit::exit(0);
            }
            Ok(Self::Version) => {
                println!("miros {}", env!("CARGO_PKG_VERSION"));
                crate::syscall::exit::exit(0);
            }
            Err(error) => {
                eprint!("{error}\n\n{}", Self::usage());
                crate::syscall::exit::exit(2);
            }
        }
    }

    pub fn usage() -> String {
        let longest_name = Flag::TABLE
            .iter()
            .map(|flag| flag.long_name().len())
            .max()
            .unwrap_or(0);

        let mut usage = String::with_capacity(1024);
        usage.push_str(USAGE_HEADER);
        for flag in Flag::TABLE {
            let _ = write!(
                usage,
                "      {:<longest_name$}   {}\n",
                flag.long_name(),
                flag.help()
            );
        }
        usage.push_str(USAGE_ENVIRONMENT);
        usage
    }
}

const USAGE_HEADER: &str =
    "Usage: miros [OPTIONS] EXECUTABLE [ARGUMENTS...]\n\nRun EXECUTABLE with miros as the dynamic linker.\n\nOptions:\n";

const USAGE_ENVIRONMENT: &str =
    "\nEnvironment:\n  MIROS_LENIENT_UNDEFINED_SYMBOLS   1 or 0; command-line flags win\n";

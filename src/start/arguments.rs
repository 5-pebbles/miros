use std::{ffi::CStr, fmt::Write as _};

use crate::{error::MirosError, start::config::ConfigOverrides, syscall::exit::exit};

macro_rules! flags {
    (
        |$state:ident: $state_type:ty, $value:ident|

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

            fn apply(self, $state: $state_type, $value: Option<&[u8]>) -> Result<(), MirosError> {
                match self {
                    $(Self::$variant => { $($body)* })+
                }
            }
        }
    };
}

flags! {
    |overrides: &mut ConfigOverrides, value|

    /// Bind unresolved strong symbols to null with a warning instead of aborting.
    LenientUndefinedSymbols("--lenient-undefined-symbols") {
        overrides.lenient_undefined_symbols = Some(true);
        Ok(())
    }
    /// Abort load on unresolved strong symbols (the default). Overrides MIROS_LENIENT_UNDEFINED_SYMBOLS.
    StrictUndefinedSymbols("--strict-undefined-symbols") {
        overrides.lenient_undefined_symbols = Some(false);
        Ok(())
    }
    /// Resolve PLT symbols on first call or at load. Overrides environment settings.
    Binding("--binding=lazy|now") {
        let value = value.ok_or(MirosError::MissingOptionValue("--binding"))?;
        overrides.lazy_binding = match value {
            b"lazy" => Some(true),
            b"now" => Some(false),
            _ => {
                return Err(MirosError::InvalidOptionValue {
                    option: "--binding",
                    value: String::from_utf8_lossy(value).into_owned(),
                });
            }
        };
        Ok(())
    }
    /// Print this message.
    Help("--help") { print!("{}", usage()); exit(0); }
    /// Print version information.
    Version("--version") { println!("miros {}", env!("CARGO_PKG_VERSION")); exit(0); }
}

impl Flag {
    fn from_argument(bytes: &[u8]) -> Option<(Self, Option<&[u8]>)> {
        if bytes == b"--binding" {
            return Some((Self::Binding, None));
        }
        if let Some(value) = bytes.strip_prefix(b"--binding=") {
            return Some((Self::Binding, Some(value)));
        }
        Self::from_long_name(bytes).map(|flag| (flag, None))
    }
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

        let (flag, value) = Flag::from_argument(bytes).ok_or_else(|| {
            MirosError::UnrecognizedOption(String::from_utf8_lossy(bytes).into_owned())
        })?;
        flag.apply(&mut overrides, value)?;
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
    "\nEnvironment:\n  MIROS_LENIENT_UNDEFINED_SYMBOLS   1 or 0. Command-line flags win.\n  MIROS_LAZY_BINDING                1 or 0 (default 1).\n  LD_BIND_NOW                       Nonempty except 0 disables lazy binding.\n";

#[cfg(test)]
mod tests {
    use super::{ConfigOverrides, Flag};
    use crate::error::MirosError;

    fn overrides_for(argument: &[u8]) -> Result<ConfigOverrides, MirosError> {
        let (flag, value) = Flag::from_argument(argument).unwrap();
        let mut overrides = ConfigOverrides::default();
        flag.apply(&mut overrides, value)?;
        Ok(overrides)
    }

    #[test]
    fn binding_accepts_lazy_and_now_values() {
        assert_eq!(
            overrides_for(b"--binding=lazy").unwrap().lazy_binding,
            Some(true)
        );
        assert_eq!(
            overrides_for(b"--binding=now").unwrap().lazy_binding,
            Some(false)
        );
    }

    #[test]
    fn binding_rejects_missing_and_invalid_values() {
        assert!(matches!(
            overrides_for(b"--binding"),
            Err(MirosError::MissingOptionValue("--binding"))
        ));
        assert!(matches!(
            overrides_for(b"--binding=eventually"),
            Err(MirosError::InvalidOptionValue { .. })
        ));
    }
}

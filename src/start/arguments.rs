use std::{ffi::CStr, fmt::Write as _, ptr};

use crate::{
    error::MirosError,
    start::{auxiliary_vector::AuxiliaryVectorItem, config::ConfigOverrides},
};

/// One entry per flag: a doc comment with the help text, the variant, the long name, and the braced body that applies it.
/// The `|state: Type|` head names and types the state the bodies mutate.
/// The usage listing is generated from this table, so the parser and `--help` cannot drift apart.
macro_rules! flags {
    (
        |$state:ident: $state_type:ty|

        $(
            #[doc = $help:expr]
            $variant:ident ($long_name:expr) { $($body:tt)* }
        )+
    ) => {
        /// The flags miros recognizes when invoked directly, in declaration order.
        #[derive(Clone, Copy)]
        enum Flag {
            $(#[doc = $help] $variant,)+
        }

        impl Flag {
            /// Declaration order doubles as the usage listing order.
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

            /// Applies the flag to the command-line state; a body that returns early ends parsing with that invocation.
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

/// The parsed miros command line.
#[derive(Debug)]
pub enum Invocation {
    Run {
        /// Index of the executable path within the original argv; everything before it is miros's own.
        executable_index: usize,
        overrides: ConfigOverrides,
    },
    Help,
    Version,
}

impl Invocation {
    /// Parses miros's own flags, stopping at the first operand (the executable path) or `--`.
    /// Operands past that index belong to the executable.
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

    /// Parses miros's flags, printing and exiting on help, version, and error.
    /// Returning means an executable was named; the call yields its argv index and the config overrides.
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

pub struct CompactedStack {
    pub arg_count: usize,
    pub env_pointer: *mut *const u8,
    pub auxv_pointer: *mut AuxiliaryVectorItem,
}

impl CompactedStack {
    /// Drops `drop_count` leading argv entries by shifting the argv/env/auxv block down in place, so the executable's `_start` pops its own argc/argv from the original stack pointer.
    ///
    /// `env_pointer` and `auxv_pointer` must point into the same block, past the argv array.
    pub unsafe fn compact(
        stack_pointer: *mut usize,
        env_pointer: *mut *const u8,
        auxv_pointer: *mut AuxiliaryVectorItem,
        drop_count: usize,
    ) -> Self {
        let arg_count = *stack_pointer;
        let arg_pointer = stack_pointer.add(1).cast::<*const u8>();

        // Walk the auxv past its AT_NULL pair: that is the end of the pointer block. The strings above it stay.
        let mut block_end = auxv_pointer.cast::<usize>();
        while *block_end != 0 {
            block_end = block_end.add(2);
        }
        block_end = block_end.add(2);

        let source = arg_pointer.add(drop_count) as *const usize;
        let destination = arg_pointer as *mut usize;
        let word_count = (block_end as usize - source as usize) / size_of::<usize>();
        ptr::copy(source, destination, word_count);
        let remaining_arg_count = arg_count - drop_count;
        *stack_pointer = remaining_arg_count;

        // Every pointer into the block shifts down by the same byte count; the auxv pointee is two words,
        // so its pointer moves by drop_count words, not drop_count items.
        let shift_in_bytes = drop_count * size_of::<usize>();
        Self {
            arg_count: remaining_arg_count,
            env_pointer: env_pointer.byte_sub(shift_in_bytes),
            auxv_pointer: auxv_pointer.byte_sub(shift_in_bytes),
        }
    }
}
#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;
    use crate::start::auxiliary_vector::AuxiliaryVectorType;

    fn parse_arguments(arguments: &[&str]) -> Result<Invocation, MirosError> {
        let owned: Vec<CString> = arguments
            .iter()
            .map(|argument| CString::new(*argument).unwrap())
            .collect();
        let pointers: Vec<*const u8> = owned
            .iter()
            .map(|argument| argument.as_ptr().cast())
            .collect();
        unsafe { Invocation::parse(&pointers) }
    }

    fn lenient(invocation: &Invocation) -> Option<bool> {
        match invocation {
            Invocation::Run { overrides, .. } => overrides.lenient,
            _ => None,
        }
    }

    #[test]
    fn first_operand_stops_flag_parsing() {
        let invocation = parse_arguments(&["miros", "./binary", "--help"]).unwrap();
        assert!(matches!(invocation, Invocation::Run {
            executable_index: 1,
            ..
        }));
    }

    #[test]
    fn double_dash_ends_flag_parsing() {
        let invocation = parse_arguments(&["miros", "--", "--help"]).unwrap();
        assert!(matches!(invocation, Invocation::Run {
            executable_index: 2,
            ..
        }));
    }

    #[test]
    fn help_and_version_are_terminal() {
        assert!(matches!(
            parse_arguments(&["miros", "--help"]),
            Ok(Invocation::Help)
        ));
        assert!(matches!(
            parse_arguments(&["miros", "--version"]),
            Ok(Invocation::Version)
        ));
    }

    #[test]
    fn the_last_lenient_or_strict_flag_wins() {
        let strict_after_lenient = parse_arguments(&[
            "miros",
            "--lenient-undefined-symbols",
            "--strict-undefined-symbols",
            "./binary",
        ])
        .unwrap();
        assert_eq!(lenient(&strict_after_lenient), Some(false));

        let lenient_after_strict = parse_arguments(&[
            "miros",
            "--strict-undefined-symbols",
            "--lenient-undefined-symbols",
            "./binary",
        ])
        .unwrap();
        assert_eq!(lenient(&lenient_after_strict), Some(true));

        let default = parse_arguments(&["miros", "./binary"]).unwrap();
        assert_eq!(lenient(&default), None);
    }

    #[test]
    fn unknown_flag_is_reported() {
        let error = parse_arguments(&["miros", "--bogus"]).unwrap_err();
        assert!(matches!(error, MirosError::UnrecognizedOption(name) if name == "--bogus"));
    }

    #[test]
    fn missing_executable_is_reported() {
        assert!(matches!(
            parse_arguments(&["miros"]),
            Err(MirosError::MissingExecutable)
        ));
        assert!(matches!(
            parse_arguments(&["miros", "--"]),
            Err(MirosError::MissingExecutable)
        ));
    }

    #[test]
    fn compact_shifts_the_pointer_block_down() {
        let miros_path = CString::new("/lib/libmiros.so").unwrap();
        let binary_path = CString::new("./print_deadbeef").unwrap();
        let home = CString::new("HOME=/home/ghostbird").unwrap();
        let phdr = 0x1234;

        // argc 2, argv [miros, binary], null, env [HOME], null, auxv [(AT_PHDR, phdr), (AT_NULL, 0)].
        let mut stack: Vec<usize> = vec![
            2,
            miros_path.as_ptr() as usize,
            binary_path.as_ptr() as usize,
            0,
            home.as_ptr() as usize,
            0,
            AuxiliaryVectorType::Phdr as usize,
            phdr,
            AuxiliaryVectorType::Null as usize,
            0,
        ];

        let (stack_pointer, env_pointer, auxv_pointer) = unsafe {
            let stack_pointer = stack.as_mut_ptr();
            let arg_pointer = stack_pointer.add(1).cast::<*const u8>();
            let env_pointer = arg_pointer.add(3);
            let auxv_pointer = env_pointer.add(2).cast::<AuxiliaryVectorItem>();
            (stack_pointer, env_pointer, auxv_pointer)
        };

        let compacted =
            unsafe { CompactedStack::compact(stack_pointer, env_pointer, auxv_pointer, 1) };

        assert_eq!(compacted.arg_count, 1);
        assert_eq!(stack[0], 1);
        assert_eq!(stack[1], binary_path.as_ptr() as usize);
        assert_eq!(stack[2], 0);
        assert_eq!(stack[3], home.as_ptr() as usize);
        assert_eq!(stack[4], 0);
        assert_eq!(stack[5], AuxiliaryVectorType::Phdr as usize);
        assert_eq!(stack[6], phdr);
        assert_eq!(stack[7], AuxiliaryVectorType::Null as usize);
        assert_eq!(stack[8], 0);
        unsafe {
            assert_eq!(*compacted.env_pointer as usize, home.as_ptr() as usize);
            assert!(matches!(
                (*compacted.auxv_pointer).a_type(),
                Ok(AuxiliaryVectorType::Phdr)
            ));
        }
    }
}

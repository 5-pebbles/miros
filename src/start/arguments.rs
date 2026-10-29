use std::{
    ffi::{CStr, OsString},
    os::unix::ffi::OsStringExt,
};

use clap::{Parser, ValueEnum};

use crate::start::config::ConfigOverrides;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Binding {
    Lazy,
    Now,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum UndefinedSymbols {
    Strict,
    Lenient,
}

#[derive(Debug, Parser)]
#[command(
    name = "miros",
    version,
    about = "Run EXECUTABLE with miros as the dynamic linker."
)]
#[command(override_usage = "miros [OPTIONS] EXECUTABLE [ARGUMENTS...]")]
#[command(args_override_self = true, after_help = ENVIRONMENT_HELP)]
struct CommandLine {
    /// Resolve PLT symbols on first call or at load.
    #[arg(long, value_enum)]
    binding: Option<Binding>,

    /// Reject unresolved strong symbols, or warn and bind them to null.
    #[arg(long, value_enum)]
    undefined_symbols: Option<UndefinedSymbols>,

    /// Executable and its arguments.
    #[arg(required = true, trailing_var_arg = true, value_name = "EXECUTABLE")]
    arguments: Vec<OsString>,
}

const ENVIRONMENT_HELP: &str = "Environment:\n  MIROS_LENIENT_UNDEFINED_SYMBOLS   1 or 0. Command-line options win.\n  MIROS_LAZY_BINDING                1 or 0 (default 1).\n  LD_BIND_NOW                       Nonempty except 0 disables lazy binding.";

fn parse_from(arguments: Vec<OsString>) -> Result<(usize, ConfigOverrides), clap::Error> {
    let argument_count = arguments.len();
    let command_line = CommandLine::try_parse_from(arguments)?;
    let executable_index = argument_count - command_line.arguments.len();
    let overrides = ConfigOverrides {
        lenient_undefined_symbols: command_line
            .undefined_symbols
            .map(|policy| matches!(policy, UndefinedSymbols::Lenient)),
        lazy_binding: command_line
            .binding
            .map(|binding| matches!(binding, Binding::Lazy)),
    };
    Ok((executable_index, overrides))
}

pub unsafe fn parse(arguments: &[*const u8]) -> (usize, ConfigOverrides) {
    let arguments = arguments
        .iter()
        .map(|argument| OsString::from_vec(CStr::from_ptr(argument.cast()).to_bytes().to_vec()))
        .collect();
    parse_from(arguments).unwrap_or_else(|error| error.exit())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{parse_from, ConfigOverrides};

    fn parse_arguments(arguments: &[&str]) -> Result<(usize, ConfigOverrides), clap::Error> {
        parse_from(arguments.iter().map(OsString::from).collect())
    }

    #[test]
    fn maps_options_to_configuration_overrides() {
        let (_, overrides) = parse_arguments(&[
            "miros",
            "--binding=now",
            "--undefined-symbols=lenient",
            "program",
        ])
        .unwrap();
        assert_eq!(overrides.lazy_binding, Some(false));
        assert_eq!(overrides.lenient_undefined_symbols, Some(true));

        let (_, overrides) = parse_arguments(&[
            "miros",
            "--binding=lazy",
            "--undefined-symbols=strict",
            "program",
        ])
        .unwrap();
        assert_eq!(overrides.lazy_binding, Some(true));
        assert_eq!(overrides.lenient_undefined_symbols, Some(false));
    }

    #[test]
    fn preserves_executable_argument_boundaries() {
        let (index, overrides) = parse_arguments(&["miros", "program", "--binding=now"]).unwrap();
        assert_eq!(index, 1);
        assert_eq!(overrides, ConfigOverrides::default());

        let (index, _) = parse_arguments(&["miros", "--", "--program"]).unwrap();
        assert_eq!(index, 2);

        assert!(parse_arguments(&["miros", "--unknown", "program"]).is_err());
    }
}

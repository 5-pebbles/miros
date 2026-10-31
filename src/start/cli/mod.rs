use std::{
    ffi::{CStr, OsString},
    fmt::Write as _,
    os::unix::ffi::OsStringExt,
};

use clap::{
    Parser,
    builder::{StyledStr, Styles},
};

use crate::start::config::{BindingMode, ConfigOverrides, UndefinedSymbolsPolicy};

mod color;
pub mod direct_invocation;

#[derive(Debug, Parser)]
#[command(
    name = "miros",
    version,
    about = "Run EXECUTABLE with miros as the runtime."
)]
#[command(override_usage = "miros [OPTIONS] EXECUTABLE [ARGUMENTS...]")]
#[command(args_override_self = true, after_help = environment_help())]
struct CommandLine {
    /// Resolve PLT symbols on first call or during startup.
    #[arg(long, value_enum)]
    binding: Option<BindingMode>,

    /// Reject unresolved strong symbols, or warn and bind them to null.
    #[arg(long, value_enum)]
    undefined_symbols: Option<UndefinedSymbolsPolicy>,

    /// Executable and its arguments.
    #[arg(required = true, trailing_var_arg = true, value_name = "EXECUTABLE")]
    arguments: Vec<OsString>,
}

const ENVIRONMENT_VARIABLES: &[(&str, &str)] = &[
    (
        "MIROS_LENIENT_UNDEFINED_SYMBOLS",
        "1 or 0. Command-line options win.",
    ),
    ("MIROS_LAZY_BINDING", "1 or 0. Command-line options win."),
    ("LD_BIND_NOW", "Nonempty. Force eager binding."),
];

fn environment_help() -> StyledStr {
    let styles = Styles::styled();
    let header = styles.get_header();
    let name_width = ENVIRONMENT_VARIABLES
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);
    let mut help = StyledStr::new();
    let _ = write!(
        help,
        "{}Environment:{}",
        header.render(),
        header.render_reset()
    );
    for (name, description) in ENVIRONMENT_VARIABLES {
        let _ = write!(help, "\n  {name:<name_width$}   {description}");
    }
    help
}

fn parse_from(arguments: Vec<OsString>) -> Result<(usize, ConfigOverrides), clap::Error> {
    let argument_count = arguments.len();
    let command_line = CommandLine::try_parse_from(arguments)?;
    let executable_index = argument_count - command_line.arguments.len();
    let overrides = ConfigOverrides {
        undefined_symbols: command_line.undefined_symbols,
        binding: command_line.binding,
    };
    Ok((executable_index, overrides))
}

pub unsafe fn parse(
    arguments: &[*const u8],
    environment_pointer: *mut *mut u8,
) -> (usize, ConfigOverrides) {
    let arguments = arguments
        .iter()
        .map(|argument| OsString::from_vec(CStr::from_ptr(argument.cast()).to_bytes().to_vec()))
        .collect();
    parse_from(arguments).unwrap_or_else(|error| color::print_and_exit(error, environment_pointer))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{BindingMode, ConfigOverrides, UndefinedSymbolsPolicy, parse_from};

    fn parse_arguments(arguments: &[&str]) -> Result<(usize, ConfigOverrides), clap::Error> {
        parse_from(arguments.iter().map(OsString::from).collect())
    }

    #[test]
    fn maps_options_to_configuration_overrides() {
        let (_, overrides) =
            parse_arguments(&["miros", "--undefined-symbols=lenient", "program"]).unwrap();
        assert_eq!(
            overrides.undefined_symbols,
            Some(UndefinedSymbolsPolicy::Lenient)
        );

        let (_, overrides) =
            parse_arguments(&["miros", "--undefined-symbols=strict", "program"]).unwrap();
        assert_eq!(
            overrides.undefined_symbols,
            Some(UndefinedSymbolsPolicy::Strict)
        );

        let (_, overrides) = parse_arguments(&["miros", "--binding=lazy", "program"]).unwrap();
        assert_eq!(overrides.binding, Some(BindingMode::Lazy));

        let (_, overrides) = parse_arguments(&["miros", "--binding=now", "program"]).unwrap();
        assert_eq!(overrides.binding, Some(BindingMode::Now));
    }

    #[test]
    fn preserves_executable_argument_boundaries() {
        let (index, overrides) =
            parse_arguments(&["miros", "program", "--undefined-symbols=lenient"]).unwrap();
        assert_eq!(index, 1);
        assert_eq!(overrides, ConfigOverrides::default());

        let (index, _) = parse_arguments(&["miros", "--", "--program"]).unwrap();
        assert_eq!(index, 2);

        assert!(parse_arguments(&["miros", "--unknown", "program"]).is_err());
    }
}

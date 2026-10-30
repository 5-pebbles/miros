use std::{
    ffi::{CStr, OsString},
    fmt::Write as _,
    io::{self, IsTerminal, Write},
    os::unix::ffi::OsStringExt,
};

use clap::{
    builder::{StyledStr, Styles},
    Parser, ValueEnum,
};

use crate::{
    start::{config::ConfigOverrides, environment_variables::EnvironmentIter},
    syscall::exit::exit,
};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum UndefinedSymbols {
    Strict,
    Lenient,
}

#[derive(Debug, Parser)]
#[command(
    name = "miros",
    version,
    about = "Run EXECUTABLE with miros as the runtime."
)]
#[command(override_usage = "miros [OPTIONS] EXECUTABLE [ARGUMENTS...]")]
#[command(args_override_self = true, after_help = environment_help())]
struct CommandLine {
    /// Reject unresolved strong symbols, or warn and bind them to null.
    #[arg(long, value_enum)]
    undefined_symbols: Option<UndefinedSymbols>,

    /// Executable and its arguments.
    #[arg(required = true, trailing_var_arg = true, value_name = "EXECUTABLE")]
    arguments: Vec<OsString>,
}

const ENVIRONMENT_VARIABLES: &[(&str, &str)] = &[(
    "MIROS_LENIENT_UNDEFINED_SYMBOLS",
    "1 or 0. Command-line options win.",
)];

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
        lenient: command_line
            .undefined_symbols
            .map(|policy| matches!(policy, UndefinedSymbols::Lenient)),
    };
    Ok((executable_index, overrides))
}

#[derive(Default)]
struct ColorEnvironment {
    no_color: bool,
    clicolor_force: bool,
    clicolor: Option<bool>,
    terminal_supports_color: bool,
    continuous_integration: bool,
}

impl ColorEnvironment {
    fn from_variables<'a>(variables: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        variables
            .into_iter()
            .fold(Self::default(), |mut environment, (name, value)| {
                match name {
                    "NO_COLOR" => environment.no_color = !value.is_empty(),
                    "CLICOLOR_FORCE" => environment.clicolor_force = !value.is_empty(),
                    "CLICOLOR" => environment.clicolor = Some(value != "0"),
                    "TERM" => environment.terminal_supports_color = value != "dumb",
                    "CI" => environment.continuous_integration = true,
                    _ => {}
                }
                environment
            })
    }

    fn enabled_for(self, is_terminal: bool) -> bool {
        if self.no_color {
            return false;
        }
        if self.clicolor_force {
            return true;
        }
        if self.clicolor == Some(false) {
            return false;
        }
        is_terminal
            && (self.terminal_supports_color
                || self.clicolor == Some(true)
                || self.continuous_integration)
    }
}

fn write_rendered(output: &mut dyn Write, rendered: &StyledStr, colored: bool) {
    if colored {
        let _ = write!(output, "{}", rendered.ansi());
    } else {
        let _ = write!(output, "{rendered}");
    }
}

unsafe fn print_and_exit(error: clap::Error, environment_pointer: *mut *mut u8) -> ! {
    let use_stderr = error.use_stderr();
    let is_terminal = if use_stderr {
        io::stderr().is_terminal()
    } else {
        io::stdout().is_terminal()
    };
    let colored = ColorEnvironment::from_variables(EnvironmentIter::new(environment_pointer))
        .enabled_for(is_terminal);
    let rendered = error.render();
    if use_stderr {
        write_rendered(&mut io::stderr().lock(), &rendered, colored);
    } else {
        write_rendered(&mut io::stdout().lock(), &rendered, colored);
    }
    exit(error.exit_code() as usize)
}

pub unsafe fn parse(
    arguments: &[*const u8],
    environment_pointer: *mut *mut u8,
) -> (usize, ConfigOverrides) {
    let arguments = arguments
        .iter()
        .map(|argument| OsString::from_vec(CStr::from_ptr(argument.cast()).to_bytes().to_vec()))
        .collect();
    parse_from(arguments).unwrap_or_else(|error| print_and_exit(error, environment_pointer))
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
        let (_, overrides) =
            parse_arguments(&["miros", "--undefined-symbols=lenient", "program"]).unwrap();
        assert_eq!(overrides.lenient, Some(true));

        let (_, overrides) =
            parse_arguments(&["miros", "--undefined-symbols=strict", "program"]).unwrap();
        assert_eq!(overrides.lenient, Some(false));
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

#[cfg(test)]
mod color_tests {
    use super::ColorEnvironment;

    #[test]
    fn resolves_color_from_environment_and_terminal() {
        for (variables, is_terminal, expected) in [
            (&[][..], true, false),
            (&[("TERM", "xterm")][..], true, true),
            (&[("TERM", "xterm")][..], false, false),
            (&[("TERM", "dumb")][..], true, false),
            (&[("CLICOLOR", "1")][..], true, true),
            (&[("CI", "")][..], true, true),
            (&[("CLICOLOR_FORCE", "1")][..], false, true),
            (
                &[("CLICOLOR_FORCE", "1"), ("CLICOLOR", "0")][..],
                false,
                true,
            ),
            (
                &[("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")][..],
                true,
                false,
            ),
            (&[("CLICOLOR", "0"), ("TERM", "xterm")][..], true, false),
        ] {
            let environment = ColorEnvironment::from_variables(variables.iter().copied());
            assert_eq!(
                environment.enabled_for(is_terminal),
                expected,
                "{variables:?}"
            );
        }
    }
}

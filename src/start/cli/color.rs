use std::io::{self, IsTerminal, Write};

use clap::builder::StyledStr;

use crate::{start::environment_variables::EnvironmentIter, syscall::exit::exit};

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

pub(super) unsafe fn print_and_exit(error: clap::Error, environment_pointer: *mut *mut u8) -> ! {
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

#[cfg(test)]
mod tests {
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

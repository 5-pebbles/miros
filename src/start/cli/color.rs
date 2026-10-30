use std::io::{self, IsTerminal, Write};

use crate::{start::environment_variables::EnvironmentIter, syscall::exit::exit};

#[derive(Default)]
struct ColorEnvironment {
    no_color: bool,
    clicolor_force: bool,
    terminal_supports_color: bool,
}

impl ColorEnvironment {
    fn from_variables<'a>(variables: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        variables
            .into_iter()
            .fold(Self::default(), |mut environment, (name, value)| {
                match name {
                    "NO_COLOR" => environment.no_color = !value.is_empty(),
                    "CLICOLOR_FORCE" => environment.clicolor_force = !value.is_empty(),
                    "TERM" => environment.terminal_supports_color = value != "dumb",
                    _ => {}
                }
                environment
            })
    }

    fn enabled_for(self, is_terminal: bool) -> bool {
        !self.no_color && (self.clicolor_force || (is_terminal && self.terminal_supports_color))
    }
}

pub(super) fn print_and_exit(error: clap::Error, environment_pointer: *mut *mut u8) -> ! {
    let (is_terminal, mut output): (bool, Box<dyn Write>) = if error.use_stderr() {
        (io::stderr().is_terminal(), Box::new(io::stderr()))
    } else {
        (io::stdout().is_terminal(), Box::new(io::stdout()))
    };

    let rendered = error.render();
    let colored = ColorEnvironment::from_variables(EnvironmentIter::new(environment_pointer))
        .enabled_for(is_terminal);
    let as_ascii = if colored {
        rendered.ansi().to_string()
    } else {
        rendered.to_string()
    };
    let _ = write!(output, "{as_ascii}");

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
            (&[("CLICOLOR_FORCE", "1")][..], false, true),
            (
                &[("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")][..],
                true,
                false,
            ),
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

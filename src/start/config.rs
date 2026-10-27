use std::sync::OnceLock;

use crate::{error::MirosError, start::environment_variables::EnvironmentIter};

const LENIENT_VARIABLE: &str = "MIROS_LENIENT_UNDEFINED_SYMBOLS";
const LAZY_BINDING_VARIABLE: &str = "MIROS_LAZY_BINDING";
const BIND_NOW_VARIABLE: &str = "LD_BIND_NOW";

#[derive(Clone, Copy)]
struct RuntimeConfig {
    lenient_undefined_symbols: bool,
    lazy_binding: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            lenient_undefined_symbols: false,
            lazy_binding: true,
        }
    }
}

static RUNTIME_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

/// Command-line overrides win over environment variables, which win over the built-in defaults.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConfigOverrides {
    pub lenient_undefined_symbols: Option<bool>,
    pub lazy_binding: Option<bool>,
}

fn current() -> RuntimeConfig {
    RUNTIME_CONFIG.get().copied().unwrap_or_default()
}

pub fn lenient_undefined_symbols() -> bool {
    current().lenient_undefined_symbols
}

pub fn lazy_binding() -> bool {
    current().lazy_binding
}

pub fn init_from_environment(env_pointer: *mut *mut u8, overrides: ConfigOverrides) {
    let config = resolve_config(EnvironmentIter::new(env_pointer), overrides);
    let _ = RUNTIME_CONFIG.set(config);
}

fn resolve_config<'a>(
    variables: impl Iterator<Item = (&'a str, &'a str)>,
    overrides: ConfigOverrides,
) -> RuntimeConfig {
    let mut config = RuntimeConfig::default();
    let mut bind_now = false;

    for (name, value) in variables {
        if name == BIND_NOW_VARIABLE {
            bind_now = !value.is_empty() && value != "0";
            continue;
        }

        let Some(setting) = setting_field(&mut config, name) else {
            continue;
        };

        match parse_setting(name, value) {
            Ok(set) => *setting = set,
            Err(error) => eprintln!("{error}"),
        }
    }

    if bind_now {
        config.lazy_binding = false;
    }

    if let Some(lenient) = overrides.lenient_undefined_symbols {
        config.lenient_undefined_symbols = lenient;
    }
    if let Some(lazy) = overrides.lazy_binding {
        config.lazy_binding = lazy;
    }

    config
}

fn parse_setting(name: &str, value: &str) -> Result<bool, MirosError> {
    match value {
        "1" => Ok(true),
        "0" => Ok(false),
        _ => Err(MirosError::UnrecognizedEnvironmentValue {
            name: name.to_owned(),
            value: value.to_owned(),
        }),
    }
}

fn setting_field<'a>(config: &'a mut RuntimeConfig, name: &str) -> Option<&'a mut bool> {
    match name {
        LENIENT_VARIABLE => Some(&mut config.lenient_undefined_symbols),
        LAZY_BINDING_VARIABLE => Some(&mut config.lazy_binding),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_config, ConfigOverrides};

    #[test]
    fn bind_now_disables_lazy_binding_independent_of_variable_order() {
        let forward = [("MIROS_LAZY_BINDING", "1"), ("LD_BIND_NOW", "1")];
        assert!(!resolve_config(forward.into_iter(), ConfigOverrides::default()).lazy_binding);

        let reverse = [("LD_BIND_NOW", "1"), ("MIROS_LAZY_BINDING", "1")];
        assert!(!resolve_config(reverse.into_iter(), ConfigOverrides::default()).lazy_binding);
    }

    #[test]
    fn lazy_binding_environment_and_command_line_override() {
        let disabled = resolve_config(
            [("MIROS_LAZY_BINDING", "0")].into_iter(),
            ConfigOverrides::default(),
        );
        assert!(!disabled.lazy_binding);

        let overridden =
            resolve_config([("MIROS_LAZY_BINDING", "0")].into_iter(), ConfigOverrides {
                lazy_binding: Some(true),
                ..Default::default()
            });
        assert!(overridden.lazy_binding);
    }

    #[test]
    fn bind_now_zero_keeps_lazy_binding() {
        let config = resolve_config(
            [("LD_BIND_NOW", "0")].into_iter(),
            ConfigOverrides::default(),
        );
        assert!(config.lazy_binding);
    }

    #[test]
    fn command_line_lazy_binding_overrides_bind_now() {
        let config = resolve_config([("LD_BIND_NOW", "1")].into_iter(), ConfigOverrides {
            lazy_binding: Some(true),
            ..Default::default()
        });
        assert!(config.lazy_binding);
    }
}

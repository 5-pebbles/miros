use std::sync::OnceLock;

use clap::ValueEnum;

use crate::{error::MirosError, start::environment_variables::EnvironmentIter};

const LENIENT_VARIABLE: &str = "MIROS_LENIENT_UNDEFINED_SYMBOLS";
const LAZY_BINDING_VARIABLE: &str = "MIROS_LAZY_BINDING";
const BIND_NOW_VARIABLE: &str = "LD_BIND_NOW";

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum UndefinedSymbolsPolicy {
    #[default]
    Strict,
    Lenient,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum BindingMode {
    #[default]
    Lazy,
    Now,
}

#[derive(Default)]
struct RuntimeConfig {
    undefined_symbols: UndefinedSymbolsPolicy,
    binding: BindingMode,
}

static RUNTIME_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

/// Command-line overrides win over environment variables, which win over the built-in defaults.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConfigOverrides {
    pub undefined_symbols: Option<UndefinedSymbolsPolicy>,
    pub binding: Option<BindingMode>,
}

pub fn lenient_undefined_symbols() -> bool {
    RUNTIME_CONFIG
        .get()
        .is_some_and(|config| config.undefined_symbols == UndefinedSymbolsPolicy::Lenient)
}

pub fn binding_mode() -> BindingMode {
    RUNTIME_CONFIG
        .get()
        .map_or(BindingMode::Lazy, |config| config.binding)
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

    for (name, value) in variables {
        if let Err(error) = apply_setting(&mut config, name, value) {
            eprintln!("{error}");
        }
    }

    if let Some(undefined_symbols) = overrides.undefined_symbols {
        config.undefined_symbols = undefined_symbols;
    }
    if let Some(binding) = overrides.binding {
        config.binding = binding;
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

fn apply_setting(config: &mut RuntimeConfig, name: &str, value: &str) -> Result<(), MirosError> {
    match name {
        LENIENT_VARIABLE => {
            config.undefined_symbols = if parse_setting(name, value)? {
                UndefinedSymbolsPolicy::Lenient
            } else {
                UndefinedSymbolsPolicy::Strict
            };
        }
        LAZY_BINDING_VARIABLE => {
            config.binding = if parse_setting(name, value)? {
                BindingMode::Lazy
            } else {
                BindingMode::Now
            };
        }
        BIND_NOW_VARIABLE if !value.is_empty() => config.binding = BindingMode::Now,
        _ => (),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BindingMode, ConfigOverrides, resolve_config};

    fn binding(variables: &[(&str, &str)]) -> BindingMode {
        resolve_config(variables.iter().copied(), ConfigOverrides::default()).binding
    }

    #[test]
    fn lazy_binding_is_the_default() {
        assert_eq!(binding(&[]), BindingMode::Lazy);
    }

    #[test]
    fn environment_controls_binding() {
        assert_eq!(binding(&[("MIROS_LAZY_BINDING", "0")]), BindingMode::Now);
        assert_eq!(binding(&[("MIROS_LAZY_BINDING", "1")]), BindingMode::Lazy);
        assert_eq!(binding(&[("LD_BIND_NOW", "0")]), BindingMode::Now);
        assert_eq!(binding(&[("LD_BIND_NOW", "")]), BindingMode::Lazy);
    }

    #[test]
    fn command_line_binding_overrides_the_environment() {
        let overrides = ConfigOverrides {
            binding: Some(BindingMode::Lazy),
            ..ConfigOverrides::default()
        };
        let variables = [("MIROS_LAZY_BINDING", "0"), ("LD_BIND_NOW", "1")];
        let config = resolve_config(variables.iter().copied(), overrides);
        assert_eq!(config.binding, BindingMode::Lazy);
    }
}

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

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuntimeConfig {
    pub binding: BindingMode,
    pub undefined_symbols: UndefinedSymbolsPolicy,
}

static RUNTIME_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

impl RuntimeConfig {
    pub fn get() -> Self {
        RUNTIME_CONFIG.get().copied().unwrap_or_default()
    }

    pub fn set(self) {
        let _ = RUNTIME_CONFIG.set(self);
    }

    pub fn from_environment(env_pointer: *mut *mut u8) -> Self {
        Self::resolve(EnvironmentIter::new(env_pointer))
    }

    fn resolve<'a>(variables: impl Iterator<Item = (&'a str, &'a str)>) -> Self {
        let mut config = Self::default();
        for (name, value) in variables {
            if let Err(error) = config.apply_setting(name, value) {
                eprintln!("{error}");
            }
        }
        config
    }

    fn apply_setting(&mut self, name: &str, value: &str) -> Result<(), MirosError> {
        match name {
            LENIENT_VARIABLE => {
                self.undefined_symbols = if parse_setting(name, value)? {
                    UndefinedSymbolsPolicy::Lenient
                } else {
                    UndefinedSymbolsPolicy::Strict
                };
            }
            LAZY_BINDING_VARIABLE => {
                self.binding = if parse_setting(name, value)? {
                    BindingMode::Lazy
                } else {
                    BindingMode::Now
                };
            }
            BIND_NOW_VARIABLE if !value.is_empty() => self.binding = BindingMode::Now,
            _ => (),
        }
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::{BindingMode, RuntimeConfig};

    fn binding(variables: &[(&str, &str)]) -> BindingMode {
        RuntimeConfig::resolve(variables.iter().copied()).binding
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
}

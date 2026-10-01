use std::sync::OnceLock;

use crate::{error::MirosError, start::environment_variables::EnvironmentIter};

const LENIENT_VARIABLE: &str = "MIROS_LENIENT_UNDEFINED_SYMBOLS";

#[derive(Default)]
struct RuntimeConfig {
    lenient_undefined_symbols: bool,
}

static RUNTIME_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

/// Command-line overrides win over `MIROS_*` environment variables, which win over the built-in defaults (all strict).
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConfigOverrides {
    pub lenient: Option<bool>,
}

pub fn lenient_undefined_symbols() -> bool {
    RUNTIME_CONFIG
        .get()
        .is_some_and(|config| config.lenient_undefined_symbols)
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
        let Some(setting) = setting_field(&mut config, name) else {
            continue;
        };

        match parse_setting(name, value) {
            Ok(set) => *setting = set,
            Err(error) => eprintln!("{error}"),
        }
    }

    if let Some(lenient) = overrides.lenient {
        config.lenient_undefined_symbols = lenient;
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
        _ => None,
    }
}

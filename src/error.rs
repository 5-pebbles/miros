use std::fmt;

use strum::{Display, EnumString};

use crate::{
    elf::dynamic_array::DynamicTag,
    start::{
        auxiliary_vector::AuxiliaryVectorType,
        config::{RuntimeConfig, UndefinedSymbolsPolicy},
    },
};

pub type Result<T> = std::result::Result<T, MirosError>;

#[derive(Display)]
pub enum ErrorLevel {
    Debug,
    Warn,
    Error,
}

#[derive(Debug)]
pub enum MirosError {
    MissingAuxvEntry(AuxiliaryVectorType),
    MissingDynamicEntry(DynamicTag),
    DependencyNotFound(String),
    ElfReadError(String),
    UnrecognizedEnvironmentValue { name: String, value: String },
    UndefinedSymbols(Vec<String>),
    MissingCpuFeature(MissingCpuFeature),
    SymbolIndexOutOfBounds(usize),
    TlsAllocationFailed,
}

impl MirosError {
    pub fn level(&self) -> ErrorLevel {
        match self {
            Self::UndefinedSymbols(_)
                if RuntimeConfig::get().undefined_symbols == UndefinedSymbolsPolicy::Lenient =>
            {
                ErrorLevel::Warn
            }
            Self::UnrecognizedEnvironmentValue { .. } => ErrorLevel::Warn,
            _ => ErrorLevel::Error,
        }
    }
}

impl fmt::Display for MirosError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level = self.level();
        write!(f, "Miros [{level}]: ")?;
        match self {
            Self::ElfReadError(message) => write!(f, "{message}"),
            Self::UnrecognizedEnvironmentValue { name, value } => {
                write!(f, "Ignoring unrecognized {name} value: {value}")
            }
            Self::UndefinedSymbols(names) => {
                let plural = (names.len() > 1).then_some("s").unwrap_or("");
                let symbols = names.join("`, `");
                write!(f, "Found Undefined Symbol{plural} [`{symbols}`]")
            }
            Self::MissingCpuFeature(feature) => write!(f, "Missing CPU Feature: `{feature}`"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(EnumString, Display, Debug)]
pub enum MissingCpuFeature {
    /// Pre Sandy Bridge, so ~2011-ish.
    XSave,
    /// This is probably the OS, not actually your CPU, but I don't really give a fuck.
    Xcr0SseBitNotSet,
}

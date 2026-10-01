use std::fmt;

use strum::Display;

use crate::{elf::dynamic_array::DynamicTag, start::auxiliary_vector::AuxiliaryVectorType};

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
    UnrecognizedOption(String),
    UnrecognizedEnvironmentValue { name: String, value: String },
    MissingExecutable,
    UndefinedSymbols(Vec<String>),
    SymbolIndexOutOfBounds(usize),
    TlsAllocationFailed,
}

impl MirosError {
    pub fn level(&self) -> ErrorLevel {
        match self {
            Self::UndefinedSymbols(_) if crate::start::config::lenient_undefined_symbols() => {
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
            Self::UnrecognizedOption(option) => write!(f, "unrecognized option: {option}"),
            Self::UnrecognizedEnvironmentValue { name, value } => {
                write!(f, "ignoring unrecognized {name} value: {value}")
            }
            Self::MissingExecutable => write!(f, "missing EXECUTABLE operand"),
            Self::UndefinedSymbols(names) => {
                let plural = (names.len() > 1).then_some("s").unwrap_or("");
                let symbols = names.join("`, `");
                write!(f, "Found Undefined Symbol{plural} [`{symbols}`]")
            }
            other => write!(f, "{other:?}"),
        }
    }
}

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
    UndefinedSymbols(Vec<String>),
    SymbolIndexOutOfBounds(usize),
    TlsAllocationFailed,
}

impl MirosError {
    pub fn level(&self) -> ErrorLevel {
        match self {
            Self::UndefinedSymbols(_) if cfg!(feature = "lenient-undefined-symbols") => {
                ErrorLevel::Warn
            }
            Self::UndefinedSymbols(_) => ErrorLevel::Error,
            _ => ErrorLevel::Error,
        }
    }
}

impl fmt::Display for MirosError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level = self.level();
        write!(f, "Miros [{level}]: ")?;
        match self {
            Self::UndefinedSymbols(names) => {
                let plural = (names.len() > 1).then_some("s").unwrap_or("");
                let symbols = names.join("`, `");
                write!(f, "Found Undefined Symbol{plural} [`{symbols}`]")
            }
            other => write!(f, "{other:?}"),
        }
    }
}

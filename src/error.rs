use crate::{elf::dynamic_array::DynamicTag, start::auxiliary_vector::AuxiliaryVectorType};

#[derive(Debug)]
pub enum MirosError {
    MissingAuxvEntry(AuxiliaryVectorType),
    MissingDynamicEntry(DynamicTag),
    DependencyNotFound(String),
}

use crate::start::auxiliary_vector::AuxiliaryVectorType;

#[derive(Debug)]
pub enum MirosError {
    MissingAuxvEntry(AuxiliaryVectorType),
}

use crate::{error::MirosError, objects::object_data_map::ObjectDataGraph};

pub mod init_array;
pub mod load_dependencies;
pub mod relocate;
pub mod thread_local_storage;

pub trait Stratagem {
    fn run(&self, object_data: &mut ObjectDataGraph) -> Result<(), MirosError>;
}

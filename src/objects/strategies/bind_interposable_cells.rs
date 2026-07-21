use crate::{
    error::MirosError,
    libc::interposable::INTERPOSABLE_CELLS,
    objects::{object_data_graph::ObjectDataGraph, strategies::Stratagem},
};

// Runs after Relocate so a COPY destination exists, before InitArray so user init sees the bound cells.
pub struct BindInterposableCells;

impl Stratagem for BindInterposableCells {
    fn run(&self, graph: &mut ObjectDataGraph) -> Result<(), MirosError> {
        INTERPOSABLE_CELLS
            .iter()
            .try_for_each(|cell| cell.bind(graph))
    }
}

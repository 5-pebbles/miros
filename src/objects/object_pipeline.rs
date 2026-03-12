use crate::{
    error::MirosError,
    objects::{object_data_graph::ObjectDataGraph, strategies::Stratagem},
};

pub struct ObjectPipeline<'a> {
    pub pipeline: &'a [&'a dyn Stratagem],
}

impl<'a> ObjectPipeline<'a> {
    pub fn new(stratagems: &'a [&'a dyn Stratagem]) -> Self {
        Self {
            pipeline: stratagems,
        }
    }

    pub fn run_pipeline(&self, object_data: &mut ObjectDataGraph) -> Result<(), MirosError> {
        self.pipeline
            .into_iter()
            .try_for_each(|stratagem| stratagem.run(object_data))
    }
}

use crate::{
    error::MirosError,
    objects::{object_data_map::ObjectDataMap, strategies::Stratagem},
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

    pub fn run_pipeline(&self, object_data: &mut ObjectDataMap) -> Result<(), MirosError> {
        self.pipeline
            .into_iter()
            .try_for_each(|stratagem| stratagem.run(object_data))
    }
}

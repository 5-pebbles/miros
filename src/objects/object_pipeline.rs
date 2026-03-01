use crate::{error::MirosError, objects::strategies::Stratagem};

pub struct ObjectPipeline<'a, T> {
    pub pipeline: &'a [&'a dyn Stratagem<T>],
}

impl<'a, T> ObjectPipeline<'a, T> {
    pub fn new(stratagems: &'a [&'a dyn Stratagem<T>]) -> Self {
        Self {
            pipeline: stratagems,
        }
    }

    pub fn run_pipeline(&self, object_data: &mut T) -> Result<(), MirosError> {
        self.pipeline
            .into_iter()
            .try_for_each(|stratagem| stratagem.run(object_data))
    }
}

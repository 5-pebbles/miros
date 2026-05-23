pub struct ThreadLocalAllocation {
    module_id: usize,
    block_offset: usize,
    // This enables lazy per-thread allocation when thread.generation < self.generation & modules are dlopen'd after threads are created
    generation: usize,
}

pub struct ModuleRegistry {}

impl ModuleRegistry {}

use crate::{tls::template::TlsTemplate, utils::mremap_allocator::MremapAllocator};

pub struct ModuleAllocation {
    pub block_offset: isize,
    pub template: TlsTemplate,
    pub generation: usize,
}

pub struct ModuleRegistry {
    modules: Vec<ModuleAllocation, MremapAllocator>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: Vec::new_in(MremapAllocator),
        }
    }

    pub fn push(&mut self, allocation: ModuleAllocation) {
        self.modules.push(allocation);
    }

    pub fn get(&self, module_id: usize) -> &ModuleAllocation {
        &self.modules[module_id]
    }

    pub fn since(&self, generation: usize) -> impl Iterator<Item = (usize, &ModuleAllocation)> {
        self.modules
            .iter()
            .enumerate()
            .filter(move |(_, module)| module.generation > generation)
    }

    pub fn count(&self) -> usize {
        self.modules.len()
    }
}

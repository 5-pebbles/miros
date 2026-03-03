use crate::elf::program_header::ProgramHeader;

pub struct ThreadLocalAllocation {
    block_id: usize,
    block_offset: usize,
    // This enables lazy per-thread allocation when thread.generation < self.generation & modules are dlopen'd after threads are created
    generation: usize,
}

impl ThreadLocalAllocation {
    pub fn new(block_id: usize, block_offset: usize) -> Self {
        // TODO: This should probably get and increment the global generation counter... someday, but not today.
        Self {
            block_id,
            block_offset,
            generation: 0,
        }
    }
}

pub struct ThreadLocalData {
    pub tls_program_header: ProgramHeader,
    pub thread_local_allocation: Option<ThreadLocalAllocation>,
}

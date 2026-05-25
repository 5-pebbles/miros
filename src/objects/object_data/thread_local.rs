use crate::elf::program_header::ProgramHeader;

pub struct ThreadLocalAllocation {
    pub module_id: usize,
    pub block_offset: isize,
}

impl ThreadLocalAllocation {
    pub fn new(module_id: usize, block_offset: isize) -> Self {
        Self {
            module_id,
            block_offset,
        }
    }
}

pub struct ThreadLocalData {
    pub tls_program_header: ProgramHeader,
    pub thread_local_allocation: Option<ThreadLocalAllocation>,
}

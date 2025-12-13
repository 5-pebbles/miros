use std::cmp::{max, min};

use crate::{
    elf::program_header::{ProgramHeader, PT_LOAD},
    page_size,
};

fn calculate_virtual_address_bounds(program_header_table: &[ProgramHeader]) -> (usize, usize) {
    let mut min_addr = usize::MAX;
    let mut max_addr = 0;

    for header in program_header_table {
        // Skip non-loadable segments
        if header.p_type != PT_LOAD {
            continue;
        }

        let start = header.p_vaddr as usize;
        let end = start + header.p_memsz as usize;

        min_addr = min(min_addr, start);
        max_addr = max(max_addr, end);
    }

    // Align bounds to page boundaries
    unsafe {
        (
            page_size::get_page_start(min_addr),
            page_size::get_page_end(max_addr),
        )
    }
}

pub struct Dependency {}

use std::mem::MaybeUninit;

use crate::{
    objects::strategies::init_array::InitArrayFunction,
    start::auxiliary_vector::{AuxiliaryVectorInfo, AuxiliaryVectorItem},
};

static mut PAGE_SIZE: MaybeUninit<usize> = MaybeUninit::uninit();

#[cfg_attr(not(test), link_section = ".preinit_array")]
#[used]
static INIT_PAGE_SIZE: InitArrayFunction = init_page_size;

extern "C" fn init_page_size(
    _arg_count: usize,
    _arg_pointer: *const *const u8,
    _env_pointer: *const *const u8,
    auxv_pointer: *const AuxiliaryVectorItem,
) {
    unsafe {
        let auxv_info = AuxiliaryVectorInfo::new(auxv_pointer).unwrap();
        #[allow(static_mut_refs)]
        PAGE_SIZE.write(auxv_info.page_size);
    }
}

pub fn get_page_size() -> usize {
    #[allow(static_mut_refs)]
    unsafe {
        PAGE_SIZE.assume_init_read()
    }
}

pub fn get_page_start(address: usize) -> usize {
    address & !(get_page_size() - 1)
}

pub fn get_page_offset(address: usize) -> usize {
    address & (get_page_size() - 1)
}

pub fn get_page_end(address: usize) -> usize {
    get_page_start(address + get_page_size() - 1)
}

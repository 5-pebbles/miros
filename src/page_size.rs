use std::mem::MaybeUninit;

static mut PAGE_SIZE: MaybeUninit<usize> = MaybeUninit::uninit();

pub unsafe fn set_page_size(page_size: usize) {
    #[allow(static_mut_refs)]
    PAGE_SIZE.write(page_size);
}

pub unsafe fn get_page_size() -> usize {
    #[allow(static_mut_refs)]
    PAGE_SIZE.assume_init_read()
}

pub unsafe fn get_page_start(address: usize) -> usize {
    address & !(get_page_size() - 1)
}

pub unsafe fn get_page_offset(address: usize) -> usize {
    address & (get_page_size() - 1)
}

pub unsafe fn get_page_end(address: usize) -> usize {
    get_page_start(address + get_page_size() - 1)
}

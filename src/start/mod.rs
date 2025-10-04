use auxiliary_vector::{
    AuxiliaryVectorIter, AT_BASE, AT_ENTRY, AT_PAGE_SIZE, AT_PHDR, AT_PHENT, AT_PHNUM, AT_RANDOM,
};
use environment_variables::EnvironmentIter;

use crate::{
    elf::program_header::ProgramHeader,
    io_macros::{syscall_assert, syscall_debug_assert},
    page_size,
    static_pie::StaticPie,
};
use std::{
    arch::naked_asm,
    fs::File,
    ptr::{null, null_mut},
    slice,
};

pub mod auxiliary_vector;
pub mod environment_variables;

#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!("mov rdi, rsp",
        "and rsp, -16", // !0b1111
        "call {}",
        "mov rdx, 0",
        "jmp rax",
        sym relocate_and_calculate_jump_address,
    );
}

pub unsafe extern "C" fn relocate_and_calculate_jump_address(stack_pointer: *mut usize) -> usize {
    // Inital Stack Layout:
    // + Newly Pushed Vaules      Examples:               |-----------------|
    // |-------------------|    |----------------|  |---> | "/bin/git", 0x0 |
    // | Arg Count         |    | 2              |  |     |-----------------|
    // |-------------------|    |----------------|  |
    // | Arg Pointers...   |    | Pointer,       | -|   |---------------|
    // |                   |    | Other Pointer  | ---> | "commit", 0x0 |
    // |-------------------|    |----------------|      |---------------|
    // | Null              |    | 0x0            |
    // |-------------------|    |----------------|       |-----------------------------|
    // | Env Pointers...   |    | Pointer,       | ----> | "HOME=/home/ghostbird", 0x0 |
    // |                   |    | Other Pointer  | ---|  |-----------------------------|
    // |-------------------|    |----------------|    |
    // | Null              |    | 0x0            |    |   |---------------------------|
    // |-------------------|    |----------------|    |-> | "PATH=/bin:/usr/bin", 0x0 |
    // | Auxv Type...      |    | AT_RANDOM      |        |---------------------------|
    // | Auxv Vaule...     |    | Union->Pointer | -|
    // |-------------------|    |----------------|  |   |---------------------------|
    // | AT_NULL Auxv Pair |    | AT_NULL (0x0)  |  |-> | [16-bytes of random data] |
    // |-------------------|    | Undefined      |      |---------------------------|
    //                          |----------------|

    // Check that `stack_pointer` is where we expect it to be.
    syscall_debug_assert!(stack_pointer != core::ptr::null_mut());
    syscall_debug_assert!(stack_pointer.addr() & 0b1111 == 0);

    let argument_count = *stack_pointer as usize;
    let argument_pointer = stack_pointer.add(1) as *mut *mut u8;
    syscall_debug_assert!((*argument_pointer.add(argument_count)).is_null());

    let environment_vector = EnvironmentIter::from_stack_pointer(stack_pointer);
    let auxiliary_vector = AuxiliaryVectorIter::from_environment_iter(environment_vector);

    // Auxilary Vector:
    let (mut base, mut entry, mut page_size) = (null(), null(), 0);
    let mut pseudorandom_bytes: *const [u8; 16] = null_mut();
    // NOTE: The program headers in the auxiliary vector belong to the executable, not us.
    let (mut program_header_pointer, mut program_header_count) = (null(), 0);
    for value in auxiliary_vector {
        match value.a_type {
            AT_BASE => base = value.a_un.a_ptr,
            AT_ENTRY => entry = value.a_un.a_ptr,
            AT_PAGE_SIZE => page_size = value.a_un.a_val,
            AT_RANDOM => pseudorandom_bytes = value.a_un.a_ptr as *const [u8; 16],
            // Executable Stuff:
            AT_PHDR => program_header_pointer = value.a_un.a_ptr as *const ProgramHeader,
            AT_PHNUM => program_header_count = value.a_un.a_val,
            #[cfg(debug_assertions)]
            AT_PHENT => syscall_assert!(value.a_un.a_val == size_of::<ProgramHeader>()),
            _ => (),
        }
    }
    syscall_debug_assert!(page_size.is_power_of_two());
    syscall_debug_assert!(base.addr() & (page_size - 1) == 0);
    page_size::set_page_size(page_size);

    let program_header_table =
        slice::from_raw_parts(program_header_pointer, program_header_count as usize);

    // We are a static pie (position-independent-executable).
    // Relocate ourselves and initialize thread local storage:
    let miros = if base.is_null() {
        StaticPie::from_program_headers(&program_header_table)
    } else {
        StaticPie::from_base(base)
    };
    miros.relocate().allocate_tls(&*pseudorandom_bytes);
    // NOTE: We can now use the Rust standard library.

    /// The execuatable we are linking for:
    let base_object = if base == null() {
        // TODO: Cli
        let _ = File::open("/home/ghostbird/git/miros/README.md");

        crate::syscall::exit::exit(0);
    } else {
        // SharedObject::from_headers(program_header_table, pseudorandom_bytes);
        let _ = File::open("/home/ghostbird/git/miros/README.md");
        crate::syscall::exit::exit(1);
    };

    // entry.addr()
}

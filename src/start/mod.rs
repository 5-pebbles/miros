use crate::{
    io_macros::syscall_debug_assert,
    libc::environ::set_environ_pointer,
    objects::{
        object_data::{NonDynamic, ObjectData},
        object_pipeline::ObjectPipeline,
        strategies::{
            init_array::InitArray, relocate::Relocate, thread_local_storage::ThreadLocalStorage,
            ObjectDataSingle, Stratagem,
        },
    },
    page_size,
    start::auxiliary_vector::{AuxiliaryVectorInfo, AuxiliaryVectorItem},
};
use std::{
    arch::naked_asm,
    env,
    ptr::{null, null_mut},
    slice,
};

pub mod auxiliary_vector;
pub mod environment_variables;

#[unsafe(naked)]
#[unsafe(no_mangle)]
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
    // + Newly Pushed Values      Example:                ┌-----------------┐
    // ┌-------------------┐    ┌----------------┐  ┌---> | "/bin/git", 0x0 |
    // | Arg Count         |    | 2              |  |     └-----------------┘
    // |-------------------|    |----------------|  |
    // | Arg Pointers...   |    | Pointer,       | -┘   ┌---------------┐
    // |                   |    | Other Pointer  | ---> | "commit", 0x0 |
    // |-------------------|    |----------------|      └---------------┘
    // | Null              |    | 0x0            |
    // |-------------------|    |----------------|       ┌-----------------------------┐
    // | Env Pointers...   |    | Pointer,       | ----> | "HOME=/home/ghostbird", 0x0 |
    // |                   |    | Other Pointer  | ---┐  └-----------------------------┘
    // |-------------------|    |----------------|    |
    // | Null              |    | 0x0            |    |   ┌---------------------------┐
    // |-------------------|    |----------------|    └-> | "PATH=/bin:/usr/bin", 0x0 |
    // | Auxv Type...      |    | AT_RANDOM      |        └---------------------------┘
    // | Auxv Value...     |    | Union->Pointer | -┐
    // |-------------------|    |----------------|  |   ┌---------------------------┐
    // | AT_NULL Auxv Pair |    | AT_NULL (0x0)  |  └-> | [16-bytes of random data] |
    // └-------------------┘    | Undefined      |      └---------------------------┘
    //                          └----------------┘

    // Check that `stack_pointer` is where (and what) we expect it to be.
    debug_assert_ne!(stack_pointer, null_mut());
    debug_assert_eq!(stack_pointer.addr() & 0b1111, 0); // 16-bit aligned

    let arg_count = *stack_pointer;
    let arg_pointer = stack_pointer.add(1).cast::<*const u8>();

    debug_assert_eq!((*arg_pointer.add(arg_count)), null()); // args are null-terminated

    let env_pointer = arg_pointer.add(arg_count + 1);

    // Find the end of the environment variables + null-terminator + 1
    // Auxilary Vector:
    let auxv_pointer = (0..)
        .map(|i| env_pointer.add(i))
        .find(|&ptr| (*ptr).is_null())
        .unwrap_unchecked() // SAFETY: I mean, it's an infinite loop, then segfaults before it's None...
        .add(1)
        .cast::<AuxiliaryVectorItem>();

    let auxv_info = AuxiliaryVectorInfo::new(auxv_pointer).unwrap();
    syscall_debug_assert!(auxv_info.page_size.is_power_of_two());
    syscall_debug_assert!(auxv_info.base.addr() & (auxv_info.page_size - 1) == 0);
    page_size::set_page_size(auxv_info.page_size);

    let program_header_table = slice::from_raw_parts(
        auxv_info.program_header_pointer,
        auxv_info.program_header_count,
    );

    // Relocate ourselves and initialize thread local storage:
    let miros = if auxv_info.base.is_null() {
        ObjectData::<NonDynamic>::from_program_headers(&program_header_table)
    } else {
        ObjectData::from_base(auxv_info.base)
    };

    let relocate = Relocate::new();
    let thread_local_storage =
        ThreadLocalStorage::new(auxv_info.pseudorandom_bytes.as_ref().unwrap_unchecked());
    let init_array = InitArray::new(arg_count, arg_pointer, env_pointer, auxv_pointer);

    let stratagems: &[&dyn Stratagem<ObjectDataSingle>] =
        &[&relocate, &thread_local_storage, &init_array];

    let pipeline = ObjectPipeline::new(stratagems);
    let _ = pipeline.run_pipeline(miros);

    println!("test");

    set_environ_pointer(env_pointer as *mut *mut u8);

    println!("{:?}", env::vars());

    crate::syscall::exit::exit(0);
}

use std::{
    arch::naked_asm,
    env,
    ptr::{self, null, null_mut},
};

use crate::{
    io_macros::syscall_debug_assert,
    libc::environ::set_environ_pointer,
    objects::{
        object_data::ObjectData,
        object_data_graph::ObjectDataGraph,
        object_pipeline::ObjectPipeline,
        strategies::{
            init_array::InitArray, load_dependencies::LoadDependencies, relocate::Relocate,
            Stratagem,
        },
    },
    page_size,
    start::{
        auxiliary_vector::{AuxiliaryVectorInfo, AuxiliaryVectorItem},
        miros::Miros,
    },
};

pub mod auxiliary_vector;
pub mod environment_variables;
pub mod miros;

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

    let program_header_table = ptr::slice_from_raw_parts(
        auxv_info.program_header_pointer,
        auxv_info.program_header_count,
    );

    // Relocate ourselves, initialize TLS, and call init functions:
    let miros = if auxv_info.base.is_null() {
        Miros::from_program_headers(program_header_table).unwrap()
    } else {
        Miros::from_base(auxv_info.base).unwrap()
    };

    miros
        .relocate()
        .allocate_tls(auxv_info.pseudorandom_bytes)
        .init_array(arg_count, arg_pointer, env_pointer, auxv_pointer);

    println!("test");

    set_environ_pointer(env_pointer as *mut *mut u8);

    println!("{:?}", env::vars());

    let miros_object_data = if auxv_info.base.is_null() {
        ObjectData::from_program_headers(program_header_table).unwrap()
    } else {
        ObjectData::from_base(auxv_info.base).unwrap()
    };

    let executable = if auxv_info.base.is_null() {
        todo!()
    } else {
        ObjectData::from_program_headers(program_header_table).unwrap()
    };
    let mut executable_and_dependencies = ObjectDataGraph::new(executable, miros_object_data);

    let load_dependencies = LoadDependencies::new();
    let relocate = Relocate::new();
    let init_array = InitArray::new(arg_count, arg_pointer, env_pointer, auxv_pointer);
    let executable_stratagems: &[&dyn Stratagem] = &[&load_dependencies, &relocate, &init_array];
    let executable_pipeline = ObjectPipeline::new(executable_stratagems);
    let _ = executable_pipeline.run_pipeline(&mut executable_and_dependencies);

    auxv_info.entry.addr()
}

use std::{
    arch::naked_asm,
    ffi::{CStr, OsStr},
    fs::File,
    os::unix::ffi::OsStrExt,
    ptr::{self, null, null_mut},
    slice,
};

use crate::{
    elf::{header::ElfHeader, program_header::ProgramHeader},
    io_macros::syscall_debug_assert,
    libc::{environ::set_environ_pointer, program_name::set_program_name},
    objects::{
        object_data::ObjectData,
        object_data_graph::ObjectDataGraph,
        object_pipeline::ObjectPipeline,
        strategies::{
            bind_interposable_cells::BindInterposableCells, init_array::InitArray,
            load_dependencies::LoadDependencies, relocate::Relocate,
            thread_local_storage::ThreadLocalStorage, Stratagem,
        },
    },
    start::{
        arguments::{CompactedStack, Invocation},
        auxiliary_vector::{AuxiliaryVectorInfo, AuxiliaryVectorItem},
        bootstrap::Bootstrap,
        config::ConfigOverrides,
    },
};

pub mod arguments;
pub mod auxiliary_vector;
pub mod bootstrap;
pub mod config;
pub mod environment_variables;

#[unsafe(naked)]
#[cfg_attr(not(test), no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
    extern "C" {
        fn rtld_fini();
    }
    naked_asm!("mov rdi, rsp",
        "and rsp, -16", // !0b1111
        "call {}",
        "lea rdx, [rip + {}]",
        "jmp rax",
        sym relocate_and_calculate_jump_address,
        sym rtld_fini,
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

    debug_assert_ne!(stack_pointer, null_mut());
    debug_assert_eq!(stack_pointer.addr() & 0b1111, 0); // 16-byte aligned

    let mut arg_count = *stack_pointer;
    let arg_pointer = stack_pointer.add(1).cast::<*const u8>();

    debug_assert_eq!((*arg_pointer.add(arg_count)), null());

    let mut env_pointer = arg_pointer.add(arg_count + 1);

    let mut auxv_pointer = (0..)
        .map(|index| env_pointer.add(index))
        .find(|pointer| (**pointer).is_null())
        // SAFETY: the env array is null-terminated, so the find always succeeds before running off the stack.
        .unwrap_unchecked()
        .add(1)
        .cast::<AuxiliaryVectorItem>();

    let auxv_info = AuxiliaryVectorInfo::new(auxv_pointer).unwrap();

    // No AT_BASE: the kernel exec'd miros itself, so miros is the main executable and argv is its own.
    let direct_invocation = auxv_info.base.is_null();

    syscall_debug_assert!(auxv_info.page_size.is_power_of_two());
    syscall_debug_assert!(auxv_info.base.addr() & (auxv_info.page_size - 1) == 0);

    let program_header_table = ptr::slice_from_raw_parts(
        auxv_info.program_header_pointer,
        auxv_info.program_header_count,
    );

    let bootstrap = if direct_invocation {
        Bootstrap::from_program_headers(program_header_table).unwrap()
    } else {
        Bootstrap::from_base(auxv_info.base).unwrap()
    };

    // Relocation must precede everything below: panics, vtables, and TLS access all assume the GOT is patched.
    let bootstrap = bootstrap.relocate();
    crate::page_size::set_page_size(auxv_info.page_size);
    bootstrap
        .allocate_tls(auxv_info.pseudorandom_bytes)
        .init_array(arg_count, arg_pointer, env_pointer, auxv_pointer);
    crate::allocator::install_heap();

    let overrides = if direct_invocation {
        let (executable_index, overrides) =
            Invocation::parse_or_exit(slice::from_raw_parts(arg_pointer, arg_count));

        let compacted =
            CompactedStack::compact(stack_pointer, env_pointer, auxv_pointer, executable_index);
        arg_count = compacted.arg_count;
        env_pointer = compacted.env_pointer;
        auxv_pointer = compacted.auxv_pointer;
        overrides
    } else {
        ConfigOverrides::default()
    };

    auxiliary_vector::set_auxiliary_vector(auxv_pointer);
    set_environ_pointer(env_pointer as *mut *mut u8);
    set_program_name(arg_pointer.read());
    config::init_from_environment(env_pointer as *mut *mut u8, overrides);

    let miros_object_data = if direct_invocation {
        ObjectData::from_program_headers(program_header_table).unwrap()
    } else {
        ObjectData::from_base(auxv_info.base).unwrap()
    };

    let (executable, entry_point) = if direct_invocation {
        load_direct_executable(arg_pointer, auxv_pointer)
    } else {
        (
            ObjectData::from_program_headers(program_header_table).unwrap(),
            auxv_info.entry.addr(),
        )
    };
    let mut executable_and_dependencies = ObjectDataGraph::new(executable, miros_object_data);

    let init_array = InitArray::new(arg_count, arg_pointer, env_pointer, auxv_pointer);
    let stratagems: &[&dyn Stratagem] = &[
        &LoadDependencies,
        &Relocate,
        &BindInterposableCells,
        &ThreadLocalStorage,
        &init_array,
    ];
    let executable_pipeline = ObjectPipeline::new(stratagems);
    if let Err(error) = executable_pipeline.run_pipeline(&mut executable_and_dependencies) {
        eprintln!("{error}");
        crate::syscall::exit::exit(1);
    }

    entry_point
}

/// Loads the executable named by the (already compacted) argv[0] and retargets the auxv at it.
unsafe fn load_direct_executable(
    arg_pointer: *const *const u8,
    auxv_pointer: *mut AuxiliaryVectorItem,
) -> (ObjectData, usize) {
    let path = CStr::from_ptr((*arg_pointer).cast());
    let file = File::open(OsStr::from_bytes(path.to_bytes())).unwrap_or_else(|error| {
        eprintln!("miros: {}: {error}", path.to_string_lossy());
        crate::syscall::exit::exit(1);
    });
    let executable = ObjectData::from_file(file).unwrap_or_else(|error| {
        eprintln!("{error}: {}", path.to_string_lossy());
        crate::syscall::exit::exit(1);
    });

    // from_file validated a PT_LOAD at file offset 0 / vaddr 0, so the ELF header sits at `base`.
    let header = &*(executable.base as *const ElfHeader);
    let entry = executable.base.byte_add(header.e_entry);
    auxiliary_vector::retarget_executable(
        auxv_pointer,
        executable.base.byte_add(header.e_phoff) as *const ProgramHeader,
        header.e_phnum as usize,
        entry,
        path.as_ptr().cast(),
    );
    (executable, entry.addr())
}

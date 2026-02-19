use std::ffi::c_void;
use std::slice;
use std::{cmp::max, ptr::null_mut};

use crate::syscall::thread_pointer::set_thread_pointer;
use crate::{
    elf::thread_local_storage::ThreadControlBlock,
    io_macros::syscall_debug_assert,
    libc::mem::{mmap, MapFlags, ProtectionFlags},
    objects::{
        object_data::{AnyDynamic, ObjectData},
        strategies::ObjectStratagem,
    },
    utils::round_up_to_boundary,
};

pub struct StaticTLS<'a> {
    pseudorandom_bytes: &'a [u8; 16],
}

impl<'a, T: AnyDynamic> ObjectStratagem<T> for StaticTLS<'a> {
    fn execute(self, mut object_data: impl Iterator<Item = ObjectData<T>>) {
        // Static Thread Local Storage [before Thread Pointer]:
        //                                         ┌---------------------┐
        //      ┌----------------------------┐ <-- |    tls-offset[1]    |
        //      |      Static TLS Block      |     |---------------------|
        //      |----------------------------| <-- | Thread Pointer (TP) |
        // ┌--- | Thread Control Block (TCB) |     └---------------------┘
        // |    └----------------------------┘
        // |
        // |   ┌------------------┐
        // └-> | Null Dtv Pointer |
        //     └------------------┘
        // NOTE: I am not bothering with alignment at the first address because it's already page aligned...
        let single_object_data = object_data.next().unwrap();
        syscall_debug_assert!(object_data.next().is_none());

        let Some(tls_program_header) = single_object_data.tls_program_header else {
            return;
        };

        let max_required_align = max(align_of::<ThreadControlBlock>(), tls_program_header.p_align);
        let tls_blocks_size_and_align =
            round_up_to_boundary(tls_program_header.p_memsz, tls_program_header.p_align);
        let tcb_size_and_align = size_of::<ThreadControlBlock>() + max_required_align;

        let required_size = tls_blocks_size_and_align + tcb_size_and_align;

        let protection_flags = ProtectionFlags::ZERO
            .with_readable(true)
            .with_writable(true);

        let map_flags = MapFlags::ZERO.with_private(true).with_anonymous(true);

        unsafe {
            let tls_block_pointer = mmap(
                null_mut(),
                required_size,
                protection_flags,
                map_flags,
                -1, // file descriptor (-1 for anonymous mapping)
                0,  // offset
            );
            syscall_debug_assert!(tls_block_pointer.addr() % max_required_align == 0);

            // Initialize the TLS data from template image
            slice::from_raw_parts_mut(tls_block_pointer as *mut u8, tls_program_header.p_filesz)
                .copy_from_slice(slice::from_raw_parts(
                    single_object_data
                        .base
                        .byte_add(tls_program_header.p_offset) as *mut u8,
                    tls_program_header.p_filesz,
                ));

            // Zero out TLS data beyond `p_filesz`
            slice::from_raw_parts_mut(
                tls_block_pointer.byte_add(tls_program_header.p_filesz) as *mut u8,
                tls_program_header.p_memsz - tls_program_header.p_filesz,
            )
            .fill(0);

            // Initialize the Thread Control Block (TCB)
            let thread_control_block =
                tls_block_pointer.byte_add(tls_blocks_size_and_align) as *mut ThreadControlBlock;

            let thread_pointer_register: *mut c_void =
                (*thread_control_block).thread_pointee.as_mut_ptr().cast();

            *thread_control_block = ThreadControlBlock {
                thread_pointee: [],
                thread_pointer_register,
                dynamic_thread_vector: null_mut(),
                _padding: [0; 3],
                canary: usize::from_ne_bytes(
                    (*self.pseudorandom_bytes)[..size_of::<usize>()]
                        .try_into()
                        .unwrap(),
                ),
            };

            // Make the thread pointer (which is fs on x86_64) point to the new TCB
            set_thread_pointer(thread_pointer_register);
        }
    }
}

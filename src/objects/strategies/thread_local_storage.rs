pub struct ThreadLocalStorage {
    pseudorandom_bytes: *const [u8; 16],
}

impl ThreadLocalStorage {
    pub fn new(pseudorandom_bytes: *const [u8; 16]) -> Self {
        Self { pseudorandom_bytes }
    }
}

// impl Stratagem<ObjectData> for ThreadLocalStorage {
//     fn run(&self, object_data: &mut ObjectData) -> Result<(), MirosError> {
//         unsafe {
//             // Static Thread Local Storage [before Thread Pointer]:
//             //                                         ┌---------------------┐
//             //      ┌----------------------------┐ <-- |    tls-offset[1]    |
//             //      |      Static TLS Block      |     |---------------------|
//             //      |----------------------------| <-- | Thread Pointer (TP) |
//             // ┌--- | Thread Control Block (TCB) |     └---------------------┘
//             // |    └----------------------------┘
//             // |
//             // |   ┌------------------┐
//             // └-> | Null Dtv Pointer |
//             //     └------------------┘
//             // NOTE: I am not bothering with alignment at the first address because it's already page aligned...
//             let Some(tls_data) = object_data.tls_data.as_mut() else {
//                 return Ok(());
//             };

//             let max_required_align = max(
//                 align_of::<ThreadControlBlock>(),
//                 tls_data.tls_program_header.p_align,
//             );
//             let tls_blocks_size_and_align = round_up_to_boundary(
//                 tls_data.tls_program_header.p_memsz,
//                 tls_data.tls_program_header.p_align,
//             );
//             let tcb_size_and_align = size_of::<ThreadControlBlock>() + max_required_align;

//             let required_size = tls_blocks_size_and_align + tcb_size_and_align;

//             let protection_flags = ProtectionFlags::ZERO
//                 .with_readable(true)
//                 .with_writable(true);

//             let map_flags = MapFlags::ZERO.with_private(true).with_anonymous(true);

//             let tls_block_pointer = mmap(
//                 null_mut(),
//                 required_size,
//                 protection_flags,
//                 map_flags,
//                 -1, // file descriptor (-1 for anonymous mapping)
//                 0,  // offset
//             );
//             assert_eq!(tls_block_pointer.addr() % max_required_align, 0);

//             // Initialize the TLS data from template image
//             slice::from_raw_parts_mut(
//                 tls_block_pointer as *mut u8,
//                 tls_data.tls_program_header.p_filesz,
//             )
//             .copy_from_slice(slice::from_raw_parts(
//                 object_data
//                     .base
//                     .byte_add(tls_data.tls_program_header.p_offset) as *mut u8,
//                 tls_data.tls_program_header.p_filesz,
//             ));

//             // Zero out TLS data beyond `p_filesz`
//             slice::from_raw_parts_mut(
//                 tls_block_pointer.byte_add(tls_data.tls_program_header.p_filesz) as *mut u8,
//                 tls_data.tls_program_header.p_memsz - tls_data.tls_program_header.p_filesz,
//             )
//             .fill(0);

//             // Initialize the Thread Control Block (TCB)
//             let thread_control_block =
//                 tls_block_pointer.byte_add(tls_blocks_size_and_align) as *mut ThreadControlBlock;

//             let thread_pointer_register: *mut c_void =
//                 (*thread_control_block).thread_pointee.as_mut_ptr().cast();

//             *thread_control_block = ThreadControlBlock {
//                 thread_pointee: [],
//                 thread_pointer_register,
//                 dynamic_thread_vector: null_mut(),
//                 _padding: [0; 3],
//                 canary: usize::from_ne_bytes(
//                     (&*self.pseudorandom_bytes)[..size_of::<usize>()]
//                         .try_into()
//                         .unwrap(),
//                 ),
//             };

//             // Make the thread pointer (which is fs on x86_64) point to the new TCB
//             set_thread_pointer(thread_pointer_register);

//             // Update the ObjectData
//             let block_id = 1;
//             let block_offset = 0;
//             tls_data.thread_local_allocation =
//                 Some(ThreadLocalAllocation::new(block_id, block_offset));

//             Ok(())
//         }
//     }
// }

// impl Stratagem<Dynamic> for ThreadLocalStorage {
//     fn execute(
//         self,
//         object_data: impl Iterator<Item = ObjectData<Dynamic>>,
//     ) -> Result<impl Iterator<Item = ObjectData<Dynamic>>, MirosError> {
//         todo!()
//     }
// }

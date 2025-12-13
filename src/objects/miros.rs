use crate::{
    elf::{
        header::{ElfHeader, ET_DYN},
        program_header::{ProgramHeader, PT_DYNAMIC, PT_PHDR, PT_TLS},
        thread_local_storage::ThreadControlBlock,
    },
    io_macros::syscall_debug_assert,
    libc::mem::{mmap, MapFlags, ProtectionFlags},
    objects::{
        object_base::{NonDynamic, ObjectBase},
        relocate::RelaRelocatable,
    },
    syscall::thread_pointer::set_thread_pointer,
    utils::round_up_to_boundary,
};
use std::{
    cmp::max,
    ffi::c_void,
    marker::PhantomData,
    ptr::{null, null_mut},
    slice,
};

pub type InitArrayFunction = extern "C" fn(usize, *const *const u8, *const *const u8);

pub struct Relocate;
pub struct AllocateTLS;
pub struct InitArray;

pub struct Miros<T> {
    object_base: ObjectBase<NonDynamic>,
    phantom_data: PhantomData<T>,
}

impl Miros<Relocate> {
    #[inline(always)]
    pub unsafe fn from_base(base: *const c_void) -> Miros<Relocate> {
        // ELf Header:
        let header = &*(base as *const ElfHeader);
        syscall_debug_assert!(header.e_type == ET_DYN);
        syscall_debug_assert!(header.e_phentsize == size_of::<ProgramHeader>() as u16);

        // Program Headers:
        let program_header_table = slice::from_raw_parts(
            base.byte_add(header.e_phoff) as *const ProgramHeader,
            header.e_phnum as usize,
        );

        let (mut dynamic_program_header, mut tls_program_header) = (null(), null());
        for header in program_header_table {
            match header.p_type {
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = header,
                _ => (),
            }
        }

        let object_base = ObjectBase::build(base, dynamic_program_header, tls_program_header);

        Self {
            object_base,
            phantom_data: PhantomData,
        }
    }

    #[inline(always)]
    pub unsafe fn from_program_headers(
        program_header_table: &'static [ProgramHeader],
    ) -> Miros<Relocate> {
        let (mut base, mut dynamic_program_header, mut tls_program_header) =
            (null(), null(), null());
        for header in program_header_table {
            match header.p_type {
                PT_PHDR => {
                    base = program_header_table.as_ptr().byte_sub(header.p_vaddr) as *const c_void;
                }
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = header,
                _ => (),
            }
        }

        let object_base = ObjectBase::build(base, dynamic_program_header, tls_program_header);

        Self {
            object_base,
            phantom_data: PhantomData,
        }
    }
}

impl RelaRelocatable for Miros<Relocate> {
    fn base(&self) -> Result<*const c_void, Self::RelaError> {
        Ok(self.object_base.base)
    }
}

impl Miros<Relocate> {
    #[must_use]
    pub fn relocate(self) -> Miros<AllocateTLS> {
        unsafe {
            use crate::elf::relocate::{R_X86_64_IRELATIVE, R_X86_64_RELATIVE};
            self.rela_relocate(
                self.object_base
                    .rela_slice
                    .into_iter()
                    .filter(|rela| matches!(rela.r_type(), R_X86_64_RELATIVE | R_X86_64_IRELATIVE)),
            )
            .unwrap();
        }

        Miros::<AllocateTLS> {
            phantom_data: PhantomData,
            ..self
        }
    }
}

impl Miros<AllocateTLS> {
    #[must_use]
    pub unsafe fn allocate_tls(self, pseudorandom_bytes: &[u8; 16]) -> Miros<InitArray> {
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
        if self.object_base.tls_program_header.is_null() {
            return Miros::<InitArray> {
                phantom_data: PhantomData,
                ..self
            };
        }
        let tls_program_header = *self.object_base.tls_program_header;

        let max_required_align = max(align_of::<ThreadControlBlock>(), tls_program_header.p_align);
        let tls_blocks_size_and_align =
            round_up_to_boundary(tls_program_header.p_memsz, tls_program_header.p_align);
        let tcb_size_and_align = size_of::<ThreadControlBlock>() + max_required_align;

        let required_size = tls_blocks_size_and_align + tcb_size_and_align;

        let protection_flags = ProtectionFlags::ZERO
            .with_readable(true)
            .with_writable(true);

        let map_flags = MapFlags::ZERO.with_private(true).with_anonymous(true);

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
                self.object_base.base.byte_add(tls_program_header.p_offset) as *mut u8,
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
                (*pseudorandom_bytes)[..size_of::<usize>()]
                    .try_into()
                    .unwrap(),
            ),
        };

        // Make the thread pointer (which is fs on x86_64) point to the new TCB
        set_thread_pointer(thread_pointer_register);

        Miros::<InitArray> {
            phantom_data: PhantomData,
            ..self
        }
    }
}

impl Miros<InitArray> {
    pub unsafe fn init_array(
        self,
        arg_count: usize,
        arg_pointer: *const *const u8,
        env_pointer: *const *const u8,
    ) {
        if let Some(init_functions) = self.object_base.init_array {
            // Call each initialization function in order
            init_functions
                .iter()
                .filter(|init_fn| **init_fn as *const c_void != null())
                .for_each(|init_fn| init_fn(arg_count, arg_pointer, env_pointer));
        }
    }
}

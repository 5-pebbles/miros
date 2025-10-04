use std::{
    arch::asm,
    marker::PhantomData,
    ptr::{null, null_mut},
    slice,
};

use crate::{
    elf::{
        dynamic_array::{DynamicArrayItem, DynamicArrayIter, DT_RELA, DT_RELAENT, DT_RELASZ},
        header::{ElfHeader, ET_DYN},
        program_header::{ProgramHeader, PT_DYNAMIC, PT_PHDR, PT_TLS},
        relocate::{Rela, RelocationSlices},
        thread_local_storage::ThreadControlBlock,
    },
    io_macros::syscall_debug_assert,
    page_size,
    syscall::{
        mmap::{mmap, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE},
        thread_pointer::set_thread_pointer,
        write,
    },
    utils::round_up_to_boundary,
};

pub struct Ingredients;
pub struct Baked;

/// A struct representing a statically relocatable Position Independent Executable (PIE). 🥧
///
/// WARN: This struct is used before the relocations are preformed, if its size exceeds 8 bytes the compiler will call `memcpy` and cause a segfault.
pub struct StaticPie<T> {
    base_address: *const (),
    dynamic_array: *const DynamicArrayItem,
    tls_program_header: *const ProgramHeader,
    phantom_data: PhantomData<T>,
}

impl StaticPie<Ingredients> {
    #[inline(always)]
    pub unsafe fn from_base(base: *const ()) -> StaticPie<Ingredients> {
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

        Self::build(base, dynamic_program_header, tls_program_header)
    }

    #[inline(always)]
    pub unsafe fn from_program_headers(
        program_header_table: &'static [ProgramHeader],
    ) -> StaticPie<Ingredients> {
        let (mut base, mut dynamic_program_header, mut tls_program_header) =
            (null(), null(), null());
        for header in program_header_table {
            match header.p_type {
                PT_PHDR => {
                    base = program_header_table.as_ptr().byte_sub(header.p_vaddr) as *const ();
                }
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = header,
                _ => (),
            }
        }

        Self::build(base, dynamic_program_header, tls_program_header)
    }

    #[inline(always)]
    #[must_use]
    unsafe fn build(
        base: *const (),
        dynamic_program_header: *const ProgramHeader,
        tls_program_header: *const ProgramHeader,
    ) -> StaticPie<Ingredients> {
        syscall_debug_assert!(dynamic_program_header != null());

        // Dynamic Arrary:
        let dynamic_array =
            base.byte_add((*dynamic_program_header).p_vaddr) as *const DynamicArrayItem;

        StaticPie::<Ingredients> {
            base_address: base,
            dynamic_array,
            tls_program_header,
            phantom_data: PhantomData,
        }
    }
}

impl StaticPie<Ingredients> {
    #[inline(always)]
    #[must_use]
    pub unsafe fn relocate(self) -> StaticPie<Baked> {
        let mut rela_pointer: *const Rela = null();
        let mut rela_count = 0;

        for item in DynamicArrayIter::new(self.dynamic_array) {
            match item.d_tag {
                DT_RELA => {
                    rela_pointer =
                        self.base_address.byte_add(item.d_un.d_ptr.addr()) as *const Rela;
                }
                DT_RELASZ => {
                    rela_count = item.d_un.d_val / core::mem::size_of::<Rela>();
                }
                #[cfg(debug_assertions)]
                DT_RELAENT => {
                    syscall_debug_assert!(item.d_un.d_val as usize == size_of::<Rela>())
                }
                _ => (),
            }
        }

        syscall_debug_assert!(rela_pointer != null());
        let rela_slice = slice::from_raw_parts(rela_pointer, rela_count);

        #[cfg(target_arch = "x86_64")]
        for rela in rela_slice {
            let relocate_address = rela.r_offset.wrapping_add(self.base_address.addr());

            // x86_64 assembly pointer widths:
            // byte  | 8 bits  (1 byte)
            // word  | 16 bits (2 bytes)
            // dword | 32 bits (4 bytes) | "double word"
            // qword | 64 bits (8 bytes) | "quad word"
            use crate::elf::relocate::{R_X86_64_IRELATIVE, R_X86_64_RELATIVE};
            match rela.r_type() {
                R_X86_64_RELATIVE => {
                    let relocate_value =
                        self.base_address.addr().wrapping_add_signed(rela.r_addend);
                    asm!(
                        "mov qword ptr [{}], {}",
                        in(reg) relocate_address,
                        in(reg) relocate_value,
                        options(nostack, preserves_flags),
                    );
                }
                R_X86_64_IRELATIVE => {
                    let function_pointer =
                        self.base_address.addr().wrapping_add_signed(rela.r_addend);
                    let function: extern "C" fn() -> usize = core::mem::transmute(function_pointer);
                    let relocate_value = function();
                    asm!(
                        "mov qword ptr [{}], {}",
                        in(reg) relocate_address,
                        in(reg) relocate_value,
                        options(nostack, preserves_flags),
                    );
                }
                _ => {
                    eprintln!("Unsupported Relocation");
                    crate::syscall::exit::exit(32);
                }
            }
        }

        StaticPie::<Baked> {
            phantom_data: PhantomData::<Baked>,
            ..self
        }
    }
}

impl StaticPie<Baked> {
    #[inline(always)]
    pub unsafe fn allocate_tls(self, pseudorandom_bytes: &[u8; 16]) {
        // Static Thread Local Storage [before Thread Pointer]:
        //                                         ┌---------------------┐
        //      ┌----------------------------┐  <- |    tls-offset[1]    |
        //      |      Static TLS Block      |     |---------------------|
        //      |----------------------------|  <- | Thread Pointer (TP) |
        // ┌--- | Thread Control Block (TCB) |     └---------------------┘
        // |    └----------------------------┘
        // |
        // |   ┌------------------┐
        // └-> | Null Dtv Pointer |
        //     └------------------┘
        // NOTE: I am not bothering with alignment at the first address because it's already page aligned...
        if self.tls_program_header.is_null() {
            return;
        }
        let tls_program_header = *self.tls_program_header;

        let tls_blocks_size_and_align =
            round_up_to_boundary(tls_program_header.p_memsz, tls_program_header.p_align);
        let tcb_size = size_of::<ThreadControlBlock>();

        let required_size = tls_blocks_size_and_align + tcb_size;
        let tls_block_pointer = mmap(
            null_mut(),
            required_size,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1, // file descriptor (-1 for anonymous mapping)
            0,  // offset
        );

        // Initialize the TLS data from template image:
        slice::from_raw_parts_mut(tls_block_pointer as *mut u8, tls_program_header.p_filesz)
            .copy_from_slice(slice::from_raw_parts(
                self.base_address.byte_add(tls_program_header.p_offset) as *mut u8,
                tls_program_header.p_filesz,
            ));

        // Zero out TLS data beyond `p_filesz`:
        slice::from_raw_parts_mut(
            tls_block_pointer.byte_add(tls_program_header.p_filesz) as *mut u8,
            tls_program_header.p_memsz - tls_program_header.p_filesz,
        )
        .fill(0);

        // Initialize the Thread Control Block (TCB):
        let thread_control_block =
            tls_block_pointer.byte_add(tls_blocks_size_and_align) as *mut ThreadControlBlock;

        let thread_pointer_register: *mut () =
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

        // Make the thread pointer (which is fs on x86_64) point to the TCB:
        set_thread_pointer(thread_pointer_register);
    }
}

use std::{
    arch::asm,
    cmp::max,
    ffi::c_void,
    marker::PhantomData,
    ptr::{self, null, null_mut},
    slice,
};

#[cfg(debug_assertions)]
use crate::io_macros::syscall_assert;
use crate::{
    elf::{
        dynamic_array::{DynamicArrayItem, DynamicArrayIter, DynamicTag},
        header::ElfHeader,
        program_header::{ProgramHeader, PT_DYNAMIC, PT_PHDR, PT_TLS},
        relocate::Rela,
        thread_local_storage::ThreadControlBlock,
    },
    error::MirosError,
    io_macros::syscall_debug_assert,
    libc::mem::{mmap, MapFlags, ProtectionFlags},
    objects::strategies::init_array::InitArrayFunction,
    start::auxiliary_vector::AuxiliaryVectorItem,
    syscall::thread_pointer::set_thread_pointer,
    utils::round_up_to_boundary,
};

pub struct Relocate;
pub struct AllocateTls;
pub struct InitArray;

pub struct Miros<Stage> {
    base: *const c_void,
    rela_slice: *const [Rela],
    tls_program_header: Option<ProgramHeader>,
    init_array: Option<*const [InitArrayFunction]>,
    _marker: PhantomData<Stage>,
}

impl<Stage> Miros<Stage> {
    fn transition<NextStage>(self) -> Miros<NextStage> {
        Miros {
            base: self.base,
            rela_slice: self.rela_slice,
            tls_program_header: self.tls_program_header,
            init_array: self.init_array,
            _marker: PhantomData,
        }
    }
}

impl Miros<Relocate> {
    pub unsafe fn from_base(base: *const c_void) -> Result<Self, MirosError> {
        let header = &*(base as *const ElfHeader);
        syscall_debug_assert!(header.e_phentsize == size_of::<ProgramHeader>() as u16);

        let program_header_table = ptr::slice_from_raw_parts(
            base.byte_add(header.e_phoff) as *const ProgramHeader,
            header.e_phnum as usize,
        );

        let mut dynamic_program_header = null();
        let mut tls_program_header = None;
        for header in &*program_header_table {
            match header.p_type {
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = Some(*header),
                _ => (),
            }
        }

        Self::build_internal(base, dynamic_program_header, tls_program_header)
    }

    pub unsafe fn from_program_headers(
        program_header_table: *const [ProgramHeader],
    ) -> Result<Self, MirosError> {
        let (mut base, mut dynamic_program_header) = (null(), null());
        let mut tls_program_header = None;
        for header in &*program_header_table {
            match header.p_type {
                PT_PHDR => {
                    base =
                        (*program_header_table).as_ptr().byte_sub(header.p_vaddr) as *const c_void;
                }
                PT_DYNAMIC => dynamic_program_header = header,
                PT_TLS => tls_program_header = Some(*header),
                _ => (),
            }
        }

        Self::build_internal(base, dynamic_program_header, tls_program_header)
    }

    unsafe fn build_internal(
        base: *const c_void,
        dynamic_program_header: *const ProgramHeader,
        tls_program_header: Option<ProgramHeader>,
    ) -> Result<Self, MirosError> {
        syscall_debug_assert!(!dynamic_program_header.is_null());

        let dynamic_array =
            base.byte_add((*dynamic_program_header).p_vaddr) as *const DynamicArrayItem;

        let mut rela_pointer: Result<*const Rela, MirosError> =
            Err(MirosError::MissingDynamicEntry(DynamicTag::Rela));
        let mut rela_count = 0;
        let mut init_array_pointer: *const InitArrayFunction = ptr::null();
        let mut init_array_size = 0;

        DynamicArrayIter::new(dynamic_array).for_each(|item| match item.d_tag() {
            Ok(DynamicTag::Rela) => {
                rela_pointer = Ok(base.byte_add(item.d_un.d_ptr.addr()) as *const Rela);
            }
            Ok(DynamicTag::RelaSz) => {
                rela_count = item.d_un.d_val / size_of::<Rela>();
            }
            #[cfg(debug_assertions)]
            Ok(DynamicTag::RelaEnt) => {
                syscall_assert!(item.d_un.d_val == size_of::<Rela>())
            }
            Ok(DynamicTag::InitArray) => {
                init_array_pointer =
                    base.byte_add(item.d_un.d_ptr.addr()) as *const InitArrayFunction;
            }
            Ok(DynamicTag::InitArraySz) => {
                init_array_size = item.d_un.d_val / size_of::<usize>();
            }
            _ => (),
        });

        let rela_slice = ptr::slice_from_raw_parts(rela_pointer?, rela_count);

        let init_array = if init_array_pointer.is_null() || init_array_size == 0 {
            None
        } else {
            Some(ptr::slice_from_raw_parts(
                init_array_pointer,
                init_array_size,
            ))
        };

        Ok(Self {
            base,
            rela_slice,
            tls_program_header,
            init_array,
            _marker: PhantomData,
        })
    }

    #[cfg(target_arch = "x86_64")]
    pub unsafe fn relocate(self) -> Miros<AllocateTls> {
        use crate::elf::relocate::{R_X86_64_IRELATIVE, R_X86_64_RELATIVE};

        let base_address = self.base.addr();
        for rela in &*self.rela_slice {
            let relocate_address = rela.r_offset.wrapping_add(base_address);

            match rela.r_type() {
                R_X86_64_RELATIVE => {
                    let relocate_value = base_address.wrapping_add_signed(rela.r_addend);
                    asm!(
                        "mov qword ptr [{}], {}",
                        in(reg) relocate_address,
                        in(reg) relocate_value,
                        options(nostack, preserves_flags),
                    );
                }
                R_X86_64_IRELATIVE => {
                    let function_pointer = base_address.wrapping_add_signed(rela.r_addend);
                    let function: extern "C" fn() -> usize = core::mem::transmute(function_pointer);
                    let relocate_value = function();
                    asm!(
                        "mov qword ptr [{}], {}",
                        in(reg) relocate_address,
                        in(reg) relocate_value,
                        options(nostack, preserves_flags),
                    );
                }
                _ => (),
            }
        }

        self.transition()
    }
}

impl Miros<AllocateTls> {
    pub unsafe fn allocate_tls(self, pseudorandom_bytes: *const [u8; 16]) -> Miros<InitArray> {
        if let Some(tls_header) = self.tls_program_header {
            let max_required_align = max(align_of::<ThreadControlBlock>(), tls_header.p_align);
            let tls_block_size = round_up_to_boundary(tls_header.p_memsz, tls_header.p_align);
            let tcb_size = size_of::<ThreadControlBlock>() + max_required_align;

            let protection_flags = ProtectionFlags::ZERO
                .with_readable(true)
                .with_writable(true);
            let map_flags = MapFlags::ZERO.with_private(true).with_anonymous(true);

            let tls_block_pointer = mmap(
                null_mut(),
                tls_block_size + tcb_size,
                protection_flags,
                map_flags,
                -1,
                0,
            );
            assert_eq!(tls_block_pointer.addr() % max_required_align, 0);

            // Copy TLS template image
            slice::from_raw_parts_mut(tls_block_pointer as *mut u8, tls_header.p_filesz)
                .copy_from_slice(slice::from_raw_parts(
                    self.base.byte_add(tls_header.p_offset) as *const u8,
                    tls_header.p_filesz,
                ));

            // Zero TLS BSS
            slice::from_raw_parts_mut(
                tls_block_pointer.byte_add(tls_header.p_filesz) as *mut u8,
                tls_header.p_memsz - tls_header.p_filesz,
            )
            .fill(0);

            // Thread Control Block
            let thread_control_block =
                tls_block_pointer.byte_add(tls_block_size) as *mut ThreadControlBlock;
            let thread_pointer_register: *mut c_void =
                (*thread_control_block).thread_pointee.as_mut_ptr().cast();

            *thread_control_block = ThreadControlBlock {
                thread_pointee: [],
                thread_pointer_register,
                dynamic_thread_vector: null_mut(),
                _padding: [0; 3],
                canary: usize::from_ne_bytes(ptr::read(
                    pseudorandom_bytes.cast::<[u8; size_of::<usize>()]>(),
                )),
            };

            set_thread_pointer(thread_pointer_register);
        }

        self.transition()
    }
}

impl Miros<InitArray> {
    pub unsafe fn init_array(
        self,
        arg_count: usize,
        arg_pointer: *const *const u8,
        env_pointer: *const *const u8,
        auxv_pointer: *const AuxiliaryVectorItem,
    ) {
        if let Some(init_functions) = self.init_array {
            // SAFETY: The compiler thinks function pointers can't be null in Rust's type system,
            // but these are unsafely read from raw ELF init_array data...
            #[allow(useless_ptr_null_checks)]
            (&*init_functions)
                .iter()
                .filter(|init_fn| !(**init_fn as *const c_void).is_null())
                .for_each(|init_fn| {
                    init_fn(arg_count, arg_pointer, env_pointer, auxv_pointer);
                });
        }
    }
}

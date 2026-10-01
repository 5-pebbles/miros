use std::{
    ffi::{c_void, CStr, OsStr},
    fs::File,
    marker::PhantomData,
    os::unix::ffi::OsStrExt,
    ptr, slice,
};

use crate::{
    elf::{header::ElfHeader, program_header::ProgramHeader},
    objects::object_data::ObjectData,
    start::{
        arguments::{parse, usage},
        auxiliary_vector::{self, AuxiliaryVectorItem},
        config::ConfigOverrides,
    },
    syscall::exit::exit,
};

pub struct ParseFlags;
pub struct CompactStack;
pub struct LoadExecutable;
pub struct RetargetAuxv;

pub struct DirectInvocation<Stage> {
    stack_pointer: *mut usize,
    arg_count: usize,
    arg_pointer: *mut *const u8,
    env_pointer: *mut *const u8,
    auxv_pointer: *mut AuxiliaryVectorItem,
    _marker: PhantomData<Stage>,
}

pub struct StackContext {
    pub arg_count: usize,
    pub arg_pointer: *mut *const u8,
    pub env_pointer: *mut *const u8,
    pub auxv_pointer: *mut AuxiliaryVectorItem,
}

impl<Stage> DirectInvocation<Stage> {
    fn transition<NextStage>(self) -> DirectInvocation<NextStage> {
        DirectInvocation {
            stack_pointer: self.stack_pointer,
            arg_count: self.arg_count,
            arg_pointer: self.arg_pointer,
            env_pointer: self.env_pointer,
            auxv_pointer: self.auxv_pointer,
            _marker: PhantomData,
        }
    }
}

impl DirectInvocation<ParseFlags> {
    pub unsafe fn new(stack_pointer: *mut usize) -> Self {
        let arg_count = *stack_pointer;
        let arg_pointer = stack_pointer.add(1).cast::<*const u8>();
        let env_pointer = arg_pointer.add(arg_count + 1);
        let auxv_pointer = (0..)
            .map(|index| env_pointer.add(index))
            .find(|pointer| (**pointer).is_null())
            // SAFETY: the env array is null-terminated, so the find always succeeds before running off the stack.
            .unwrap_unchecked()
            .add(1)
            .cast::<AuxiliaryVectorItem>();

        Self {
            stack_pointer,
            arg_count,
            arg_pointer,
            env_pointer,
            auxv_pointer,
            _marker: PhantomData,
        }
    }

    pub unsafe fn parse_flags(self) -> (DirectInvocation<CompactStack>, ConfigOverrides, usize) {
        let arguments = slice::from_raw_parts(self.arg_pointer, self.arg_count);
        let (executable_index, overrides) = parse(arguments).unwrap_or_else(|error| {
            eprint!("{error}\n\n{}", usage());
            exit(2);
        });
        (self.transition(), overrides, executable_index)
    }
}

impl DirectInvocation<CompactStack> {
    pub unsafe fn compact_stack(mut self, drop_count: usize) -> DirectInvocation<LoadExecutable> {
        // The auxv's AT_NULL pair ends the pointer block. The strings above it stay.
        let mut block_end = self.auxv_pointer.cast::<usize>();
        while *block_end != 0 {
            block_end = block_end.add(2);
        }
        block_end = block_end.add(2);

        let source = self.arg_pointer.add(drop_count) as *const usize;
        let destination = self.arg_pointer as *mut usize;
        let word_count = (block_end as usize - source as usize) / size_of::<usize>();
        ptr::copy(source, destination, word_count);

        self.arg_count -= drop_count;
        *self.stack_pointer = self.arg_count;

        // byte_sub, not the typed sub: an auxv item is two words, so sub would overshoot.
        let shift_in_bytes = drop_count * size_of::<usize>();
        self.env_pointer = self.env_pointer.byte_sub(shift_in_bytes);
        self.auxv_pointer = self.auxv_pointer.byte_sub(shift_in_bytes);
        self.transition()
    }
}

impl DirectInvocation<LoadExecutable> {
    pub unsafe fn load_executable(
        self,
    ) -> (DirectInvocation<RetargetAuxv>, ObjectData, *const c_void) {
        let path = CStr::from_ptr(self.arg_pointer.read().cast());
        let file = File::open(OsStr::from_bytes(path.to_bytes())).unwrap_or_else(|error| {
            eprintln!("miros: {}: {error}", path.to_string_lossy());
            exit(1);
        });
        let executable = ObjectData::from_file(file).unwrap_or_else(|error| {
            eprintln!("{error}: {}", path.to_string_lossy());
            exit(1);
        });

        // from_file validated a PT_LOAD at file offset 0 / vaddr 0, so the ELF header sits at `base`.
        let header = &*(executable.base as *const ElfHeader);
        let entry = executable.base.byte_add(header.e_entry);
        (self.transition(), executable, entry)
    }
}

impl DirectInvocation<RetargetAuxv> {
    pub unsafe fn retarget_auxv(
        self,
        executable: &ObjectData,
        entry: *const c_void,
    ) -> StackContext {
        let header = &*(executable.base as *const ElfHeader);
        auxiliary_vector::retarget_executable(
            self.auxv_pointer,
            executable.base.byte_add(header.e_phoff) as *const ProgramHeader,
            header.e_phnum as usize,
            entry,
            self.arg_pointer.read(),
        );

        StackContext {
            arg_count: self.arg_count,
            arg_pointer: self.arg_pointer,
            env_pointer: self.env_pointer,
            auxv_pointer: self.auxv_pointer,
        }
    }
}

use std::{
    arch::{
        naked_asm,
        x86_64::{__cpuid_count, _xgetbv},
    },
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use crate::{
    elf::symbol::SymbolBinding,
    error::MirosError,
    objects::{object_data::ObjectData, object_data_graph::ObjectDataGraph},
    start::config,
};

// Naked assembly references these cells directly, so they cannot be thread-local.
static XSAVE_ALLOC_SIZE: AtomicUsize = AtomicUsize::new(0);
static XSAVE_MASK: AtomicU64 = AtomicU64::new(0);

pub fn initialize() -> bool {
    let Some((area_size, mask)) = detect_xsave() else {
        return false;
    };
    XSAVE_ALLOC_SIZE.store(area_size + 64, Ordering::Relaxed);
    XSAVE_MASK.store(mask, Ordering::Relaxed);
    true
}

fn detect_xsave() -> Option<(usize, u64)> {
    const XSAVE_CPUID_BIT: u32 = 1 << 26;
    const OSXSAVE_CPUID_BIT: u32 = 1 << 27;
    const XCR0_SSE_BIT: u64 = 1 << 1;

    let features = __cpuid_count(1, 0);
    let xsave = features.ecx & XSAVE_CPUID_BIT != 0;
    let osxsave = features.ecx & OSXSAVE_CPUID_BIT != 0;
    if !xsave || !osxsave {
        return None;
    }

    let mask = unsafe { _xgetbv(0) };
    if mask & XCR0_SSE_BIT == 0 {
        return None;
    }

    let area_size = __cpuid_count(0xD, 0).ebx as usize;
    Some((area_size, mask))
}

pub unsafe fn setup_global_offset_table(object: &ObjectData) {
    let Some(got) = object.dynamic_fields.global_offset_table else {
        return;
    };
    let got = got as *mut usize;
    got.add(1).write(object as *const ObjectData as usize);
    got.add(2).write(lazy_resolve_stub as *const () as usize);
}

// At entry, [rsp] is the ObjectData pointer and [rsp + 8] is the .rela.plt index.
// Every argument register and all vector state must survive, including rax because varargs callers pass the vector count in al.
#[unsafe(naked)]
unsafe extern "C" fn lazy_resolve_stub() {
    naked_asm!(
        "endbr64",
        "push rbx",
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        // The saved-register base must live in callee-saved rbx across fixup.
        "mov rbx, rsp",
        "sub rsp, qword ptr [rip + {alloc_size}]",
        "and rsp, -64",
        // XSAVE leaves header bits outside its mask unwritten, and XRSTOR rejects nonzero reserved header bytes.
        "mov qword ptr [rsp + 512], 0",
        "mov qword ptr [rsp + 520], 0",
        "mov qword ptr [rsp + 528], 0",
        "mov qword ptr [rsp + 536], 0",
        "mov qword ptr [rsp + 544], 0",
        "mov qword ptr [rsp + 552], 0",
        "mov qword ptr [rsp + 560], 0",
        "mov qword ptr [rsp + 568], 0",
        "mov rax, qword ptr [rip + {mask}]",
        "mov rdx, rax",
        "shr rdx, 32",
        "xsave64 [rsp]",
        "mov rdi, qword ptr [rbx + 64]",
        "mov rsi, qword ptr [rbx + 72]",
        "call {fixup}",
        "mov r11, rax",
        "mov rax, qword ptr [rip + {mask}]",
        "mov rdx, rax",
        "shr rdx, 32",
        "xrstor64 [rsp]",
        "mov rsp, rbx",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "pop rbx",
        "add rsp, 16",
        "jmp r11",
        alloc_size = sym XSAVE_ALLOC_SIZE,
        mask = sym XSAVE_MASK,
        fixup = sym fixup,
    )
}

extern "C" fn fixup(object: *const ObjectData, reloc_index: usize) -> usize {
    let object = unsafe { &*object };
    match resolve_and_patch(object, reloc_index) {
        Ok(address) => address,
        Err(error) => {
            eprintln!("{error}");
            crate::syscall::exit::exit(1);
        }
    }
}

fn resolve_and_patch(object: &ObjectData, reloc_index: usize) -> Result<usize, MirosError> {
    let rela = object
        .dynamic_fields
        .plt_rela_slice()
        .and_then(|relocations| relocations.get(reloc_index))
        .ok_or_else(|| {
            MirosError::ElfReadError(format!(
                "PLT relocation index {reloc_index} is out of bounds"
            ))
        })?;
    let symbol = object
        .dynamic_fields
        .checked_symbol(rela.r_sym() as usize)?;
    let symbol_name = unsafe {
        object
            .dynamic_fields
            .string_table
            .get(symbol.st_name as usize)
    };
    let resolved = ObjectDataGraph::global().resolve_symbol_address(symbol_name, object);

    if resolved.is_none() && symbol.binding() != Ok(SymbolBinding::Weak) {
        let error = MirosError::UndefinedSymbols(vec![symbol_name.to_string()]);
        if !config::lenient_undefined_symbols() {
            return Err(error);
        }
        eprintln!("{error}");
    }

    let address = resolved.map_or(0, |address| address as usize);
    let jump_slot = object.base.wrapping_add(rela.r_offset) as *mut usize;
    unsafe { AtomicUsize::from_ptr(jump_slot).store(address, Ordering::Relaxed) };
    Ok(address)
}

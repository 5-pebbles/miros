use std::{
    arch::x86_64::{__cpuid_count, _xgetbv},
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};

use crate::error::{MirosError, MissingCpuFeature, Result};

pub(crate) static XSAVE_ALLOC_SIZE: AtomicUsize = AtomicUsize::new(0);
pub(crate) static XSAVE_MASK: AtomicU64 = AtomicU64::new(0);

pub fn initialize() -> Result<()> {
    let (area_size, mask) = detect_xsave()?;
    XSAVE_ALLOC_SIZE.store(area_size + 64, Ordering::Relaxed);
    XSAVE_MASK.store(mask, Ordering::Relaxed);
    Ok(())
}

fn detect_xsave() -> Result<(usize, u64)> {
    const XSAVE_CPUID_BIT: u32 = 1 << 26;
    const OSXSAVE_CPUID_BIT: u32 = 1 << 27;
    const XCR0_SSE_BIT: u64 = 1 << 1;

    let features = __cpuid_count(1, 0);
    let xsave = features.ecx & XSAVE_CPUID_BIT != 0;
    let osxsave = features.ecx & OSXSAVE_CPUID_BIT != 0;
    if !xsave || !osxsave {
        return Err(MirosError::MissingCpuFeature(MissingCpuFeature::XSave));
    }

    let mask = unsafe { _xgetbv(0) };
    if mask & XCR0_SSE_BIT == 0 {
        return Err(MirosError::MissingCpuFeature(
            MissingCpuFeature::Xcr0SseBitNotSet,
        ));
    }

    let area_size = __cpuid_count(0xD, 0).ebx as usize;
    Ok((area_size, mask))
}

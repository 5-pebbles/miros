use core::ffi::{c_int, c_long, c_ulong};

use strum::FromRepr;

use crate::{
    libc::errno::{set_errno, Errno},
    page_size::get_page_size,
    signature_matches_libc,
    start::auxiliary_vector::get_auxiliary_value,
    syscall::{syscall, Syscall},
};

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn getauxval(auxv_type: c_ulong) -> c_ulong {
    signature_matches_libc!(libc::getauxval(auxv_type));
    get_auxiliary_value(auxv_type as usize).unwrap_or(0) as c_ulong
}

#[derive(FromRepr)]
#[repr(i32)]
enum SysconfName {
    ClockTicks = 2,
    OpenMax = 4,
    PageSize = 30,
    ProcessorsConfigured = 83,
    ProcessorsOnline = 84,
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sysconf(name: c_int) -> c_long {
    signature_matches_libc!(libc::sysconf(name));
    use SysconfName::*;
    match SysconfName::from_repr(name) {
        Some(PageSize) => get_page_size() as c_long,
        Some(ProcessorsConfigured | ProcessorsOnline) => online_processor_count(),
        Some(ClockTicks) => 100,
        Some(OpenMax) => 1024,
        None => {
            set_errno(Errno::INVAL);
            -1
        }
    }
}

/// Population count of the affinity mask; `sched_getaffinity` returns the number of bytes it wrote.
unsafe fn online_processor_count() -> c_long {
    let mut affinity_mask = [0u8; 128];
    let bytes_written = syscall!(
        Syscall::SchedGetAffinity,
        0usize,
        affinity_mask.len(),
        affinity_mask.as_mut_ptr()
    );
    if bytes_written < 0 {
        return 1;
    }
    affinity_mask
        .iter()
        .take(bytes_written as usize)
        .map(|byte| byte.count_ones() as c_long)
        .sum::<c_long>()
        .max(1)
}

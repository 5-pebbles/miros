use core::ffi::{c_int, c_long, c_ulong};

use strum::FromRepr;

use crate::{
    libc::errno::{set_errno, Errno},
    page_size::get_page_size,
    signature_matches_libc,
    start::auxiliary_vector::{get_auxiliary_value, AuxiliaryVectorType},
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
        Some(ClockTicks) => clock_ticks_per_second(),
        Some(OpenMax) => open_file_descriptor_limit(),
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

/// `AT_CLKTCK` from the kernel, falling back to `USER_HZ` (100) like glibc's `SYSTEM_CLK_TCK`.
unsafe fn clock_ticks_per_second() -> c_long {
    get_auxiliary_value(AuxiliaryVectorType::ClkTck as usize)
        .filter(|&value| value != 0)
        .unwrap_or(100) as c_long
}

/// Soft `RLIMIT_NOFILE` via `prlimit64`, falling back to `OPEN_MAX` (256) like glibc's `__getdtablesize`.
unsafe fn open_file_descriptor_limit() -> c_long {
    #[repr(C)]
    struct Rlimit64 {
        rlim_cur: u64,
        rlim_max: u64,
    }
    const RLIMIT_NOFILE: usize = 7;
    let mut rlimit = Rlimit64 {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let result = syscall!(
        Syscall::PrLimit64,
        0usize,
        RLIMIT_NOFILE,
        0usize,
        &mut rlimit as *mut Rlimit64
    );
    if result < 0 {
        256
    } else {
        rlimit.rlim_cur as c_long
    }
}

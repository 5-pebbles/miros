use std::arch::asm;

#[no_mangle]
unsafe extern "C" fn printf(_format: *const i8, ...) -> i32 {
    asm!("ud2", options(noreturn, nostack));
}

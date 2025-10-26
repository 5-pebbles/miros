// TODO: Kill the creators of this API... ヾ(๑╹◡╹)ﾉ🔪

pub mod environ;
// TODO: Add error handling for these things:
mod fs;
mod mem;

mod errno;

/// A macro for ensuring that the `libc` crate signature for a function matches
/// the signature that our implementation of it is using.
///
/// # Example
///
/// ```no_compile
/// #[no_mangle]
/// unsafe extern "C" fn strlen(s: *const c_char) -> usize {
///     signature_matches_libc!(libc::strlen(s));
///     // ...
/// }
/// ```
///
/// This will elicit a compile-time error if the signature doesn't match.
#[macro_export]
macro_rules! signature_matches_libc {
    ($e:expr) => {
        #[allow(unreachable_code)]
        #[allow(clippy::diverging_sub_expression)]
        if false { // NOTE: I usually just use core::mem::transmute and check that the sizes are the same...
            #[allow(unused_imports)]
            use crate::libc::*;
            return $e;
        }
    };
}

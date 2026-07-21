use core::{
    ffi::c_int,
    sync::atomic::{AtomicI32, Ordering},
};

use linkme::distributed_slice;

use crate::libc::interposable::{Bindable, InterposableCell, INTERPOSABLE_CELLS};

// The whole libm.so.6 surface, forwarded to the pure-Rust `libm` crate under identical names.
// Braced entries append their starred out pointers to the C signature and write the crate's
// tuple through them; with `-> ret` the tuple's .0 is the C return value and only .1 is written.
macro_rules! forward_to_libm {
    () => {};
    ($name:ident($($argument:ident: $argument_type:ty),*) -> $return_type:ty; $($rest:tt)*) => {
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name($($argument: $argument_type),*) -> $return_type {
            libm::$name($($argument),*)
        }
        forward_to_libm! { $($rest)* }
    };
    ($name:ident($($argument:ident: $argument_type:ty),*) -> $return_type:ty { *$out:ident: $out_type:ty } $($rest:tt)*) => {
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name($($argument: $argument_type,)* $out: *mut $out_type) -> $return_type {
            let (result, through_pointer) = libm::$name($($argument),*);
            *$out = through_pointer;
            result
        }
        forward_to_libm! { $($rest)* }
    };
    ($name:ident($($argument:ident: $argument_type:ty),*) { $(*$out:ident: $out_type:ty),* } $($rest:tt)*) => {
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name($($argument: $argument_type,)* $($out: *mut $out_type),*) {
            ($(*$out),*) = libm::$name($($argument),*);
        }
        forward_to_libm! { $($rest)* }
    };
}

forward_to_libm! {
    // f64
    acos(value: f64) -> f64;
    acosh(value: f64) -> f64;
    asin(value: f64) -> f64;
    asinh(value: f64) -> f64;
    atan(value: f64) -> f64;
    atan2(y: f64, x: f64) -> f64;
    atanh(value: f64) -> f64;
    cbrt(value: f64) -> f64;
    ceil(value: f64) -> f64;
    copysign(magnitude: f64, sign: f64) -> f64;
    cos(value: f64) -> f64;
    cosh(value: f64) -> f64;
    erf(value: f64) -> f64;
    erfc(value: f64) -> f64;
    exp(value: f64) -> f64;
    exp10(value: f64) -> f64;
    exp2(value: f64) -> f64;
    expm1(value: f64) -> f64;
    fabs(value: f64) -> f64;
    fdim(x: f64, y: f64) -> f64;
    floor(value: f64) -> f64;
    fma(multiplicand: f64, multiplier: f64, addend: f64) -> f64;
    fmax(x: f64, y: f64) -> f64;
    fmaximum(x: f64, y: f64) -> f64;
    fmaximum_num(x: f64, y: f64) -> f64;
    fmin(x: f64, y: f64) -> f64;
    fminimum(x: f64, y: f64) -> f64;
    fminimum_num(x: f64, y: f64) -> f64;
    fmod(numerator: f64, denominator: f64) -> f64;
    frexp(value: f64) -> f64 { *exponent: c_int }
    hypot(x: f64, y: f64) -> f64;
    ilogb(value: f64) -> c_int;
    j0(value: f64) -> f64;
    j1(value: f64) -> f64;
    jn(order: c_int, value: f64) -> f64;
    ldexp(value: f64, exponent: c_int) -> f64;
    lgamma_r(value: f64) -> f64 { *sign: c_int }
    log(value: f64) -> f64;
    log10(value: f64) -> f64;
    log1p(value: f64) -> f64;
    log2(value: f64) -> f64;
    modf(value: f64) -> f64 { *integral: f64 }
    nextafter(from: f64, toward: f64) -> f64;
    pow(base: f64, exponent: f64) -> f64;
    remainder(numerator: f64, denominator: f64) -> f64;
    remquo(numerator: f64, denominator: f64) -> f64 { *quotient: c_int }
    rint(value: f64) -> f64;
    round(value: f64) -> f64;
    roundeven(value: f64) -> f64;
    scalbn(value: f64, exponent: c_int) -> f64;
    sin(value: f64) -> f64;
    sincos(angle: f64) { *sine: f64, *cosine: f64 }
    sinh(value: f64) -> f64;
    sqrt(value: f64) -> f64;
    tan(value: f64) -> f64;
    tanh(value: f64) -> f64;
    tgamma(value: f64) -> f64;
    trunc(value: f64) -> f64;
    y0(value: f64) -> f64;
    y1(value: f64) -> f64;
    yn(order: c_int, value: f64) -> f64;
    // f32
    acosf(value: f32) -> f32;
    acoshf(value: f32) -> f32;
    asinf(value: f32) -> f32;
    asinhf(value: f32) -> f32;
    atan2f(y: f32, x: f32) -> f32;
    atanf(value: f32) -> f32;
    atanhf(value: f32) -> f32;
    cbrtf(value: f32) -> f32;
    ceilf(value: f32) -> f32;
    copysignf(magnitude: f32, sign: f32) -> f32;
    cosf(value: f32) -> f32;
    coshf(value: f32) -> f32;
    erfcf(value: f32) -> f32;
    erff(value: f32) -> f32;
    exp10f(value: f32) -> f32;
    exp2f(value: f32) -> f32;
    expf(value: f32) -> f32;
    expm1f(value: f32) -> f32;
    fabsf(value: f32) -> f32;
    fdimf(x: f32, y: f32) -> f32;
    floorf(value: f32) -> f32;
    fmaf(multiplicand: f32, multiplier: f32, addend: f32) -> f32;
    fmaxf(x: f32, y: f32) -> f32;
    fmaximum_numf(x: f32, y: f32) -> f32;
    fmaximumf(x: f32, y: f32) -> f32;
    fminf(x: f32, y: f32) -> f32;
    fminimum_numf(x: f32, y: f32) -> f32;
    fminimumf(x: f32, y: f32) -> f32;
    fmodf(numerator: f32, denominator: f32) -> f32;
    frexpf(value: f32) -> f32 { *exponent: c_int }
    hypotf(x: f32, y: f32) -> f32;
    ilogbf(value: f32) -> c_int;
    j0f(value: f32) -> f32;
    j1f(value: f32) -> f32;
    jnf(order: c_int, value: f32) -> f32;
    ldexpf(value: f32, exponent: c_int) -> f32;
    lgammaf_r(value: f32) -> f32 { *sign: c_int }
    log10f(value: f32) -> f32;
    log1pf(value: f32) -> f32;
    log2f(value: f32) -> f32;
    logf(value: f32) -> f32;
    modff(value: f32) -> f32 { *integral: f32 }
    nextafterf(from: f32, toward: f32) -> f32;
    powf(base: f32, exponent: f32) -> f32;
    remainderf(numerator: f32, denominator: f32) -> f32;
    remquof(numerator: f32, denominator: f32) -> f32 { *quotient: c_int }
    rintf(value: f32) -> f32;
    roundevenf(value: f32) -> f32;
    roundf(value: f32) -> f32;
    scalbnf(value: f32, exponent: c_int) -> f32;
    sincosf(angle: f32) { *sine: f32, *cosine: f32 }
    sinf(value: f32) -> f32;
    sinhf(value: f32) -> f32;
    sqrtf(value: f32) -> f32;
    tanf(value: f32) -> f32;
    tanhf(value: f32) -> f32;
    tgammaf(value: f32) -> f32;
    truncf(value: f32) -> f32;
    y0f(value: f32) -> f32;
    y1f(value: f32) -> f32;
    ynf(order: c_int, value: f32) -> f32;
}

// POSIX: lgamma reports Γ's sign through this global.
#[cfg_attr(not(test), export_name = "__signgam")]
#[allow(non_upper_case_globals)]
static signgam: AtomicI32 = AtomicI32::new(0);

static SIGNGAM: InterposableCell<i32> = InterposableCell::new("__signgam", signgam.as_ptr());

#[distributed_slice(INTERPOSABLE_CELLS)]
static SIGNGAM_CELL: &'static dyn Bindable = &SIGNGAM;

// Manifest exceptions: the crate names differ (lgamma → lgamma_r) and the sign must land in signgam atomically.

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn lgamma(value: f64) -> f64 {
    let (result, sign) = libm::lgamma_r(value);
    AtomicI32::from_ptr(SIGNGAM.as_ptr()).store(sign, Ordering::Relaxed);
    result
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn lgammaf(value: f32) -> f32 {
    let (result, sign) = libm::lgammaf_r(value);
    AtomicI32::from_ptr(SIGNGAM.as_ptr()).store(sign, Ordering::Relaxed);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_macros::eq_tests;

    // One representative per braced macro rule; every sibling is the same expansion.
    eq_tests!(mod out_pointer_family {
        frexp_six, {
            let mut exponent = 0;
            let mantissa = unsafe { frexp(6.0, &mut exponent) };
            (mantissa, exponent)
        }, (0.75, 3);
        sincos_matches_crate, {
            let (mut sine, mut cosine) = (0.0, 0.0);
            unsafe { sincos(0.5, &mut sine, &mut cosine) };
            (sine, cosine)
        }, libm::sincos(0.5)
    });
}

use core::ffi::c_int;

// miros intercepts libm.so.6, so the whole C math surface is ours to provide. Each export is a
// straight pass-through to the pure-Rust `libm` crate (fdlibm algorithms, no FFI, no global
// state); the crate's names are the C names, so the wrappers are correct by construction.
macro_rules! forward_to_libm {
    ($( $name:ident($($argument:ident: $argument_type:ty),*) -> $return_type:ty; )*) => { $(
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name($($argument: $argument_type),*) -> $return_type {
            libm::$name($($argument),*)
        }
    )* };
}

forward_to_libm! {
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
    hypot(x: f64, y: f64) -> f64;
    ilogb(value: f64) -> c_int;
    j0(value: f64) -> f64;
    j1(value: f64) -> f64;
    jn(order: c_int, value: f64) -> f64;
    ldexp(value: f64, exponent: c_int) -> f64;
    lgamma(value: f64) -> f64;
    log(value: f64) -> f64;
    log10(value: f64) -> f64;
    log1p(value: f64) -> f64;
    log2(value: f64) -> f64;
    nextafter(from: f64, toward: f64) -> f64;
    pow(base: f64, exponent: f64) -> f64;
    remainder(numerator: f64, denominator: f64) -> f64;
    rint(value: f64) -> f64;
    round(value: f64) -> f64;
    roundeven(value: f64) -> f64;
    scalbn(value: f64, exponent: c_int) -> f64;
    sin(value: f64) -> f64;
    sinh(value: f64) -> f64;
    sqrt(value: f64) -> f64;
    tan(value: f64) -> f64;
    tanh(value: f64) -> f64;
    tgamma(value: f64) -> f64;
    trunc(value: f64) -> f64;
    y0(value: f64) -> f64;
    y1(value: f64) -> f64;
    yn(order: c_int, value: f64) -> f64;

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
    hypotf(x: f32, y: f32) -> f32;
    ilogbf(value: f32) -> c_int;
    j0f(value: f32) -> f32;
    j1f(value: f32) -> f32;
    jnf(order: c_int, value: f32) -> f32;
    ldexpf(value: f32, exponent: c_int) -> f32;
    lgammaf(value: f32) -> f32;
    log10f(value: f32) -> f32;
    log1pf(value: f32) -> f32;
    log2f(value: f32) -> f32;
    logf(value: f32) -> f32;
    nextafterf(from: f32, toward: f32) -> f32;
    powf(base: f32, exponent: f32) -> f32;
    remainderf(numerator: f32, denominator: f32) -> f32;
    rintf(value: f32) -> f32;
    roundevenf(value: f32) -> f32;
    roundf(value: f32) -> f32;
    scalbnf(value: f32, exponent: c_int) -> f32;
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

// The out-pointer family: the crate returns tuples; .0 is the C return value, .1 goes through the pointer (sincos writes both).

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn frexp(value: f64, exponent: *mut c_int) -> f64 {
    let (mantissa, power) = libm::frexp(value);
    *exponent = power;
    mantissa
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn frexpf(value: f32, exponent: *mut c_int) -> f32 {
    let (mantissa, power) = libm::frexpf(value);
    *exponent = power;
    mantissa
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn lgamma_r(value: f64, sign: *mut c_int) -> f64 {
    let (result, sign_of_gamma) = libm::lgamma_r(value);
    *sign = sign_of_gamma;
    result
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn lgammaf_r(value: f32, sign: *mut c_int) -> f32 {
    let (result, sign_of_gamma) = libm::lgammaf_r(value);
    *sign = sign_of_gamma;
    result
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn modf(value: f64, integral: *mut f64) -> f64 {
    let (fractional, integral_part) = libm::modf(value);
    *integral = integral_part;
    fractional
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn modff(value: f32, integral: *mut f32) -> f32 {
    let (fractional, integral_part) = libm::modff(value);
    *integral = integral_part;
    fractional
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn remquo(numerator: f64, denominator: f64, quotient: *mut c_int) -> f64 {
    let (remainder, quotient_bits) = libm::remquo(numerator, denominator);
    *quotient = quotient_bits;
    remainder
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn remquof(numerator: f32, denominator: f32, quotient: *mut c_int) -> f32 {
    let (remainder, quotient_bits) = libm::remquof(numerator, denominator);
    *quotient = quotient_bits;
    remainder
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sincos(angle: f64, sine: *mut f64, cosine: *mut f64) {
    (*sine, *cosine) = libm::sincos(angle);
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn sincosf(angle: f32, sine: *mut f32, cosine: *mut f32) {
    (*sine, *cosine) = libm::sincosf(angle);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_macros::eq_tests;

    eq_tests!(mod out_pointer_family {
        frexp_six, {
            let mut exponent = 0;
            let mantissa = unsafe { frexp(6.0, &mut exponent) };
            (mantissa, exponent)
        }, (0.75, 3);
        frexpf_six, {
            let mut exponent = 0;
            let mantissa = unsafe { frexpf(6.0, &mut exponent) };
            (mantissa, exponent)
        }, (0.75, 3);
        modf_mixed, {
            let mut integral = 0.0;
            let fractional = unsafe { modf(3.25, &mut integral) };
            (fractional, integral)
        }, (0.25, 3.0);
        modff_mixed, {
            let mut integral = 0.0;
            let fractional = unsafe { modff(3.25, &mut integral) };
            (fractional, integral)
        }, (0.25, 3.0);
        remquo_seven_halves, {
            let mut quotient = 0;
            let remainder = unsafe { remquo(7.0, 2.0, &mut quotient) };
            (remainder, quotient)
        }, (-1.0, 4);
        remquof_seven_halves, {
            let mut quotient = 0;
            let remainder = unsafe { remquof(7.0, 2.0, &mut quotient) };
            (remainder, quotient)
        }, (-1.0, 4);
        sincos_matches_crate, {
            let (mut sine, mut cosine) = (0.0, 0.0);
            unsafe { sincos(0.5, &mut sine, &mut cosine) };
            (sine, cosine)
        }, libm::sincos(0.5);
        sincosf_matches_crate, {
            let (mut sine, mut cosine) = (0.0, 0.0);
            unsafe { sincosf(0.5, &mut sine, &mut cosine) };
            (sine, cosine)
        }, libm::sincosf(0.5);
        lgamma_r_matches_crate, {
            let mut sign = 0;
            let result = unsafe { lgamma_r(-0.5, &mut sign) };
            (result, sign)
        }, libm::lgamma_r(-0.5);
        lgammaf_r_matches_crate, {
            let mut sign = 0;
            let result = unsafe { lgammaf_r(-0.5, &mut sign) };
            (result, sign)
        }, libm::lgammaf_r(-0.5)
    });
}

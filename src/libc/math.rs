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

// TODO: frexp/modf/sincos/remquo/lgamma_r (+ f32 twins) return tuples in the `libm` crate but
// write through out-pointers in the C ABI; wire them once a target needs them.

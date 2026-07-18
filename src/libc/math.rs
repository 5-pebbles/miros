use core::ffi::c_int;

// miros intercepts libm.so.6, so the whole C math surface is ours to provide. Each export is a
// straight pass-through to the pure-Rust `libm` crate (fdlibm algorithms, no FFI, no global state);
// the crate's names are the C names, so the wrappers are correct by construction.

macro_rules! unary {
    ($float:ty; $($name:ident),* $(,)?) => { $(
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name(value: $float) -> $float {
            libm::$name(value)
        }
    )* };
}

macro_rules! binary {
    ($float:ty; $($name:ident),* $(,)?) => { $(
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name(x: $float, y: $float) -> $float {
            libm::$name(x, y)
        }
    )* };
}

macro_rules! ternary {
    ($float:ty; $($name:ident),* $(,)?) => { $(
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name(x: $float, y: $float, z: $float) -> $float {
            libm::$name(x, y, z)
        }
    )* };
}

// `ldexp`/`scalbn`: scale by a power of two given an integer exponent.
macro_rules! scale {
    ($float:ty; $($name:ident),* $(,)?) => { $(
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name(value: $float, exponent: c_int) -> $float {
            libm::$name(value, exponent)
        }
    )* };
}

// Bessel functions of integer order.
macro_rules! bessel {
    ($float:ty; $($name:ident),* $(,)?) => { $(
        #[cfg_attr(not(test), no_mangle)]
        unsafe extern "C" fn $name(order: c_int, value: $float) -> $float {
            libm::$name(order, value)
        }
    )* };
}

unary!(f64;
    acos, acosh, asin, asinh, atan, atanh, cbrt, ceil, cos, cosh, erf, erfc, exp, exp10, exp2,
    expm1, fabs, floor, j0, j1, lgamma, log, log10, log1p, log2, rint, round, roundeven, sin,
    sinh, sqrt, tan, tanh, tgamma, trunc, y0, y1,
);
unary!(f32;
    acosf, acoshf, asinf, asinhf, atanf, atanhf, cbrtf, ceilf, cosf, coshf, erfcf, erff, exp10f,
    exp2f, expf, expm1f, fabsf, floorf, j0f, j1f, lgammaf, log10f, log1pf, log2f, logf, rintf,
    roundevenf, roundf, sinf, sinhf, sqrtf, tanf, tanhf, tgammaf, truncf, y0f, y1f,
);

binary!(f64;
    atan2, copysign, fdim, fmax, fmaximum, fmaximum_num, fmin, fminimum, fminimum_num, fmod,
    hypot, nextafter, pow, remainder,
);
binary!(f32;
    atan2f, copysignf, fdimf, fmaxf, fmaximumf, fmaximum_numf, fminf, fminimumf, fminimum_numf,
    fmodf, hypotf, nextafterf, powf, remainderf,
);

ternary!(f64; fma);
ternary!(f32; fmaf);

scale!(f64; ldexp, scalbn);
scale!(f32; ldexpf, scalbnf);

bessel!(f64; jn, yn);
bessel!(f32; jnf, ynf);

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn ilogb(value: f64) -> c_int {
    libm::ilogb(value)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn ilogbf(value: f32) -> c_int {
    libm::ilogbf(value)
}

// TODO: frexp/modf/sincos/remquo/lgamma_r (+ f32) return tuples in the `libm` crate but write
// through out-pointers in the C ABI; wire them once a target needs them (verify the field order).

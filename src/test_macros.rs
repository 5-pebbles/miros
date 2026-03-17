/// Generates a module of `assert_eq!` tests from `(actual, expected)` pairs.
macro_rules! eq_tests {
    (mod $mod_name:ident {
        $($name:ident, $actual:expr, $expected:expr);* $(;)?
    }) => {
        mod $mod_name {
            use super::*;
            $(
                #[test]
                fn $name() {
                    assert_eq!($actual, $expected);
                }
            )*
        }
    };
}

pub(crate) use eq_tests;

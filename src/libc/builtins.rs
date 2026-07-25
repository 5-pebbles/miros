use core::arch::asm;

// mangled-names strips compiler-builtins' C-named intrinsics;
// LLVM's u128 division libcalls land here instead, bound locally by -Bsymbolic.

// Shift-subtract long division: u128 `/` or `%` here would lower to calls to the exports below.
fn unsigned_division_128(numerator: u128, denominator: u128) -> (u128, u128) {
    if denominator == 0 {
        // C UB; match hardware division and raise SIGFPE.
        unsafe { asm!("xor edx, edx", "div rdx", out("rax") _, out("rdx") _, options(nostack)) };
    }

    let mut quotient = 0u128;
    let mut remainder = 0u128;
    for bit_index in (0..u128::BITS - numerator.leading_zeros()).rev() {
        // Bit 127 shifting out makes the 129-bit remainder exceed any denominator.
        let shifted_out = remainder >> 127;
        remainder = (remainder << 1) | ((numerator >> bit_index) & 1);
        if shifted_out != 0 || remainder >= denominator {
            remainder = remainder.wrapping_sub(denominator);
            quotient |= 1u128 << bit_index;
        }
    }

    (quotient, remainder)
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __udivti3(numerator: u128, denominator: u128) -> u128 {
    unsigned_division_128(numerator, denominator).0
}

#[cfg_attr(not(test), no_mangle)]
unsafe extern "C" fn __umodti3(numerator: u128, denominator: u128) -> u128 {
    unsigned_division_128(numerator, denominator).1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_macros::eq_tests;

    // Test builds link real libgcc, so the native operators are the oracle.
    eq_tests!(mod edge_cases {
        zero_numerator, unsigned_division_128(0, 7), (0, 0);
        denominator_exceeds_numerator, unsigned_division_128(3, 10), (0, 3);
        equal_operands, unsigned_division_128(9, 9), (1, 0);
        max_by_one, unsigned_division_128(u128::MAX, 1), (u128::MAX, 0);
        max_by_max, unsigned_division_128(u128::MAX, u128::MAX), (1, 0);
        word_boundary, unsigned_division_128(u64::MAX as u128 + 5, u64::MAX as u128), (1, 5);
        remainder_carry, unsigned_division_128(u128::MAX, (1u128 << 127) | 1),
            (u128::MAX / ((1u128 << 127) | 1), u128::MAX % ((1u128 << 127) | 1));
    });

    #[test]
    fn sweep_matches_native_operators() {
        let values: Vec<u128> = (0..u128::BITS)
            .step_by(7)
            .flat_map(|shift| {
                let power = 1u128 << shift;
                [power - 1, power, power | 1, power | 0xdead_beef]
            })
            .chain([u128::MAX, u128::MAX - 1])
            .collect();

        for &numerator in &values {
            for &denominator in &values {
                if denominator == 0 {
                    continue;
                }
                assert_eq!(
                    unsigned_division_128(numerator, denominator),
                    (numerator / denominator, numerator % denominator),
                    "{numerator} / {denominator}"
                );
            }
        }
    }
}

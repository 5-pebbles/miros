use std::ptr;

use super::span::MAX_SLOTS_PER_SPAN;

/// C standard requires `malloc`/`realloc` to return memory aligned for any fundamental type. `_Alignof(max_align_t)` is 16 on x86_64.
const C_ABI_MIN_ALIGNMENT: usize = 16;

#[derive(Clone, Copy, PartialEq)]
pub struct SizeClass(u8);

impl SizeClass {
    /// `None` when the request exceeds [`MAX_SIZE_CLASS_SIZE`].
    ///
    /// Every class is a multiple of 16, so pow2 aligns up to 16 hold at every slot; a larger pow2 align holds iff it divides the slot size, since span bases are multiples of the pow2 span stride.
    #[inline(always)]
    pub fn from_layout(size: usize, align: usize) -> Option<Self> {
        // Rust and C only produce pow2 aligns; the promotion is defensive.
        let effective_align = align.next_power_of_two();
        let effective_size = size.max(effective_align).max(C_ABI_MIN_ALIGNMENT);
        if effective_size > MAX_SIZE_CLASS_SIZE {
            return None;
        }

        let bucket_index = (effective_size - 1) / C_ABI_MIN_ALIGNMENT;
        let mut class_index = *SIZE_CLASS_LOOKUP.get(bucket_index).unwrap() as usize;

        if effective_align > C_ABI_MIN_ALIGNMENT {
            // The largest class is a pow2 >= effective_align, so the scan stays inside the table.
            while SIZE_CLASSES.get(class_index).unwrap().slot_size_in_bytes % effective_align != 0 {
                class_index += 1;
            }
        }
        Some(SizeClass(class_index as u8))
    }

    #[inline(always)]
    pub const fn from_raw(raw: u8) -> Self {
        debug_assert!((raw as usize) < SIZE_CLASS_COUNT);
        SizeClass(raw)
    }

    #[inline(always)]
    pub const fn index(&self) -> usize {
        self.0 as usize
    }

    /// The stride is the next pow2 at or above the span length; the padding inside it stays unmapped.
    pub const fn span_stride_shift(&self) -> u32 {
        self.span_length_in_bytes()
            .next_power_of_two()
            .trailing_zeros()
    }

    #[inline(always)]
    pub const fn slot_size_in_bytes(&self) -> usize {
        SIZE_CLASSES[self.0 as usize].slot_size_in_bytes
    }

    #[inline(always)]
    pub const fn slot_index(&self, offset_into_span: usize) -> u16 {
        SIZE_CLASSES[self.0 as usize].slot_index(offset_into_span)
    }

    #[inline(always)]
    pub const fn slots_per_span(&self) -> u32 {
        SIZE_CLASSES[self.0 as usize].slots_per_span
    }

    #[inline(always)]
    pub const fn span_length_in_bytes(&self) -> usize {
        SIZE_CLASSES[self.0 as usize].span_length_in_bytes
    }
}

pub struct SizeClassInfo {
    pub(crate) slot_size_in_bytes: usize,
    slot_reciprocal: u64,
    slots_per_span: u32,
    span_length_in_bytes: usize,
}

/// Exact for every offset whose product with the reciprocal error term stays below 2^64: span offsets are under 2^20 and the error term is under the slot size (at most 2^17), so the product is under 2^37.
const fn reciprocal(divisor: usize) -> u64 {
    let numerator = 1u128 << 64;
    let divisor = divisor as u128;
    ((numerator + divisor - 1) / divisor) as u64
}

impl SizeClassInfo {
    const fn new(slot_size_in_bytes: usize) -> Self {
        // The multiple-of-16 property `from_layout`'s align shortcut depends on.
        assert!(slot_size_in_bytes % C_ABI_MIN_ALIGNMENT == 0);

        let slots_per_span =
            (MAX_SIZE_CLASS_SIZE / slot_size_in_bytes).clamp(8, MAX_SLOTS_PER_SPAN) as u32;

        Self {
            slot_size_in_bytes,
            slot_reciprocal: reciprocal(slot_size_in_bytes),
            slots_per_span,
            span_length_in_bytes: slot_size_in_bytes * slots_per_span as usize,
        }
    }

    #[inline(always)]
    const fn slot_index(&self, offset_into_span: usize) -> u16 {
        (((offset_into_span as u128) * (self.slot_reciprocal as u128)) >> 64) as u16
    }
}

/// Copy `SIZE` bytes using `u128` load/store pairs. For sizes ≤ 128 bytes
/// the compiler fully unrolls the loop into individual SIMD loads/stores.
/// Larger sizes delegate to `copy_nonoverlapping`.
///
/// Unlike `ptr::copy_nonoverlapping` with a constant size, which LLVM merges
/// across match arms into a single `memcpy` call when `#![no_builtins]` and
/// `-Z build-std` are active, each monomorphization here has a structurally
/// different loop body (different iteration count). This prevents the merge.
#[inline(always)]
unsafe fn copy_slot_inline<const SIZE: usize>(source: *const u8, dest: *mut u8) {
    if SIZE <= 128 {
        let src = source as *const u128;
        let dst = dest as *mut u128;
        for index in 0..SIZE / 16 {
            let value = ptr::read_unaligned(src.add(index));
            ptr::write_unaligned(dst.add(index), value);
        }
    } else {
        ptr::copy_nonoverlapping(source, dest, SIZE);
    }
}

macro_rules! define_size_classes {
    ($($sizes:expr),+) => {
        pub const MAX_SIZE_CLASS_SIZE: usize = define_size_classes!(@last $($sizes),+);
        pub const SIZE_CLASS_COUNT: usize = [$($sizes),+].len();
        pub const SIZE_CLASSES: &[SizeClassInfo; SIZE_CLASS_COUNT] = &[$(SizeClassInfo::new($sizes)),+];

        /// Bucket `b` covers request sizes in `(b * 16, b * 16 + 16]` and stores the tightest class covering them.
        pub const SIZE_CLASS_LOOKUP: [u8; MAX_SIZE_CLASS_SIZE / C_ABI_MIN_ALIGNMENT] = {
            let mut table = [0u8; MAX_SIZE_CLASS_SIZE / C_ABI_MIN_ALIGNMENT];
            let mut class_index = 0usize;
            let mut bucket = 0usize;
            while bucket < table.len() {
                // A slot above the bucket floor covers it: slots are multiples of 16 and the bucket's top is floor + 16.
                while SIZE_CLASSES[class_index].slot_size_in_bytes <= bucket * C_ABI_MIN_ALIGNMENT {
                    class_index += 1;
                }
                table[bucket] = class_index as u8;
                bucket += 1;
            }
            table
        };

        impl SizeClass {
            /// Copy exactly one slot's worth of bytes from `source` to `dest`.
            /// Generated by `define_size_classes!` so each class gets a
            /// compile-time-constant copy size.
            #[inline(always)]
            pub unsafe fn copy_slot(&self, source: *const u8, dest: *mut u8) {
                define_size_classes!(@gen_copy self.index(), source, dest, 0usize, $($sizes),+);
            }
        }
    };
    (@last $last:expr) => { $last };
    (@last $head:expr, $($tail:expr),+) => { define_size_classes!(@last $($tail),+) };
    (@gen_copy $idx:expr, $src:expr, $dst:expr, $n:expr, $size:expr, $($rest:expr),+) => {
        if $idx == $n {
            copy_slot_inline::<$size>($src, $dst);
            return;
        }
        define_size_classes!(@gen_copy $idx, $src, $dst, ($n + 1), $($rest),+);
    };
    (@gen_copy $idx:expr, $src:expr, $dst:expr, $n:expr, $size:expr) => {
        copy_slot_inline::<$size>($src, $dst);
    };
}

// 48 classes; the per-thread magazine pointer array (~6 KB) caps the count.
define_size_classes!(
    16, 32, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 448, 512, 640, 768, 896, 1024,
    1280, 1536, 1792, 2048, 2560, 3072, 3584, 4096, 5120, 6144, 7168, 8192, 10240, 12288, 14336,
    16384, 20480, 24576, 28672, 32768, 40960, 49152, 57344, 65536, 81920, 98304, 114688, 131072
);

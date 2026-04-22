use crate::allocator::primary_allocator::span::MAX_SLOTS_PER_SPAN;

/// C standard requires `malloc`/`realloc` to return memory aligned for any
/// fundamental type — `_Alignof(max_align_t)`, which is 16 on x86_64.
const C_ABI_MIN_ALIGNMENT: usize = 16;

#[derive(Clone, Copy, PartialEq)]
pub struct SizeClass(u8);

impl SizeClass {
    /// Map an allocation request to the tightest size class whose slot satisfies both the requested `size` and `align`.
    /// Returns `None` when the request exceeds [`MAX_SLOT_SIZE`].
    ///
    /// Because every class is a power of two, any slot >= `align` is naturally aligned.
    /// We take `max(size, align, C_ABI_MIN_ALIGNMENT)` then round up to the next power of two.
    #[inline(always)]
    pub fn from_layout(size: usize, align: usize) -> Option<Self> {
        let effective_size = size.max(align).max(C_ABI_MIN_ALIGNMENT);
        if effective_size > MAX_SIZE_CLASS_SIZE {
            return None;
        }

        // Sizes at or below the minimum class all land in index 0.
        // SAFETY: Only works as long as size classes are powers of two.
        let minimum_slot_size = 1usize << SIZE_CLASS_BASE_EXPONENT;
        let rounded = effective_size.next_power_of_two().max(minimum_slot_size);

        // All classes are contiguous powers of two starting at BASE_EXPONENT,
        // so trailing_zeros gives the exponent and subtracting the base gives the index.
        let index = rounded.trailing_zeros() - SIZE_CLASS_BASE_EXPONENT;
        debug_assert!((index as usize) < SIZE_CLASS_COUNT);
        Some(SizeClass(index as u8))
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

    #[inline(always)]
    pub const fn slot_size_in_bytes(&self) -> usize {
        SIZE_CLASSES[self.0 as usize].slot_size_in_bytes
    }

    #[inline(always)]
    pub const fn slot_shift(&self) -> u32 {
        SIZE_CLASSES[self.0 as usize].slot_shift
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
    slot_size_in_bytes: usize,
    /// Allows shifting offset instead of an expensive div instruction.
    // SAFETY: This only works as long as all size classes are powers of 2...
    slot_shift: u32,
    slots_per_span: u32,
    pub span_length_in_bytes: usize,
}

impl SizeClassInfo {
    const fn new(slot_size_in_bytes: usize) -> Self {
        assert!(slot_size_in_bytes.is_power_of_two());

        let slot_shift = slot_size_in_bytes.trailing_zeros();
        let slots_per_span =
            (MAX_SIZE_CLASS_SIZE / slot_size_in_bytes).clamp(8, MAX_SLOTS_PER_SPAN) as u32;

        Self {
            slot_size_in_bytes,
            slot_shift,
            slots_per_span,
            span_length_in_bytes: slot_size_in_bytes * slots_per_span as usize,
        }
    }
}

macro_rules! define_size_classes {
    ($($sizes:expr),+) => {
        pub const SIZE_CLASS_BASE_EXPONENT: u32 = SIZE_CLASSES[0].slot_size_in_bytes.trailing_zeros();
        pub const MAX_SIZE_CLASS_SIZE: usize = define_size_classes!(@last $($sizes),+);
        pub const SIZE_CLASS_COUNT: usize = [$($sizes),+].len();
        pub const SIZE_CLASSES: &[SizeClassInfo; SIZE_CLASS_COUNT] = &[$(SizeClassInfo::new($sizes)),+];
    };
    (@last $last:expr) => { $last };
    (@last $head:expr, $($tail:expr),+) => { define_size_classes!(@last $($tail),+) };
}

define_size_classes!(
    16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072
);

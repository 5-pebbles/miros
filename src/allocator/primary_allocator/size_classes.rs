pub struct SizeClass(usize);

impl SizeClass {
    #[inline(always)]
    pub const fn from_raw(raw: u8) -> Self {
        debug_assert!((raw as usize) < SIZE_CLASS_COUNT);
        SizeClass(raw as usize)
    }

    #[inline(always)]
    pub const fn slot_size_in_bytes(&self) -> usize {
        SIZE_CLASSES[self.0].slot_size_in_bytes
    }

    #[inline(always)]
    pub const fn slot_shift(&self) -> u32 {
        SIZE_CLASSES[self.0].slot_shift
    }

    #[inline(always)]
    pub const fn slots_per_span(&self) -> u32 {
        SIZE_CLASSES[self.0].slots_per_span
    }

    #[inline(always)]
    pub const fn span_length_in_bytes(&self) -> usize {
        SIZE_CLASSES[self.0].span_length_in_bytes
    }
}

pub struct SizeClassInfo {
    slot_size_in_bytes: usize,
    /// Allows shifting offset instead of an expensive div instruction.
    // SAFETY: This only works as long as all size classes are powers of 2...
    slot_shift: u32,
    slots_per_span: u32,
    span_length_in_bytes: usize,
}

impl SizeClassInfo {
    const fn new(slot_size_in_bytes: usize) -> Self {
        assert!(slot_size_in_bytes.is_power_of_two());

        let slot_shift = slot_size_in_bytes.trailing_zeros();
        let slots_per_span = (MAX_SLOT_SIZE / slot_size_in_bytes).clamp(8, 4096) as u32;

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
        pub const MAX_SLOT_SIZE: usize = define_size_classes!(@last $($sizes),+);
        pub const SIZE_CLASS_COUNT: usize = [$($sizes),+].len();
        pub const SIZE_CLASSES: &[SizeClassInfo; SIZE_CLASS_COUNT] = &[$(SizeClassInfo::new($sizes)),+];
    };
    (@last $last:expr) => { $last };
    (@last $head:expr, $($tail:expr),+) => { define_size_classes!(@last $($tail),+) };
}

define_size_classes!(
    8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072
);

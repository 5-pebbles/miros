/// xoroshiro128++ (Blackman & Vigna) with a bit cache: one 64-bit step feeds ~four bounded draws,
/// so the alloc hot path pays a multiply-shift, not a full PRNG step.
pub struct HeapRng {
    state_high: u64,
    state_low: u64,
    cache: u64,
    cache_bits: u32,
}

const DRAW_BITS: u32 = 16;

impl HeapRng {
    pub fn from_bytes(seed: u128) -> Self {
        debug_assert!(seed != 0);
        Self {
            state_high: (seed >> 64) as u64,
            state_low: seed as u64,
            cache: 0,
            cache_bits: 0,
        }
    }

    fn step(&mut self) -> u64 {
        let result = self
            .state_low
            .wrapping_add(self.state_high)
            .rotate_left(17)
            .wrapping_add(self.state_low);

        let xored = self.state_high ^ self.state_low;
        self.state_high = xored.rotate_left(28);
        self.state_low = self.state_low.rotate_left(49) ^ xored ^ (xored << 21);

        result
    }

    /// Uniform index in `[0, bound)` via Lemire multiply-shift over 16 cached bits.
    /// `bound` is a magazine count (<= 64), so 16 bits leaves the bias below 0.1%.
    #[inline(always)]
    pub fn index_below(&mut self, bound: usize) -> usize {
        if self.cache_bits < DRAW_BITS {
            self.cache = self.step();
            self.cache_bits = u64::BITS;
        }
        let chunk = self.cache & ((1 << DRAW_BITS) - 1);
        self.cache >>= DRAW_BITS;
        self.cache_bits -= DRAW_BITS;
        ((chunk * bound as u64) >> DRAW_BITS) as usize
    }

    /// Full-width entropy for the cold refill-claim path; bypasses the cache.
    #[inline(always)]
    pub fn next_u64(&mut self) -> u64 {
        self.step()
    }
}

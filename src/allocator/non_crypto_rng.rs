/// Transcribed from David Blackman and Sebastiano Vigna's work.
pub struct Xoroshiro128PlusPlus {
    state_high: u64,
    state_low: u64,
}

impl Xoroshiro128PlusPlus {
    pub fn from_bytes(seed: u128) -> Self {
        debug_assert!(seed != 0);

        Self {
            state_high: (seed >> 64) as u64,
            state_low: seed as u64,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
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
}

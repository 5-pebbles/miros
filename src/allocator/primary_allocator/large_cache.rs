use core::ptr;

const CAPACITY: usize = 8;

#[derive(Clone, Copy)]
pub struct CachedRegion {
    pub pointer: *mut u8,
    pub size_in_bytes: usize,
}

pub struct LargeCache {
    entries: [CachedRegion; CAPACITY],
    entry_count: u8,
}

impl LargeCache {
    pub const fn new() -> Self {
        Self {
            entries: [CachedRegion {
                pointer: ptr::null_mut(),
                size_in_bytes: 0,
            }; CAPACITY],
            entry_count: 0,
        }
    }

    /// Reclaim the tightest-fitting cached region with at least `minimum_bytes`.
    pub fn take(&mut self, minimum_bytes: usize) -> Option<CachedRegion> {
        let (index, _) = self.entries[..self.entry_count as usize]
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.size_in_bytes >= minimum_bytes)
            .min_by_key(|(_, entry)| entry.size_in_bytes)?;

        let region = self.entries[index];
        self.entry_count -= 1;
        self.entries[index] = self.entries[self.entry_count as usize];
        Some(region)
    }

    /// Attempt to cache a freed region for reuse. Returns `true` if stored,
    /// `false` if the cache is full and the caller should unmap.
    pub fn park(&mut self, region: CachedRegion) -> bool {
        if (self.entry_count as usize) >= CAPACITY {
            return false;
        }

        self.entries[self.entry_count as usize] = region;
        self.entry_count += 1;
        true
    }
}

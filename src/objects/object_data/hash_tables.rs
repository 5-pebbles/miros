use std::{ffi::c_void, ptr};

use crate::elf::{
    string_table::StringTable,
    symbol::{Symbol, SymbolTable},
};

pub enum HashTable {
    SystemV {
        buckets: *const [u32],
        chain: *const [u32],
    },
    GNU {
        symbol_offset: u32,
        bloom_shift: u32,
        bloom: *const [u64],
        buckets: *const [u32],
        chain: *const u32,
    },
}

impl HashTable {
    pub unsafe fn from_sysv(base: *const c_void, hash_table_pointer: *const c_void) -> Self {
        // ┌────────────────────┐
        // │ bucket_count (u32) │
        // │ chain_count  (u32) │
        // ├────────────────────┤
        // │ buckets   [u32; …] │
        // ├────────────────────┤
        // │ chain     [u32; …] │
        // └────────────────────┘
        let header = base.byte_add(hash_table_pointer.addr()) as *const u32;
        let bucket_count = *header as usize;
        let chain_count = *header.add(1) as usize;

        let buckets_start = header.add(2);
        let chain_start = buckets_start.add(bucket_count);

        Self::SystemV {
            buckets: ptr::slice_from_raw_parts(buckets_start, bucket_count),
            chain: ptr::slice_from_raw_parts(chain_start, chain_count),
        }
    }

    pub unsafe fn from_gnu(base: *const c_void, hash_table_pointer: *const c_void) -> Self {
        // ┌────────────────────┐
        // │ bucket_count (u32) │
        // │ symoffset    (u32) │
        // │ bloom_count  (u32) │
        // │ bloom_shift  (u32) │
        // ├────────────────────┤
        // │ bloom     [u64; …] │
        // ├────────────────────┤
        // │ buckets   [u32; …] │
        // ├────────────────────┤
        // │ chain     [u32; …] │
        // └────────────────────┘
        let header = base.byte_add(hash_table_pointer.addr()) as *const u32;
        let bucket_count = *header as usize;
        let symbol_offset = *header.add(1);
        let bloom_count = *header.add(2) as usize;
        let bloom_shift = *header.add(3);

        let bloom_start = header.add(4) as *const u64;
        let buckets_start = bloom_start.add(bloom_count) as *const u32;
        let chain_start = buckets_start.add(bucket_count);

        Self::GNU {
            symbol_offset,
            bloom_shift,
            bloom: ptr::slice_from_raw_parts(bloom_start, bloom_count),
            buckets: ptr::slice_from_raw_parts(buckets_start, bucket_count),
            chain: chain_start,
        }
    }

    pub unsafe fn lookup(
        &self,
        name: &str,
        symbol_table: &SymbolTable,
        string_table: &StringTable,
    ) -> Option<Symbol> {
        match self {
            Self::SystemV { buckets, chain } => {
                let buckets = &**buckets;
                let chain = &**chain;
                let hash = elf_hash(name);

                let mut symbol_index = buckets[hash as usize % buckets.len()] as usize;
                while symbol_index != 0 {
                    if let Some(symbol) =
                        resolve_symbol(symbol_index, name, symbol_table, string_table)
                    {
                        return Some(symbol);
                    }
                    symbol_index = chain[symbol_index] as usize;
                }
                None
            }
            Self::GNU {
                symbol_offset,
                bloom_shift,
                bloom,
                buckets,
                chain,
            } => {
                let bloom = &**bloom;
                let buckets = &**buckets;
                let hash = gnu_hash(name);

                // Bloom filter rejection:
                let word_bits = u64::BITS;
                let bloom_word = bloom[(hash / word_bits) as usize % bloom.len()];
                let primary_bit = 1u64 << (hash % word_bits);
                let secondary_bit = 1u64 << ((hash >> bloom_shift) % word_bits);
                let bloom_mask = primary_bit | secondary_bit;
                if bloom_word & bloom_mask != bloom_mask {
                    return None;
                }

                // Bucket lookup:
                let mut symbol_index = buckets[hash as usize % buckets.len()] as usize;
                if symbol_index == 0 {
                    return None;
                }

                // Chain walk (stop bit in bit 0 marks end of chain):
                let symbol_offset = *symbol_offset as usize;
                loop {
                    let chain_entry = *chain.add(symbol_index - symbol_offset);
                    if (chain_entry | 1) == (hash | 1) {
                        if let Some(symbol) =
                            resolve_symbol(symbol_index, name, symbol_table, string_table)
                        {
                            return Some(symbol);
                        }
                    }
                    if chain_entry & 1 != 0 {
                        break;
                    }
                    symbol_index += 1;
                }
                None
            }
        }
    }
}

unsafe fn resolve_symbol(
    symbol_index: usize,
    name: &str,
    symbol_table: &SymbolTable,
    string_table: &StringTable,
) -> Option<Symbol> {
    let symbol = symbol_table.get(symbol_index);
    (name == string_table.get(symbol.st_name as usize)).then_some(symbol)
}

fn elf_hash(name: &str) -> u32 {
    name.bytes().fold(0u32, |hash, byte| {
        let shifted = (hash << 4).wrapping_add(byte as u32);
        let high_nibble = shifted & 0xf0000000;
        (shifted ^ (high_nibble >> 24)) & !high_nibble
    })
}

fn gnu_hash(name: &str) -> u32 {
    const HASH_SEED: u32 = 5381;
    name.bytes().fold(HASH_SEED, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(byte as u32)
    })
}

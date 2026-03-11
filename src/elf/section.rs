use bitbybit::bitenum;

/// Special section header indices for the `st_shndx` field of a symbol.
/// Normal section indices (1..0xFF00) are not represented here and fall into `Err(raw)`.
///
/// The reserved range 0xFF00..0xFFFF (`SHN_LORESERVE`..`SHN_HIRESERVE`) includes
/// processor-specific indices (0xFF00..0xFF1F) and OS-specific indices (0xFF20..0xFF3F)
/// that are not modeled here — they will also fall into `Err(raw)`.
#[bitenum(u16, exhaustive = false)]
#[derive(PartialEq, Eq)]
pub enum SectionIndex {
    Undefined = 0,
    Absolute = 0xFFF1,
    Common = 0xFFF2,
    ExtendedIndex = 0xFFFF,
}

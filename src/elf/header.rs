pub const ET_DYN: u16 = 3;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq)]
pub struct ElfHeader {
    /// ELF identification array, containing the magic number and other information.
    ///
    /// The layout is as follows:
    /// - [0..4]: Magic Number (0x7F, 'E', 'L', 'F')
    /// - [4]: File Class (1 = 32-bit, 2 = 64-bit)
    /// - [5]: Endianness (1 = little-endian, 2 = big-endian)
    /// - [6]: Elf Version (should be 1)
    /// - [7]: OS ABI (0 = System V, 3 = Linux, etc.)
    /// - [8]: ABI Version
    /// - [9..16]: Padding (currently unused)
    pub e_ident: [u8; 16],
    /// The Elf file type, see the ET_.* constants.
    pub e_type: u16,
    /// The target archectecture, see the TODO constants.
    pub e_machine: u16,
    /// The Elf format version, only version one is currently available.
    pub e_version: u32,
    /// The virtual address to which the kernal or dynamic linker will jump when begining execution.
    pub e_entry: usize,
    /// The offset into the file at which the program header table resides.
    pub e_phoff: usize,
    /// The offset into the file at which the section header table resides.
    pub e_shoff: usize,
    /// A collection of processor-specific flags.
    pub e_flags: u32,
    /// The size of the Elf header you are currently reading, 52 for 32-bit systems and 64 for 64-bit ones.
    pub e_ehsize: u16,
    /// The size of each Elf program header table entry in bytes.
    pub e_phentsize: u16,
    /// The number of Elf program header table entries.
    pub e_phnum: u16,
    /// The size of each Elf section header table entry in bytes.
    pub e_shentsize: u16,
    /// The number of Elf section header table entries.
    pub e_shnum: u16,
    /// The index into the section header table at which the string table resides.
    pub e_shstrndx: u16,
}

// src/elf.rs

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64, // Virtual address of the entry point
    pub e_phoff: u64, // Offset of the program header table
    pub e_shoff: u64, // Offset of the section header table
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16, // Size of each entry in the program header table
    pub e_phnum: u16,     // Number of entries in the program header table
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,   // Segment type
    pub p_flags: u32,  // Permissions (R/W/X)
    pub p_offset: u64, // Offset of the segment in the file
    pub p_vaddr: u64,  // Virtual address of the segment
    pub p_paddr: u64,
    pub p_filesz: u64, // Size of the segment in the file
    pub p_memsz: u64,  // Size of the segment in memory
    pub p_align: u64,  // Alignment of the segment
}

pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
pub const PT_LOAD: u32 = 1; // Segment is loadable

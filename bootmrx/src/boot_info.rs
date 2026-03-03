// src/boot_info.rs
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    Free,
    KernelCode,
    KernelData,
    KernelStack,
    Reserved,
}

#[repr(C)]
pub struct MemoryMapEntry {
    pub mem_type: MemoryType,
    pub start: u64,
    pub n_pages: u64,
}

#[repr(C)]
pub struct MemoryMap {
    pub len: usize,
    pub entries: *mut MemoryMapEntry,
}

#[repr(C)]
pub struct Framebuffer {
    pub addr: u64,
    pub size: usize,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: u32,
}

#[repr(C)]
pub struct BootInfo {
    pub physical_memory_map: MemoryMap,
    pub framebuffer: Framebuffer,
}

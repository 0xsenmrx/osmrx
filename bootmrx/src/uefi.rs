use core::ffi::c_void;

// --- [1] BASIC TYPES & CONSTANTS ---
// -----------------------------------------------------------------------
pub type EfiStatus = usize;
pub type EfiHandle = *mut c_void;
pub type EfiPhysicalAddress = u64;
pub type EfiVirtualAddress = u64;

pub const EFI_SUCCESS: EfiStatus = 0;
// UEFI status codes use the top bit to indicate error.
pub const EFI_ERROR_MASK: EfiStatus = 1usize << (usize::BITS - 1);
pub const EFI_BUFFER_TOO_SMALL: EfiStatus = EFI_ERROR_MASK | 5;

#[inline(always)]
pub const fn efi_is_error(status: EfiStatus) -> bool {
    (status & EFI_ERROR_MASK) != 0
}

// File Open Modes
pub const EFI_FILE_MODE_READ: u64 = 0x0000000000000001;
pub const EFI_FILE_READ_ONLY: u64 = 0x0000000000000001;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EfiGuid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[repr(C)]
pub struct EfiTableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

// --- [2] GUID DEFINITIONS ---
// -----------------------------------------------------------------------
pub const EFI_LOADED_IMAGE_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x5B1B31A1,
    data2: 0x9562,
    data3: 0x11D2,
    data4: [0x8E, 0x3F, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B],
};

pub const EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x964E5B22,
    data2: 0x6459,
    data3: 0x11D2,
    data4: [0x8E, 0x39, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B],
};

pub const EFI_FILE_INFO_GUID: EfiGuid = EfiGuid {
    data1: 0x09576E92,
    data2: 0x6D3F,
    data3: 0x11D2,
    data4: [0x8E, 0x39, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B],
};

// Graphics Output Protocol (GOP)
pub const EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID: EfiGuid = EfiGuid {
    data1: 0x9042A9DE,
    data2: 0x23DC,
    data3: 0x4A38,
    data4: [0x96, 0xFB, 0x7A, 0xDE, 0xD0, 0x80, 0x51, 0x6A],
};

// --- [3] ENUMS ---
// -----------------------------------------------------------------------
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfiMemoryType {
    ReservedMemory = 0,
    LoaderCode,
    LoaderData,
    BootServicesCode,
    BootServicesData,
    RuntimeServicesCode,
    RuntimeServicesData,
    ConventionalMemory,
    UnusableMemory,
    ACPIReclaimMemory,
    ACPIMemoryNVS,
    MemoryMappedIO,
    MemoryMappedIOPortSpace,
    PalCode,
    PersistentMemory,
    UnacceptedMemory,
    // Custom OS Vendor types
    OsvKernelCode = 0x80000000,
    OsvKernelData = 0x80000001,
    OsvKernelStack = 0x80000002,
    MaxMemoryType,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfiAllocateType {
    AllocateAnyPages,
    AllocateMaxAddress,
    AllocateAddress,
    MaxAllocateType,
}

// --- [4] CORE SYSTEM TABLES ---
// -----------------------------------------------------------------------
#[repr(C)]
pub struct EfiSystemTable {
    pub header: EfiTableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub console_in_handle: EfiHandle,
    pub con_in: *const c_void,
    pub console_out_handle: EfiHandle,
    pub con_out: *mut SimpleTextOutputProtocol,
    pub standard_error_handle: EfiHandle,
    pub std_err: *mut SimpleTextOutputProtocol,
    pub runtime_services: *const c_void,
    pub boot_services: *mut EfiBootServices,
    pub num_table_entries: usize,
    pub config_table: *const c_void,
}

#[repr(C)]
pub struct EfiBootServices {
    pub hdr: EfiTableHeader,
    // Task Priority Services
    pub raise_tpl: unsafe extern "win64" fn(new_tpl: usize) -> usize,
    pub restore_tpl: unsafe extern "win64" fn(old_tpl: usize),
    // Memory Services
    pub allocate_pages: unsafe extern "win64" fn(
        allocate_type: EfiAllocateType,
        memory_type: EfiMemoryType,
        pages: usize,
        memory: *mut EfiPhysicalAddress,
    ) -> EfiStatus,
    pub free_pages: unsafe extern "win64" fn(memory: EfiPhysicalAddress, pages: usize) -> EfiStatus,
    pub get_memory_map: unsafe extern "win64" fn(
        memory_map_size: *mut usize,
        memory_map: *mut EfiMemoryDescriptor,
        map_key: *mut usize,
        descriptor_size: *mut usize,
        descriptor_version: *mut u32,
    ) -> EfiStatus,
    pub allocate_pool: unsafe extern "win64" fn(
        pool_type: EfiMemoryType,
        size: usize,
        buffer: *mut *mut c_void,
    ) -> EfiStatus,
    pub free_pool: unsafe extern "win64" fn(buffer: *mut c_void) -> EfiStatus,
    // Event & Timer Services
    _pad_event: [usize; 6],
    // Protocol Handler Services
    pub install_protocol_interface: *const c_void,
    pub reinstall_protocol_interface: *const c_void,
    pub uninstall_protocol_interface: *const c_void,
    pub handle_protocol: unsafe extern "win64" fn(
        handle: EfiHandle,
        protocol: *const EfiGuid,
        interface: *mut *mut c_void,
    ) -> EfiStatus,
    _reserved: *const c_void,
    pub register_protocol_notify: *const c_void,
    pub locate_handle: *const c_void,
    pub locate_device_path: *const c_void,
    pub install_configuration_table: *const c_void,
    // Image Services
    pub load_image: *const c_void,
    pub start_image: *const c_void,
    pub exit: *const c_void,
    pub unload_image: *const c_void,
    pub exit_boot_services:
        unsafe extern "win64" fn(image_handle: EfiHandle, map_key: usize) -> EfiStatus,
    // Misc & Protocol lookup (subset; remaining services are padded)
    _pad_after_exit_boot: [usize; 10], // GetNextMonotonicCount..LocateHandleBuffer
    pub locate_protocol: unsafe extern "win64" fn(
        protocol: *const EfiGuid,
        registration: *mut c_void,
        interface: *mut *mut c_void,
    ) -> EfiStatus,
    _pad_tail: [usize; 6], // InstallMultipleProtocolInterfaces..CreateEventEx
}

// --- [5] PROTOCOLS ---
// -----------------------------------------------------------------------

// Graphics Output Protocol (GOP)
#[repr(C)]
pub struct EfiGraphicsOutputModeInformation {
    pub version: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub pixel_format: u32,
    pub pixel_information: [u8; 16],
    pub pixels_per_scan_line: u32,
}

#[repr(C)]
pub struct EfiGraphicsOutputProtocolMode {
    pub max_mode: u32,
    pub mode: u32,
    pub info: *mut EfiGraphicsOutputModeInformation,
    pub size_of_info: usize,
    pub frame_buffer_base: EfiPhysicalAddress,
    pub frame_buffer_size: usize,
}

#[repr(C)]
pub struct EfiGraphicsOutputProtocol {
    pub query_mode: unsafe extern "win64" fn(
        this: *mut EfiGraphicsOutputProtocol,
        mode_number: u32,
        size_of_info: *mut usize,
        info: *mut *mut EfiGraphicsOutputModeInformation,
    ) -> EfiStatus,
    pub set_mode:
        unsafe extern "win64" fn(this: *mut EfiGraphicsOutputProtocol, mode_number: u32) -> EfiStatus,
    pub blt: *const c_void,
    pub mode: *mut EfiGraphicsOutputProtocolMode,
}

// Console Output
#[repr(C)]
pub struct SimpleTextOutputProtocol {
    pub reset:
        unsafe extern "win64" fn(this: *mut SimpleTextOutputProtocol, verify: bool) -> EfiStatus,
    pub output_string:
        unsafe extern "win64" fn(this: *mut SimpleTextOutputProtocol, s: *const u16) -> EfiStatus,
    _pad: [usize; 3],
    pub set_attribute:
        unsafe extern "win64" fn(this: *mut SimpleTextOutputProtocol, attr: usize) -> EfiStatus,
    pub clear_screen: unsafe extern "win64" fn(this: *mut SimpleTextOutputProtocol) -> EfiStatus,
    _pad2: [usize; 3],
}

// Loaded Image
#[repr(C)]
pub struct EfiLoadedImageProtocol {
    pub revision: u32,
    pub parent_handle: EfiHandle,
    pub system_table: *mut EfiSystemTable,
    pub device_handle: EfiHandle,
    pub file_path: *mut c_void,
    pub reserved: *mut c_void,
    pub load_options_size: u32,
    pub load_options: *mut c_void,
    pub image_base: *mut c_void,
    pub image_size: u64,
    pub image_code_type: EfiMemoryType,
    pub image_data_type: EfiMemoryType,
    pub unload: unsafe extern "win64" fn(image_handle: EfiHandle) -> EfiStatus,
}

// File System
#[repr(C)]
pub struct EfiSimpleFileSystemProtocol {
    pub revision: u64,
    pub open_volume: unsafe extern "win64" fn(
        this: *mut EfiSimpleFileSystemProtocol,
        root: *mut *mut EfiFileProtocol,
    ) -> EfiStatus,
}

#[repr(C)]
pub struct EfiFileProtocol {
    pub revision: u64,
    pub open: unsafe extern "win64" fn(
        this: *mut EfiFileProtocol,
        new_handle: *mut *mut EfiFileProtocol,
        name: *const u16,
        mode: u64,
        attr: u64,
    ) -> EfiStatus,
    pub close: unsafe extern "win64" fn(this: *mut EfiFileProtocol) -> EfiStatus,
    pub delete: unsafe extern "win64" fn(this: *mut EfiFileProtocol) -> EfiStatus,
    pub read: unsafe extern "win64" fn(
        this: *mut EfiFileProtocol,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> EfiStatus,
    pub write: unsafe extern "win64" fn(
        this: *mut EfiFileProtocol,
        buffer_size: *mut usize,
        buffer: *const c_void,
    ) -> EfiStatus,
    pub get_position:
        unsafe extern "win64" fn(this: *mut EfiFileProtocol, position: *mut u64) -> EfiStatus,
    pub set_position:
        unsafe extern "win64" fn(this: *mut EfiFileProtocol, position: u64) -> EfiStatus,
    pub get_info: unsafe extern "win64" fn(
        this: *mut EfiFileProtocol,
        info_type: *const EfiGuid,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> EfiStatus,
    pub set_info: unsafe extern "win64" fn(
        this: *mut EfiFileProtocol,
        info_type: *const EfiGuid,
        buffer_size: usize,
        buffer: *const c_void,
    ) -> EfiStatus,
    pub flush: unsafe extern "win64" fn(this: *mut EfiFileProtocol) -> EfiStatus,
}

// --- [6] DATA STRUCTURES ---
// -----------------------------------------------------------------------
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct EfiMemoryDescriptor {
    pub memory_type: EfiMemoryType,
    pub physical_start: EfiPhysicalAddress,
    pub virtual_start: EfiVirtualAddress,
    pub number_of_pages: u64,
    pub attribute: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct EfiTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub pad1: u8,
    pub nanosecond: u32,
    pub time_zone: i16,
    pub daylight: u8,
    pub pad2: u8,
}

#[repr(C)]
pub struct EfiFileInfo {
    pub size: u64,
    pub file_size: u64,
    pub physical_size: u64,
    pub create_time: EfiTime,
    pub last_access_time: EfiTime,
    pub modification_time: EfiTime,
    pub attribute: u64,
    pub file_name: [u16; 256],
}

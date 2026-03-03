#![no_std]
#![no_main]

mod boot_info;
mod elf;
mod logger;
mod uefi;
mod utils;

use core::mem::MaybeUninit;
use core::panic::PanicInfo;

use elf::{ELF_MAGIC, Elf64Header, Elf64Phdr, PT_LOAD};

use uefi::{
    EFI_BUFFER_TOO_SMALL, EFI_FILE_INFO_GUID, EFI_FILE_MODE_READ, EFI_FILE_READ_ONLY,
    EFI_LOADED_IMAGE_PROTOCOL_GUID, EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID, EFI_SUCCESS,
    EfiAllocateType, EfiFileProtocol, EfiHandle, EfiLoadedImageProtocol, EfiMemoryDescriptor,
    EfiMemoryType, EfiSimpleFileSystemProtocol, EfiStatus, EfiSystemTable,
};

use utils::{check_status, halt, print_kernel_info};

/// Bootloader Constants
const PAGE_SIZE: usize = 4096;
// Fallback load base if allocating at the ELF vaddr fails.
// (Current kernel is linked at 0x0020_0000.)
const KERNEL_PHYSICAL_BASE: u64 = 0x0020_0000;
const KERNEL_STACK_SIZE: usize = 128 * 1024; // 128 KiB

#[unsafe(no_mangle)]
/// # Safety
///
/// UEFI calls this entry point with valid pointers for `system_table`, and a valid
/// `image_handle`. The bootloader assumes `system_table`, its nested pointers
/// (boot services, console output), and the referenced protocols remain valid
/// for the duration of this function (until `ExitBootServices`).
pub unsafe extern "win64" fn efi_main(
    image_handle: EfiHandle,
    system_table: *mut EfiSystemTable,
) -> EfiStatus {
    let st = unsafe { &*system_table };
    let bt = unsafe { &*st.boot_services };
    let con_out = unsafe { &mut *st.con_out };

    // --- 1. Initialization & UI ---
    unsafe {
        (con_out.clear_screen)(con_out);
    }
    logger::log(st, 0x0E, "OSMRX Bootloader\n\n");

    // --- 2. Protocol Acquisition ---
    let mut loaded_image: *mut EfiLoadedImageProtocol = core::ptr::null_mut();
    let status = unsafe {
        (bt.handle_protocol)(
            image_handle,
            &EFI_LOADED_IMAGE_PROTOCOL_GUID,
            &mut loaded_image as *mut *mut _ as *mut *mut core::ffi::c_void,
        )
    };
    check_status(st, status, "boot: acquiring LoadedImageProtocol");

    let device_handle = unsafe { (*loaded_image).device_handle };
    let mut file_system: *mut EfiSimpleFileSystemProtocol = core::ptr::null_mut();
    let status = unsafe {
        (bt.handle_protocol)(
            device_handle,
            &EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID,
            &mut file_system as *mut *mut _ as *mut *mut core::ffi::c_void,
        )
    };
    check_status(st, status, "boot: acquiring SimpleFileSystemProtocol");

    // --- 3. File System Operations ---
    let mut root_dir: *mut EfiFileProtocol = core::ptr::null_mut();
    let status = unsafe { ((*file_system).open_volume)(file_system, &mut root_dir) };
    check_status(st, status, "boot: opening root directory");

    let mut kernel_file: *mut EfiFileProtocol = core::ptr::null_mut();
    let kernel_path = [
        'k' as u16, 'e' as u16, 'r' as u16, 'n' as u16, 'e' as u16, 'l' as u16, '.' as u16,
        'e' as u16, 'l' as u16, 'f' as u16, 0,
    ];

    let status = unsafe {
        ((*root_dir).open)(
            root_dir,
            &mut kernel_file,
            kernel_path.as_ptr(),
            EFI_FILE_MODE_READ,
            EFI_FILE_READ_ONLY,
        )
    };
    check_status(st, status, "boot: opening kernel file");

    // --- 4. Kernel Metadata Analysis ---
    // EfiFileInfo size is variable (file name length), so do a two-call query.
    let mut info_size: usize = 0;
    let mut status = unsafe {
        ((*kernel_file).get_info)(
            kernel_file,
            &EFI_FILE_INFO_GUID,
            &mut info_size,
            core::ptr::null_mut(),
        )
    };
    if status != EFI_BUFFER_TOO_SMALL || info_size == 0 {
        logger::log(st, 0x0C, "boot: failed to query kernel info size\n");
        halt();
    }

    let mut info_buf: *mut core::ffi::c_void = core::ptr::null_mut();
    status = unsafe { (bt.allocate_pool)(EfiMemoryType::LoaderData, info_size, &mut info_buf) };
    check_status(st, status, "boot: allocating kernel metadata buffer");

    status = unsafe {
        ((*kernel_file).get_info)(kernel_file, &EFI_FILE_INFO_GUID, &mut info_size, info_buf)
    };
    check_status(st, status, "boot: retrieving kernel metadata");

    let info = unsafe { &*(info_buf as *const uefi::EfiFileInfo) };
    print_kernel_info(st, info);

    // --- 5. ELF Header Parsing ---

    let mut elf_hdr = MaybeUninit::<Elf64Header>::uninit();
    let mut hdr_size = core::mem::size_of::<Elf64Header>();
    let status = unsafe {
        ((*kernel_file).read)(kernel_file, &mut hdr_size, elf_hdr.as_mut_ptr() as *mut _)
    };
    check_status(st, status, "boot: reading ELF header");
    if hdr_size != core::mem::size_of::<Elf64Header>() {
        logger::log(st, 0x0C, "boot: short read while reading ELF header\n");
        halt();
    }
    let elf_hdr = unsafe { elf_hdr.assume_init() };

    if elf_hdr.e_ident[0..4] != ELF_MAGIC {
        logger::log(st, 0x0C, "boot: error - kernel is not a valid ELF file\n");
        halt();
    }

    // --- 6. Determine ELF Load Range & Allocate Kernel Memory ---

    // First pass over program headers: find lowest and highest virtual addresses
    let mut first_vaddr: u64 = u64::MAX;
    let mut last_vaddr: u64 = 0;

    for i in 0..elf_hdr.e_phnum {
        let mut phdr = MaybeUninit::<Elf64Phdr>::uninit();
        let mut phdr_size = elf_hdr.e_phentsize as usize;

        unsafe {
            let _ = ((*kernel_file).set_position)(
                kernel_file,
                elf_hdr.e_phoff + (i as u64 * elf_hdr.e_phentsize as u64),
            );
            let _ = ((*kernel_file).read)(kernel_file, &mut phdr_size, phdr.as_mut_ptr() as *mut _);
        }
        let phdr = unsafe { phdr.assume_init() };

        if phdr.p_type == PT_LOAD && phdr.p_memsz != 0 {
            if phdr.p_vaddr < first_vaddr {
                first_vaddr = phdr.p_vaddr;
            }
            let segment_end = phdr.p_vaddr + phdr.p_memsz;
            if segment_end > last_vaddr {
                last_vaddr = segment_end;
            }
        }
    }

    if first_vaddr == u64::MAX || last_vaddr <= first_vaddr {
        logger::log(st, 0x0C, "boot: no loadable segments in ELF\n");
        halt();
    }

    // Page-align the total span of the kernel image.
    let first_vaddr_aligned = first_vaddr & !((PAGE_SIZE as u64) - 1);
    let last_vaddr_aligned = (last_vaddr + (PAGE_SIZE as u64) - 1) & !((PAGE_SIZE as u64) - 1);
    let total_size = last_vaddr_aligned - first_vaddr_aligned;
    let kernel_pages = (total_size as usize) / PAGE_SIZE;

    // Allocate at the ELF's linked address range (best case: delta = 0).
    // If that fails (address already used), fall back to a fixed base.
    let mut kernel_image_base: u64 = first_vaddr_aligned;
    let mut status = unsafe {
        (bt.allocate_pages)(
            EfiAllocateType::AllocateAddress,
            EfiMemoryType::OsvKernelCode,
            kernel_pages,
            &mut kernel_image_base,
        )
    };
    if status != EFI_SUCCESS {
        kernel_image_base = KERNEL_PHYSICAL_BASE;
        status = unsafe {
            (bt.allocate_pages)(
                EfiAllocateType::AllocateAddress,
                EfiMemoryType::OsvKernelCode,
                kernel_pages,
                &mut kernel_image_base,
            )
        };
    }
    check_status(st, status, "boot: allocating kernel memory");

    // Relocation delta so that first_vaddr_aligned maps to kernel_image_base.
    let relocate_delta = match kernel_image_base.checked_sub(first_vaddr_aligned) {
        Some(d) => d,
        None => {
            logger::log(
                st,
                0x0C,
                "boot: kernel base below ELF vaddr; cannot relocate safely\n",
            );
            halt();
        }
    };

    // Allocate a stack for the kernel
    let mut kernel_stack_base: u64 = 0;
    let stack_pages = KERNEL_STACK_SIZE / PAGE_SIZE;
    let status = unsafe {
        (bt.allocate_pages)(
            EfiAllocateType::AllocateAnyPages,
            EfiMemoryType::OsvKernelStack,
            stack_pages,
            &mut kernel_stack_base,
        )
    };
    check_status(st, status, "boot: allocating kernel stack");

    // --- 7. Program Header Parsing & Segment Loading ---

    for i in 0..elf_hdr.e_phnum {
        let mut phdr = MaybeUninit::<Elf64Phdr>::uninit();
        let mut phdr_size = elf_hdr.e_phentsize as usize;

        unsafe {
            let status = ((*kernel_file).set_position)(
                kernel_file,
                elf_hdr.e_phoff + (i as u64 * elf_hdr.e_phentsize as u64),
            );
            if status != EFI_SUCCESS {
                logger::log(st, 0x0C, "boot: failed to seek to program header\n");
                halt();
            }
            let status =
                ((*kernel_file).read)(kernel_file, &mut phdr_size, phdr.as_mut_ptr() as *mut _);
            if status != EFI_SUCCESS || phdr_size != core::mem::size_of::<Elf64Phdr>() {
                logger::log(st, 0x0C, "boot: failed to read program header\n");
                halt();
            }
        }
        let phdr = unsafe { phdr.assume_init() };

        if phdr.p_type == PT_LOAD && phdr.p_memsz != 0 {
            // Destination physical address after relocation
            let dest = (phdr.p_vaddr + relocate_delta) as *mut u8;

            // Read initialized data from file
            let mut read_size = phdr.p_filesz as usize;
            if read_size != 0 {
                unsafe {
                    let status = ((*kernel_file).set_position)(kernel_file, phdr.p_offset);
                    if status != EFI_SUCCESS {
                        logger::log(st, 0x0C, "boot: failed to seek to segment\n");
                        halt();
                    }
                    let status = ((*kernel_file).read)(kernel_file, &mut read_size, dest as *mut _);
                    if status != EFI_SUCCESS || read_size != phdr.p_filesz as usize {
                        logger::log(st, 0x0C, "boot: short read while reading segment\n");
                        halt();
                    }
                }
            }

            // Zero-fill BSS (uninitialized data where MemSiz > FileSiz)
            if phdr.p_memsz > phdr.p_filesz {
                let bss_start = unsafe { dest.add(phdr.p_filesz as usize) };
                let bss_size = (phdr.p_memsz - phdr.p_filesz) as usize;
                unsafe {
                    core::ptr::write_bytes(bss_start, 0, bss_size);
                }
            }
        }
    }

    // --- 6. Resource Cleanup ---
    unsafe {
        let _ = (bt.free_pool)(info_buf);
        let _ = ((*kernel_file).close)(kernel_file);
        let _ = ((*root_dir).close)(root_dir);
    }

    // --- 7. Memory Map & Boot Services Exit ---
    let mut map_size: usize = 0;
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_ver: u32 = 0;

    // Get initial size requirement (expected: EFI_BUFFER_TOO_SMALL)
    let status = unsafe {
        (bt.get_memory_map)(
            &mut map_size,
            core::ptr::null_mut(),
            &mut map_key,
            &mut desc_size,
            &mut desc_ver,
        )
    };
    if status != EFI_BUFFER_TOO_SMALL || desc_size == 0 {
        logger::log(st, 0x0C, "boot: failed to query memory map size\n");
        halt();
    }

    // Allocate buffer with headroom for growth during allocation
    map_size += desc_size * 2;
    let mut memory_map: *mut EfiMemoryDescriptor = core::ptr::null_mut();
    let status = unsafe {
        (bt.allocate_pool)(
            EfiMemoryType::LoaderData,
            map_size,
            &mut memory_map as *mut *mut _ as *mut *mut core::ffi::c_void,
        )
    };
    check_status(st, status, "boot: allocating memory map pool");

    // 1. Allocate BootInfo buffer large enough for the memory map we will copy.
    // Allocate before the final get_memory_map call, so it doesn't perturb the map_key.
    let max_entries = (map_size / desc_size) + 16; // headroom for map growth
    let boot_info_bytes = core::mem::size_of::<boot_info::BootInfo>()
        + max_entries * core::mem::size_of::<boot_info::MemoryMapEntry>();
    let boot_info_pages = boot_info_bytes.div_ceil(PAGE_SIZE);

    let mut boot_info_addr: u64 = 0;
    let status = unsafe {
        (bt.allocate_pages)(
            EfiAllocateType::AllocateAnyPages,
            EfiMemoryType::OsvKernelData,
            boot_info_pages,
            &mut boot_info_addr,
        )
    };
    check_status(st, status, "boot: allocating boot info buffer");

    // Initialize framebuffer information in BootInfo from GOP.
    let mut gop: *mut uefi::EfiGraphicsOutputProtocol = core::ptr::null_mut();
    let status = unsafe {
        (bt.locate_protocol)(
            &uefi::EFI_GRAPHICS_OUTPUT_PROTOCOL_GUID,
            core::ptr::null_mut(),
            &mut gop as *mut *mut _ as *mut *mut core::ffi::c_void,
        )
    };
    check_status(st, status, "boot: locating graphics output protocol");

    let gop_mode = unsafe { &*(*gop).mode };
    let gop_info = unsafe { &*gop_mode.info };

    let boot_info = boot_info_addr as *mut boot_info::BootInfo;
    unsafe {
        (*boot_info).framebuffer.addr = gop_mode.frame_buffer_base;
        (*boot_info).framebuffer.size = gop_mode.frame_buffer_size;
        (*boot_info).framebuffer.width = gop_info.horizontal_resolution;
        (*boot_info).framebuffer.height = gop_info.vertical_resolution;
        (*boot_info).framebuffer.stride = gop_info.pixels_per_scan_line;
        (*boot_info).framebuffer.pixel_format = gop_info.pixel_format;
    }

    // 2. Final Memory Map Retrieval (must match map_key used for ExitBootServices)
    logger::log(
        st,
        0x0F,
        "boot: getting memory map and exiting boot services",
    );
    logger::log(st, 0x0B, " [ info ]\r\n");

    let status = unsafe {
        (bt.get_memory_map)(
            &mut map_size,
            memory_map,
            &mut map_key,
            &mut desc_size,
            &mut desc_ver,
        )
    };
    if status != EFI_SUCCESS {
        logger::log(st, 0x0C, "boot: failed to retrieve final memory map");
        halt();
    }

    // 3. Convert and Copy Memory Map into BootInfo
    let entries_ptr = (boot_info_addr + core::mem::size_of::<boot_info::BootInfo>() as u64)
        as *mut boot_info::MemoryMapEntry;
    let uefi_entries_count = map_size / desc_size;
    let mut converted_count = 0;

    for i in 0..uefi_entries_count {
        let uefi_entry = unsafe {
            &*((memory_map as usize + i * desc_size) as *const uefi::EfiMemoryDescriptor)
        };
        let m_type = match uefi_entry.memory_type {
            uefi::EfiMemoryType::ConventionalMemory
            | uefi::EfiMemoryType::BootServicesCode
            | uefi::EfiMemoryType::BootServicesData
            | uefi::EfiMemoryType::LoaderCode
            | uefi::EfiMemoryType::LoaderData => boot_info::MemoryType::Free,
            uefi::EfiMemoryType::OsvKernelCode => boot_info::MemoryType::KernelCode,
            uefi::EfiMemoryType::OsvKernelData => boot_info::MemoryType::KernelData,
            uefi::EfiMemoryType::OsvKernelStack => boot_info::MemoryType::KernelStack,
            _ => boot_info::MemoryType::Reserved,
        };

        unsafe {
            *entries_ptr.add(converted_count) = boot_info::MemoryMapEntry {
                mem_type: m_type,
                start: uefi_entry.physical_start,
                n_pages: uefi_entry.number_of_pages,
            };
        }

        converted_count += 1;
    }

    unsafe {
        (*boot_info).physical_memory_map.len = converted_count;
        (*boot_info).physical_memory_map.entries = entries_ptr;
    }

    // POINT OF NO RETURN
    let status = unsafe { (bt.exit_boot_services)(image_handle, map_key) };
    if status != EFI_SUCCESS {
        logger::log(st, 0x0C, "boot: failed to exit boot services");
        halt();
    }

    // Align stack to 16 bytes for ABI compliance
    let stack_top = (kernel_stack_base + KERNEL_STACK_SIZE as u64) & !0xF;
    let entry_point = elf_hdr.e_entry + relocate_delta;

    unsafe {
        core::arch::asm!(
            "mov rsp, {stack}",     // Set the new kernel stack
            "jmp {entry}",          // Jump to kernel entry point
            stack = in(reg) stack_top,
            entry = in(reg) entry_point,
            in("rdi") boot_info_addr,
            options(noreturn)
        );
    };
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    halt();
}

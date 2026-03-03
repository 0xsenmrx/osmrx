use crate::logger::{UefiWriter, log};
use crate::uefi::{EFI_SUCCESS, EfiFileInfo, EfiStatus, EfiSystemTable, efi_is_error};
use core::fmt::Write;

#[inline(always)]
pub fn halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Checks UEFI status and hangs on error with a red message
pub fn check_status(st: &EfiSystemTable, status: EfiStatus, msg: &str) {
    if status != EFI_SUCCESS && efi_is_error(status) {
        log(st, 0x0C, msg); // 0x0C is Red
        log(st, 0x0C, " [ fail ]\r\n");
        halt();
    } else {
        log(st, 0x0F, msg); // 0x0F is White
        log(st, 0x0A, " [  ok  ]\r\n"); // 0x0A is Light Green
    }
}

/// Helper function to display kernel file information
pub fn print_kernel_info(st: &EfiSystemTable, info: &EfiFileInfo) {
    let mut writer = UefiWriter { proto: st.con_out };
    let cyan = 0x0B;
    let white = 0x0F;

    log(st, white, "    boot: kernel file size: ");
    let _ = write!(writer, "{} bytes", info.file_size);
    log(st, cyan, " [ info ]\r\n");

    log(st, white, "    boot: disk storage size: ");
    let _ = write!(writer, "{} bytes", info.physical_size);
    log(st, cyan, " [ info ]\r\n");

    log(st, white, "    boot: file attributes: ");
    let _ = write!(writer, "{:#x}", info.attribute);
    log(st, cyan, " [ info ]\r\n");

    log(st, white, "    boot: last modified: ");
    let _ = write!(
        writer,
        "{:04}-{:02}-{:02} {:02}:{:02}",
        info.modification_time.year,
        info.modification_time.month,
        info.modification_time.day,
        info.modification_time.hour,
        info.modification_time.minute
    );
    log(st, cyan, " [ info ]\r\n");
}

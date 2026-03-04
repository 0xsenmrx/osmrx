#![no_std]
#![no_main]

mod boot_info;
mod pmm;

use boot_info::BootInfo;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { core::hint::spin_loop(); }
}

fn fill_screen(fb: &boot_info::Framebuffer, color: u32) {
    let fb_ptr = fb.addr as *mut u32;
    let size = (fb.stride * fb.height) as usize;
    for i in 0..size {
        unsafe { *fb_ptr.add(i) = color; }
    }
}

fn font_bitmap(c: u8) -> [u8; 16] {
    match c {
        b'O' => [0,0, 0b00111100,0b01100110,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b01100110,0b00111100, 0,0,0],
        b'S' => [0,0, 0b00111110,0b01100011,0b11000011,0b11000000,0b01111100,0b00000110,0b00000011,0b11000011,0b11000011,0b01100110,0b00111100, 0,0,0],
        b'M' => [0,0, 0b11000011,0b11100111,0b11111111,0b11011011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011, 0,0,0],
        b'K' => [0,0, 0b11000011,0b11000110,0b11001100,0b11011000,0b11110000,0b11100000,0b11110000,0b11011000,0b11001100,0b11000110,0b11000011, 0,0,0],
        b'R' => [0,0, 0b11111110,0b11000111,0b11000011,0b11000011,0b11000111,0b11111110,0b11001100,0b11000110,0b11000011,0b11000011,0b11000011, 0,0,0],
        b'X' => [0,0, 0b11000011,0b11000011,0b01100110,0b01100110,0b00111100,0b00011000,0b00111100,0b01100110,0b01100110,0b11000011,0b11000011, 0,0,0],
        b'P' => [0,0, 0b11111100,0b11000110,0b11000011,0b11000011,0b11000110,0b11111100,0b11000000,0b11000000,0b11000000,0b11000000,0b11000000, 0,0,0],
        b'U' => [0,0, 0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b01100110,0b00111100, 0,0,0],
        b'C' => [0,0, 0b00111100,0b01100110,0b11000011,0b11000000,0b11000000,0b11000000,0b11000000,0b11000000,0b11000011,0b01100110,0b00111100, 0,0,0],
        b'E' => [0,0, 0b11111111,0b11000000,0b11000000,0b11000000,0b11111110,0b11000000,0b11000000,0b11000000,0b11000000,0b11000000,0b11111111, 0,0,0],
        _ => [0; 16],
    }
}

fn draw_scaled_string(fb: &boot_info::Framebuffer, x0: usize, y0: usize, s: &[u8], scale: usize, color: u32) {
    let fb_ptr = fb.addr as *mut u32;
    let stride = fb.stride as usize;
    for (i, &ch) in s.iter().enumerate() {
        let bitmap = font_bitmap(ch);
        let char_offset_x = x0 + (i * 9 * scale);
        for (row_idx, row_bits) in bitmap.iter().enumerate() {
            for col_idx in 0..8 {
                if row_bits & (1 << (7 - col_idx)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = char_offset_x + col_idx * scale + sx;
                            let py = y0 + row_idx * scale + sy;
                            unsafe { *fb_ptr.add(py * stride + px) = color; }
                        }
                    }
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    let bi = unsafe { &*boot_info };
    
    // STEP 0: Immediate Feedback. If you don't see Grey, the kernel didn't start.
    fill_screen(&bi.framebuffer, 0x555555); 

    // STEP 1: Initialize PMM
    unsafe { pmm::init(bi) };
    // Blue: PMM init completed
    fill_screen(&bi.framebuffer, 0x0000FF);

    // STEP 2: Record initial free pages
    let initial_free = unsafe { pmm::total_free_pages() };

    // STEP 3: Allocate some pages
    let a1 = unsafe { pmm::alloc_pages(4) };   // 4 pages
    let a2 = unsafe { pmm::alloc_pages(16) };  // 16 pages
    let a3 = unsafe { pmm::alloc_pages(32) };  // 32 pages

    // Yellow: allocations done
    fill_screen(&bi.framebuffer, 0xFFFF00);

    // STEP 4: Free them back if allocations succeeded
    if let Some(addr) = a1 {
        unsafe { pmm::free_pages(addr, 4) };
    }
    if let Some(addr) = a2 {
        unsafe { pmm::free_pages(addr, 16) };
    }
    if let Some(addr) = a3 {
        unsafe { pmm::free_pages(addr, 32) };
    }

    // STEP 5: Verify final free pages
    let final_free = unsafe { pmm::total_free_pages() };

    if initial_free > 0 && final_free == initial_free {
        // Dark green: PMM alloc/free works and accounting is consistent
        fill_screen(&bi.framebuffer, 0x003300);
        draw_scaled_string(&bi.framebuffer, 80, 80, b"OSMRX PMM OK", 6, 0x00FF00);
    } else {
        // Red: mismatch in accounting or no free memory
        fill_screen(&bi.framebuffer, 0xFF0000);
        draw_scaled_string(&bi.framebuffer, 80, 80, b"PMM ERROR", 6, 0xFFFFFF);
    }

    loop { core::hint::spin_loop(); }
}

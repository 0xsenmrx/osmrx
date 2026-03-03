#![no_std]
#![no_main]

mod boot_info;
use boot_info::BootInfo;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { core::hint::spin_loop(); }
}

/// Normalized Font: Every character now exists strictly between Row 2 and Row 12.
fn font_bitmap(c: u8) -> [u8; 16] {
    match c {
        // Rows:  0 1  2(TOP)    3          4          5          6          7          8          9          10         11         12(BOT)   13 14 15
        b'O' => [0,0, 0b00111100,0b01100110,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b01100110,0b00111100, 0,0,0],
        b'S' => [0,0, 0b00111110,0b01100011,0b11000011,0b11000000,0b01111100,0b00000110,0b00000011,0b11000011,0b11000011,0b01100110,0b00111100, 0,0,0],
        b'M' => [0,0, 0b11000011,0b11100111,0b11111111,0b11011011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011,0b11000011, 0,0,0],
        b'R' => [0,0, 0b11111110,0b11000111,0b11000011,0b11000011,0b11000111,0b11111110,0b11001100,0b11000110,0b11000011,0b11000011,0b11000011, 0,0,0],
        b'X' => [0,0, 0b11000011,0b11000011,0b01100110,0b01100110,0b00111100,0b00011000,0b00111100,0b01100110,0b01100110,0b11000011,0b11000011, 0,0,0],
        _ => [0; 16],
    }
}

fn draw_scaled_string(
    fb_ptr: *mut u32,
    stride: usize,
    x0: usize,
    y0: usize,
    s: &[u8],
    scale: usize,
    spacing: usize,
    color: u32,
) {
    for (i, &ch) in s.iter().enumerate() {
        let bitmap = font_bitmap(ch);
        let char_offset_x = x0 + (i * (8 + spacing) * scale);
        
        for (row_idx, row_bits) in bitmap.iter().enumerate() {
            for col_idx in 0..8 {
                if row_bits & (1 << (7 - col_idx)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = char_offset_x + (col_idx * scale) + sx;
                            let py = y0 + (row_idx * scale) + sy;
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
    let fb = unsafe { &(*boot_info).framebuffer };
    let fb_ptr = fb.addr as *mut u32;
    let width = fb.width as usize;
    let height = fb.height as usize;
    let stride = fb.stride as usize;

    // Background Gradient
    for y in 0..height {
        let blue = (60 - (y * 60 / height)) as u32;
        for x in 0..width {
            unsafe { *fb_ptr.add(y * stride + x) = blue; }
        }
    }

    let text = b"OSMRX";
    let scale = 12; 
    let spacing = 3; // Even more breathing room looks cleaner
    
    let char_w = (8 + spacing) * scale;
    let char_h = 16 * scale;
    let total_w = text.len() * char_w - (spacing * scale);

    let start_x = (width - total_w) / 2;
    let start_y = (height - char_h) / 2;

    // Drop Shadow (Solid Black-ish)
    draw_scaled_string(fb_ptr, stride, start_x + 8, start_y + 8, text, scale, spacing, 0x050505);
    
    // Main Text (Vibrant Cyan)
    draw_scaled_string(fb_ptr, stride, start_x, start_y, text, scale, spacing, 0x00FFFF);

    // Bottom Decorative Bar
    let line_y = start_y + char_h + (2 * scale);
    let line_w = total_w;
    let line_x = start_x;
    for x in 0..line_w {
        for thickness in 0..4 {
            unsafe { *fb_ptr.add((line_y + thickness) * stride + (line_x + x)) = 0x00AABB; }
        }
    }

    loop { core::hint::spin_loop(); }
}
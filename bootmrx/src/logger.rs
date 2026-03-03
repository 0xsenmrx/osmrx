use crate::uefi::{EfiSystemTable, SimpleTextOutputProtocol};
use core::fmt::{Result, Write};

/// A simple wrapper to make UEFI console compatible with Rust's write! macro
pub struct UefiWriter {
    pub proto: *mut SimpleTextOutputProtocol,
}

impl Write for UefiWriter {
    fn write_str(&mut self, s: &str) -> Result {
        let mut buffer = [0u16; 128];
        let mut i = 0;

        for c in s.chars() {
            if c == '\n' {
                buffer[i] = '\r' as u16;
                i += 1;
            }

            let mut utf16_buf = [0u16; 2];
            // Fix 2: Changed &u to u
            for u in c.encode_utf16(&mut utf16_buf) {
                if i >= buffer.len() - 2 {
                    buffer[i] = 0;
                    unsafe {
                        ((*self.proto).output_string)(self.proto, buffer.as_ptr());
                    }
                    i = 0;
                }
                buffer[i] = *u; // Dereference the u16
                i += 1;
            }
        }

        if i > 0 {
            buffer[i] = 0;
            unsafe {
                ((*self.proto).output_string)(self.proto, buffer.as_ptr());
            }
        }
        Ok(())
    }
}

/// Core logging function
pub fn log(st: &EfiSystemTable, color: usize, msg: &str) {
    let con_out = st.con_out;
    unsafe {
        // Set the requested color
        ((*con_out).set_attribute)(con_out, color);

        // Write the message
        let mut writer = UefiWriter { proto: con_out };
        let _ = writer.write_str(msg);

        // Reset to White (0x0F) immediately after
        ((*con_out).set_attribute)(con_out, 0x0F);
    }
}

extern crate alloc;

use alloc::vec::Vec;

use uefi::proto::console::gop::BltPixel;

pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<BltPixel>,
}

pub fn parse(data: &[u8]) -> Result<Bitmap, &'static str> {
    if data.len() < 54 {
        return Err("too small for BMP");
    }
    if data[0] != b'B' || data[1] != b'M' {
        return Err("not a BMP file");
    }

    let pixel_offset = read_u32(data, 10) as usize;
    let width = read_i32(data, 18);
    let height = read_i32(data, 22);
    let bpp = read_u16(data, 28) as usize;
    let compression = read_u32(data, 30);

    if compression != 0 {
        return Err("compressed BMP not supported");
    }
    if bpp != 24 && bpp != 32 {
        return Err("only 24/32-bit BMP supported");
    }

    let abs_w = width.unsigned_abs() as usize;
    let abs_h = height.unsigned_abs() as usize;
    let bottom_up = height > 0;
    let bytes_per_px = bpp / 8;
    let row_stride = ((abs_w * bytes_per_px + 3) / 4) * 4;

    let mut pixels = Vec::with_capacity(abs_w * abs_h);

    for row in 0..abs_h {
        let src_row = if bottom_up { abs_h - 1 - row } else { row };
        let row_off = pixel_offset + src_row * row_stride;

        for col in 0..abs_w {
            let off = row_off + col * bytes_per_px;
            if off + bytes_per_px > data.len() {
                return Err("BMP pixel data truncated");
            }
            let b = data[off];
            let g = data[off + 1];
            let r = data[off + 2];
            pixels.push(BltPixel::new(r, g, b));
        }
    }

    Ok(Bitmap {
        width: abs_w,
        height: abs_h,
        pixels,
    })
}

fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn read_i32(data: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

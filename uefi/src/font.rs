include!(concat!(env!("OUT_DIR"), "/font_data.rs"));

pub fn glyph(ch: u8) -> &'static [u8] {
    if ch >= 0x20 && ch <= 0x7E {
        &FONT_DATA[(ch - 0x20) as usize]
    } else {
        &FALLBACK
    }
}

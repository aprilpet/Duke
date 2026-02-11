use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::{
    env,
    fs,
};

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest = out.join("font_data.rs");

    let candidates = [
        PathBuf::from(&manifest).join("cozette.bdf"),
        PathBuf::from(&manifest).join("..").join("cozette.bdf"),
    ];

    let bdf_path = candidates
        .iter()
        .find(|p| p.exists())
        .expect("cozette.bdf not found place it in the project root");

    println!("cargo:rerun-if-changed={}", bdf_path.display());
    eprintln!("build.rs: Using Cozette font from {}", bdf_path.display());
    let src = fs::read_to_string(bdf_path).expect("read BDF");
    generate_from_bdf(&src, &dest);
}

struct BdfGlyph {
    encoding: u32,
    bbx_h: i32,
    bbx_xoff: i32,
    bbx_yoff: i32,
    bitmap: Vec<u16>,
}

struct BdfFont {
    font_ascent: i32,
    font_descent: i32,
    dwidth: i32,
    glyphs: Vec<BdfGlyph>,
}

fn parse_bdf(content: &str) -> BdfFont {
    let mut font_ascent: i32 = 10;
    let mut font_descent: i32 = 3;
    let mut default_dwidth: i32 = 6;
    let mut glyphs = Vec::new();
    let mut lines = content.lines();

    while let Some(line) = lines.next() {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("FONT_ASCENT ") {
            if let Ok(v) = rest.trim().parse::<i32>() {
                font_ascent = v;
            }
        }
        if let Some(rest) = line.strip_prefix("FONT_DESCENT ") {
            if let Ok(v) = rest.trim().parse::<i32>() {
                font_descent = v;
            }
        }

        if line.starts_with("STARTCHAR") {
            let mut encoding: Option<u32> = None;
            let mut bh: i32 = 0;
            let mut bxo: i32 = 0;
            let mut byo: i32 = 0;
            let mut glyph_dw: Option<i32> = None;
            let mut bitmap_rows: Vec<u16> = Vec::new();
            let mut in_bitmap = false;

            for gline in lines.by_ref() {
                let gline = gline.trim();
                if gline == "ENDCHAR" {
                    break;
                }

                if let Some(rest) = gline.strip_prefix("ENCODING ") {
                    encoding = rest.trim().parse().ok();
                } else if let Some(rest) = gline.strip_prefix("DWIDTH ") {
                    let p: Vec<i32> = rest
                        .split_whitespace()
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if let Some(&w) = p.first() {
                        glyph_dw = Some(w);
                    }
                } else if let Some(rest) = gline.strip_prefix("BBX ") {
                    let p: Vec<i32> = rest
                        .split_whitespace()
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if p.len() >= 4 {
                        bh = p[1];
                        bxo = p[2];
                        byo = p[3];
                    }
                } else if gline == "BITMAP" {
                    in_bitmap = true;
                } else if in_bitmap {
                    let val = if gline.len() >= 4 {
                        u16::from_str_radix(&gline[..4], 16).unwrap_or(0)
                    } else if gline.len() >= 2 {
                        (u16::from_str_radix(&gline[..2], 16).unwrap_or(0)) << 8
                    } else {
                        0
                    };
                    bitmap_rows.push(val);
                }
            }

            if let (Some(enc), Some(dw)) = (encoding, glyph_dw) {
                if enc == 0x20 {
                    default_dwidth = dw;
                }
            }

            if let Some(enc) = encoding {
                glyphs.push(BdfGlyph {
                    encoding: enc,
                    bbx_h: bh,
                    bbx_xoff: bxo,
                    bbx_yoff: byo,
                    bitmap: bitmap_rows,
                });
            }
        }
    }

    BdfFont {
        font_ascent,
        font_descent,
        dwidth: default_dwidth,
        glyphs,
    }
}

fn generate_from_bdf(content: &str, dest: &PathBuf) {
    let font = parse_bdf(content);

    let cell_w = font.dwidth as usize;
    let cell_h = (font.font_ascent + font.font_descent) as usize;

    eprintln!(
        "build.rs: cell {}x{}, ascent={}, descent={}",
        cell_w, cell_h, font.font_ascent, font.font_descent
    );

    let map: HashMap<u32, &BdfGlyph> = font.glyphs.iter().map(|g| (g.encoding, g)).collect();

    let mut cells: Vec<Vec<u16>> = Vec::new();

    for ch in 0x20u32..=0x7Eu32 {
        let mut cell = vec![0u16; cell_h];

        if let Some(g) = map.get(&ch) {
            let top_row = font.font_ascent - g.bbx_yoff - g.bbx_h;

            for (i, &val) in g.bitmap.iter().enumerate() {
                let r = top_row + i as i32;
                if r < 0 || r >= cell_h as i32 {
                    continue;
                }

                let shifted = if g.bbx_xoff > 0 {
                    val >> g.bbx_xoff
                } else if g.bbx_xoff < 0 {
                    val << (-g.bbx_xoff)
                } else {
                    val
                };
                cell[r as usize] |= shifted;
            }
        }

        cells.push(cell);
    }

    let mut f = fs::File::create(dest).expect("create font_data.rs");
    writeln!(f, "// (auto-generated from cozette.bdf)").unwrap();
    writeln!(f, "// Do not edit").unwrap();
    writeln!(f).unwrap();
    writeln!(f, "pub const GLYPH_W: usize = {};", cell_w).unwrap();
    writeln!(f, "pub const GLYPH_H: usize = {};", cell_h).unwrap();
    writeln!(f).unwrap();
    writeln!(f, "pub(super) static FONT_DATA: [[u16; {}]; 95] = [", cell_h).unwrap();

    for (i, cell) in cells.iter().enumerate() {
        let ch = (0x20 + i) as u8 as char;
        let label = if ch == '\\' {
            String::from("backslash")
        } else if ch == '\'' {
            String::from("apostrophe")
        } else {
            format!("{}", ch)
        };
        write!(f, "    // 0x{:02X}  {}\n    [", 0x20 + i, label).unwrap();
        for (j, b) in cell.iter().enumerate() {
            if j > 0 {
                write!(f, ", ").unwrap();
            }
            write!(f, "0x{:04X}", b).unwrap();
        }
        writeln!(f, "],").unwrap();
    }

    writeln!(f, "];").unwrap();
    writeln!(f).unwrap();
    write!(f, "pub(super) static FALLBACK: [u16; {}] = [", cell_h).unwrap();
    for i in 0..cell_h {
        if i > 0 {
            write!(f, ", ").unwrap();
        }
        write!(f, "0xFFFF").unwrap();
    }
    writeln!(f, "];").unwrap();
}

#![no_main]
#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::time::Duration;

use log::info;
use uefi::boot::SearchType;
use uefi::fs::FileSystem;
use uefi::prelude::*;
use uefi::proto::BootPolicy;
use uefi::proto::console::gop::{
    BltOp,
    BltPixel,
    BltRegion,
    GraphicsOutput,
};
use uefi::proto::console::text::{
    Key,
    ScanCode,
};
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::build::{
    self as dp_build,
    DevicePathBuilder,
};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::{
    CStr16,
    CString16,
    Handle,
    boot,
};

mod bmp;
mod font;
mod logger;

use jvm::interpreter::{
    Vm,
    jvm_value_to_string,
};
use jvm::native::NativeBridge;
use shared::classfile;
use shared::types::{
    JvmError,
    JvmValue,
};
use shared::zip::ZipArchive;

struct BootEntry {
    name: String,
    path: String,
    device: Handle,
}

struct UefiNatives {
    boot_entries: Vec<BootEntry>,
    gop_handle: Option<Handle>,
    screen_w: usize,
    screen_h: usize,
}

impl UefiNatives {
    fn new() -> Self {
        Self {
            boot_entries: Vec::new(),
            gop_handle: None,
            screen_w: 0,
            screen_h: 0,
        }
    }

    fn discover(&mut self) -> i32 {
        self.boot_entries = discover_efi_entries();
        self.boot_entries.len() as i32
    }
}

impl NativeBridge for UefiNatives {
    fn call_native(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: &[JvmValue],
    ) -> Result<Option<JvmValue>, JvmError> {
        match (class_name, method_name) {
            (_, "print") => {
                if let Some(arg) = args.first() {
                    uefi::print!("{}", jvm_value_to_string(arg));
                }
                Ok(None)
            }
            (_, "println") => {
                if let Some(arg) = args.first() {
                    uefi::println!("{}", jvm_value_to_string(arg));
                } else {
                    uefi::println!();
                }
                Ok(None)
            }

            (_, "readKey") => loop {
                let result = uefi::system::with_stdin(|stdin| stdin.read_key());
                match result {
                    Ok(Some(Key::Printable(c))) => {
                        let ch = u16::from(c) as i32;
                        return Ok(Some(JvmValue::Int(ch)));
                    }
                    Ok(Some(Key::Special(scan))) => {
                        let code = if scan == ScanCode::UP {
                            -1
                        } else if scan == ScanCode::DOWN {
                            -2
                        } else if scan == ScanCode::ESCAPE {
                            -3
                        } else if scan == ScanCode::HOME {
                            -4
                        } else if scan == ScanCode::END {
                            -5
                        } else if scan == ScanCode::RIGHT {
                            -6
                        } else if scan == ScanCode::LEFT {
                            -7
                        } else {
                            continue;
                        };
                        return Ok(Some(JvmValue::Int(code)));
                    }
                    _ => {
                        boot::stall(Duration::from_millis(50));
                    }
                }
            },

            (_, "chainload") => {
                if let Some(JvmValue::Int(idx)) = args.first() {
                    if let Some(entry) = self.boot_entries.get(*idx as usize) {
                        do_chainload(entry.device, &entry.path)?;
                    }
                } else if let Some(JvmValue::StringRef(path)) = args.first() {
                    let loaded_image =
                        boot::open_protocol_exclusive::<LoadedImage>(boot::image_handle())
                            .map_err(|e| JvmError::IoError(format!("LoadedImage: {:?}", e)))?;
                    let device_handle = loaded_image
                        .device()
                        .ok_or_else(|| JvmError::IoError(String::from("no device handle")))?;
                    drop(loaded_image);
                    do_chainload(device_handle, path)?;
                }
                Ok(None)
            }

            (_, "stall") => {
                if let Some(JvmValue::Int(ms)) = args.first() {
                    boot::stall(Duration::from_millis(*ms as u64));
                }
                Ok(None)
            }

            (_, "readFile") => {
                if let Some(JvmValue::StringRef(path)) = args.first() {
                    match read_esp_file(path) {
                        Ok(_data) => Ok(Some(JvmValue::ArrayRef(0))),
                        Err(_) => Ok(Some(JvmValue::Null)),
                    }
                } else {
                    Ok(Some(JvmValue::Null))
                }
            }

            (_, "listDirectory") => {
                if let Some(JvmValue::StringRef(path)) = args.first() {
                    match list_esp_directory(path) {
                        Ok(names) => Ok(Some(JvmValue::Int(names.len() as i32))),
                        Err(_) => Ok(Some(JvmValue::Null)),
                    }
                } else {
                    Ok(Some(JvmValue::Null))
                }
            }

            (_, "discoverEntries") => {
                let count = self.discover();
                Ok(Some(JvmValue::Int(count)))
            }

            (_, "entryName") => {
                if let Some(JvmValue::Int(idx)) = args.first() {
                    let name = self
                        .boot_entries
                        .get(*idx as usize)
                        .map(|e| e.name.clone())
                        .unwrap_or_else(|| String::from("?"));
                    Ok(Some(JvmValue::StringRef(name)))
                } else {
                    Ok(Some(JvmValue::StringRef(String::from("?"))))
                }
            }

            (_, "entryPath") => {
                if let Some(JvmValue::Int(idx)) = args.first() {
                    let path = self
                        .boot_entries
                        .get(*idx as usize)
                        .map(|e| e.path.clone())
                        .unwrap_or_else(|| String::from(""));
                    Ok(Some(JvmValue::StringRef(path)))
                } else {
                    Ok(Some(JvmValue::StringRef(String::from(""))))
                }
            }

            (_, "chainloadEntry") => {
                if let Some(JvmValue::Int(idx)) = args.first() {
                    if let Some(entry) = self.boot_entries.get(*idx as usize) {
                        do_chainload(entry.device, &entry.path)?;
                    }
                }
                Ok(None)
            }

            (_, "initGraphics") => {
                let handles =
                    boot::locate_handle_buffer(SearchType::from_proto::<GraphicsOutput>())
                        .map_err(|e| JvmError::IoError(format!("GOP locate: {:?}", e)));

                match handles {
                    Ok(buf) => {
                        let h = buf[0];
                        match boot::open_protocol_exclusive::<GraphicsOutput>(h) {
                            Ok(gop) => {
                                let (w, h_res) = gop.current_mode_info().resolution();
                                self.screen_w = w;
                                self.screen_h = h_res;
                                self.gop_handle = Some(h);
                                drop(gop);
                                Ok(Some(JvmValue::Int(1)))
                            }
                            Err(_) => Ok(Some(JvmValue::Int(0))),
                        }
                    }
                    Err(_) => Ok(Some(JvmValue::Int(0))),
                }
            }

            (_, "screenWidth") => Ok(Some(JvmValue::Int(self.screen_w as i32))),

            (_, "screenHeight") => Ok(Some(JvmValue::Int(self.screen_h as i32))),

            (_, "fontWidth") => Ok(Some(JvmValue::Int(font::GLYPH_W as i32))),

            (_, "fontHeight") => Ok(Some(JvmValue::Int(font::GLYPH_H as i32))),

            (_, "clearScreen") => {
                if let Some(JvmValue::Int(color)) = args.first() {
                    let (r, g, b) = unpack_rgb(*color);
                    if let Some(h) = self.gop_handle {
                        if let Ok(mut gop) = boot::open_protocol_exclusive::<GraphicsOutput>(h) {
                            let _ = gop.blt(BltOp::VideoFill {
                                color: BltPixel::new(r, g, b),
                                dest: (0, 0),
                                dims: (self.screen_w, self.screen_h),
                            });
                        }
                    }
                }
                Ok(None)
            }

            (_, "fillRect") => {
                if let (
                    Some(JvmValue::Int(x)),
                    Some(JvmValue::Int(y)),
                    Some(JvmValue::Int(w)),
                    Some(JvmValue::Int(h)),
                    Some(JvmValue::Int(color)),
                ) = (
                    args.get(0),
                    args.get(1),
                    args.get(2),
                    args.get(3),
                    args.get(4),
                ) {
                    let (cr, cg, cb) = unpack_rgb(*color);
                    if let Some(gh) = self.gop_handle {
                        if let Ok(mut gop) = boot::open_protocol_exclusive::<GraphicsOutput>(gh) {
                            let _ = gop.blt(BltOp::VideoFill {
                                color: BltPixel::new(cr, cg, cb),
                                dest: (*x as usize, *y as usize),
                                dims: (*w as usize, *h as usize),
                            });
                        }
                    }
                }
                Ok(None)
            }

            (_, "drawText") => {
                if let (
                    Some(JvmValue::StringRef(text)),
                    Some(JvmValue::Int(x)),
                    Some(JvmValue::Int(y)),
                    Some(JvmValue::Int(fg)),
                    Some(JvmValue::Int(scale)),
                ) = (
                    args.get(0),
                    args.get(1),
                    args.get(2),
                    args.get(3),
                    args.get(4),
                ) {
                    let (fr, fga, fb) = unpack_rgb(*fg);
                    let sc = *scale as usize;
                    draw_text_gop(
                        self.gop_handle,
                        text,
                        *x as usize,
                        *y as usize,
                        BltPixel::new(fr, fga, fb),
                        sc,
                    )?;
                }
                Ok(None)
            }

            (_, "drawImage") => {
                if let (
                    Some(JvmValue::StringRef(path)),
                    Some(JvmValue::Int(x)),
                    Some(JvmValue::Int(y)),
                ) = (args.get(0), args.get(1), args.get(2))
                {
                    if let Ok(data) = read_esp_file(path) {
                        if let Ok(bitmap) = bmp::parse(&data) {
                            if let Some(h) = self.gop_handle {
                                if let Ok(mut gop) =
                                    boot::open_protocol_exclusive::<GraphicsOutput>(h)
                                {
                                    let _ = gop.blt(BltOp::BufferToVideo {
                                        buffer: &bitmap.pixels,
                                        src: BltRegion::Full,
                                        dest: (*x as usize, *y as usize),
                                        dims: (bitmap.width, bitmap.height),
                                    });
                                }
                            }
                        }
                    }
                }
                Ok(None)
            }

            (_, "imageWidth") | (_, "imageHeight") => {
                if let Some(JvmValue::StringRef(path)) = args.first() {
                    if let Ok(data) = read_esp_file(path) {
                        if let Ok(bm) = bmp::parse(&data) {
                            let val = if method_name == "imageWidth" {
                                bm.width
                            } else {
                                bm.height
                            };
                            return Ok(Some(JvmValue::Int(val as i32)));
                        }
                    }
                    Ok(Some(JvmValue::Int(0)))
                } else {
                    Ok(Some(JvmValue::Int(0)))
                }
            }

            _ => {
                uefi::println!(
                    "[duke] unhandled native: {}::{}{}",
                    class_name,
                    method_name,
                    descriptor,
                );
                Ok(None)
            }
        }
    }
}

fn has_efi_extension(name: &str) -> bool {
    name.len() >= 5 && name[name.len() - 4..].eq_ignore_ascii_case(".efi")
}

fn capitalize(s: &str) -> String {
    let mut bytes = Vec::from(s.as_bytes());
    if let Some(first) = bytes.first_mut() {
        first.make_ascii_uppercase();
    }
    String::from_utf8(bytes).unwrap_or_else(|_| String::from(s))
}

fn is_utility_efi(name: &str) -> bool {
    const SKIP: &[&str] = &[
        "mmx64.efi",
        "mmia32.efi",
        "mmaa64.efi",
        "fwupx64.efi",
        "fwupia32.efi",
        "fwupaa64.efi",
        "fbx64.efi",
        "fbia32.efi",
        "fbaa64.efi",
        "memtest86.efi",
        "memtest86plus.efi",
        "duke.efi",
    ];
    SKIP.iter().any(|s| name.eq_ignore_ascii_case(s))
}

fn uki_display_name(filename: &str) -> String {
    let stem = match filename.rfind('.') {
        Some(pos) => &filename[..pos],
        None => filename,
    };
    let cleaned: String = stem
        .chars()
        .map(|c| match c {
            '-' | '_' => ' ',
            _ => c,
        })
        .collect();
    capitalize(cleaned.trim())
}

fn discover_efi_entries() -> Vec<BootEntry> {
    const KNOWN_LOADERS: &[&str] = &[
        "shimx64.efi",
        "shimia32.efi",
        "shimaa64.efi",
        "grubx64.efi",
        "grubia32.efi",
        "grubaa64.efi",
        "systemd-bootx64.efi",
        "systemd-bootia32.efi",
        "systemd-bootaa64.efi",
        "refind_x64.efi",
        "refind_ia32.efi",
        "refind_aa64.efi",
        "vmlinuz.efi",
        "bootmgfw.efi",
        "bootx64.efi",
        "bootia32.efi",
        "bootaa64.efi",
    ];

    let mut entries = Vec::new();

    let handles: Vec<Handle> =
        match boot::locate_handle_buffer(SearchType::from_proto::<SimpleFileSystem>()) {
            Ok(buf) => buf.to_vec(),
            Err(_) => return entries,
        };

    for handle in handles {
        let Ok(sfs) = boot::open_protocol_exclusive::<SimpleFileSystem>(handle) else {
            continue;
        };
        let mut fs = FileSystem::new(sfs);

        let vendor_dirs: Vec<String> = match fs.read_dir(uefi::cstr16!("\\EFI")) {
            Ok(iter) => iter
                .filter_map(|r| r.ok())
                .filter(|info| info.is_directory())
                .map(|info| format!("{}", info.file_name()))
                .collect(),
            Err(_) => continue,
        };

        scan_esp(&mut fs, handle, &vendor_dirs, KNOWN_LOADERS, &mut entries);
    }

    let mut seen = Vec::new();
    entries.retain(|e| {
        let key = e.name.clone();
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });

    entries
}

fn scan_esp(
    fs: &mut FileSystem,
    device: Handle,
    vendor_dirs: &[String],
    known_loaders: &[&str],
    entries: &mut Vec<BootEntry>,
) {
    for vendor in vendor_dirs {
        if vendor == "." || vendor == ".." {
            continue;
        }
        if vendor.eq_ignore_ascii_case("duke") {
            continue;
        }

        let is_boot_dir = vendor.eq_ignore_ascii_case("boot");
        let is_linux_dir = vendor.eq_ignore_ascii_case("linux");

        let dir_str = format!("\\EFI\\{}", vendor);
        let Ok(dir_path) = CString16::try_from(dir_str.as_str()) else {
            continue;
        };

        let contents: Vec<(String, bool)> = match fs.read_dir(&*dir_path) {
            Ok(iter) => iter
                .filter_map(|r| r.ok())
                .map(|info| (format!("{}", info.file_name()), info.is_directory()))
                .collect(),
            Err(_) => continue,
        };

        if is_linux_dir {
            for (fname, is_dir) in &contents {
                if fname == "." || fname == ".." || *is_dir {
                    continue;
                }
                if has_efi_extension(fname) && !is_utility_efi(fname) {
                    let entry_path = format!("\\EFI\\{}\\{}", vendor, fname);
                    entries.push(BootEntry {
                        name: format!("Linux ({})", uki_display_name(fname)),
                        path: entry_path,
                        device,
                    });
                }
            }
            continue;
        }

        let mut all_efi: Vec<(String, String)> = Vec::new();

        for (fname, is_dir) in &contents {
            if fname == "." || fname == ".." {
                continue;
            }
            if !is_dir && has_efi_extension(fname) && !is_utility_efi(fname) {
                all_efi.push((fname.clone(), format!("\\EFI\\{}\\{}", vendor, fname)));
            } else if *is_dir {
                let sub_str = format!("\\EFI\\{}\\{}", vendor, fname);
                let Ok(sub_path) = CString16::try_from(sub_str.as_str()) else {
                    continue;
                };
                let sub_files: Vec<String> = match fs.read_dir(&*sub_path) {
                    Ok(iter) => iter
                        .filter_map(|r| r.ok())
                        .filter(|info| !info.is_directory())
                        .map(|info| format!("{}", info.file_name()))
                        .collect(),
                    Err(_) => continue,
                };
                for sub_fname in &sub_files {
                    if has_efi_extension(sub_fname) && !is_utility_efi(sub_fname) {
                        all_efi.push((
                            sub_fname.clone(),
                            format!("\\EFI\\{}\\{}\\{}", vendor, fname, sub_fname),
                        ));
                    }
                }
            }
        }

        if is_boot_dir {
            if all_efi
                .iter()
                .any(|(f, _)| f.eq_ignore_ascii_case("duke.efi"))
            {
                continue;
            }
            if let Some(best) = pick_best_loader(&all_efi, known_loaders) {
                entries.push(BootEntry {
                    name: String::from("UEFI Default"),
                    path: best,
                    device,
                });
            }
            continue;
        }

        if let Some(best) = pick_best_loader(&all_efi, known_loaders) {
            entries.push(BootEntry {
                name: capitalize(vendor),
                path: best,
                device,
            });
        }
    }
}

fn pick_best_loader(candidates: &[(String, String)], known: &[&str]) -> Option<String> {
    for loader in known {
        for (fname, full_path) in candidates {
            if fname.eq_ignore_ascii_case(loader) {
                return Some(full_path.clone());
            }
        }
    }
    candidates.first().map(|(_, p)| p.clone())
}

fn unpack_rgb(color: i32) -> (u8, u8, u8) {
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    (r, g, b)
}

fn draw_text_gop(
    gop_handle: Option<Handle>,
    text: &str,
    x: usize,
    y: usize,
    fg: BltPixel,
    scale: usize,
) -> Result<(), JvmError> {
    let h =
        gop_handle.ok_or_else(|| JvmError::IoError(String::from("Graphics not initialized")))?;
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(h)
        .map_err(|e| JvmError::IoError(format!("GOP: {:?}", e)))?;

    let char_w = font::GLYPH_W * scale;
    let char_h = font::GLYPH_H * scale;
    let total_w = text.len() * char_w;
    let total_h = char_h;

    if total_w == 0 || total_h == 0 {
        return Ok(());
    }

    let mut buf = alloc::vec![BltPixel::new(0, 0, 0); total_w * total_h];
    let _ = gop.blt(BltOp::VideoToBltBuffer {
        buffer: &mut buf,
        src: (x, y),
        dest: BltRegion::Full,
        dims: (total_w, total_h),
    });

    for (ci, ch) in text.bytes().enumerate() {
        let gly = font::glyph(ch);
        for row in 0..font::GLYPH_H {
            let bits = gly[row];
            for col in 0..font::GLYPH_W {
                if bits & (0x8000 >> col) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = ci * char_w + col * scale + sx;
                            let py = row * scale + sy;
                            if px < total_w && py < total_h {
                                buf[py * total_w + px] = fg;
                            }
                        }
                    }
                }
            }
        }
    }

    let _ = gop.blt(BltOp::BufferToVideo {
        buffer: &buf,
        src: BltRegion::Full,
        dest: (x, y),
        dims: (total_w, total_h),
    });

    Ok(())
}

fn do_chainload(device_handle: Handle, path_str: &str) -> Result<(), JvmError> {
    let path_wide = CString16::try_from(path_str)
        .map_err(|_| JvmError::IoError(String::from("invalid path encoding")))?;

    let device_path = boot::open_protocol_exclusive::<DevicePath>(device_handle)
        .map_err(|e| JvmError::IoError(format!("DevicePath: {:?}", e)))?;

    let mut buf = Vec::new();
    let mut builder = DevicePathBuilder::with_vec(&mut buf);
    for node in device_path.node_iter() {
        builder = builder
            .push(&node)
            .map_err(|e| JvmError::IoError(format!("path build: {:?}", e)))?;
    }
    builder = builder
        .push(&dp_build::media::FilePath {
            path_name: &path_wide,
        })
        .map_err(|e| JvmError::IoError(format!("path build: {:?}", e)))?;
    let full_path = builder
        .finalize()
        .map_err(|e| JvmError::IoError(format!("path finalize: {:?}", e)))?;

    drop(device_path);

    let handle = boot::load_image(
        boot::image_handle(),
        boot::LoadImageSource::FromDevicePath {
            device_path: full_path,
            boot_policy: BootPolicy::ExactMatch,
        },
    )
    .map_err(|e| JvmError::IoError(format!("load_image: {:?}", e)))?;

    boot::start_image(handle).map_err(|e| JvmError::IoError(format!("start_image: {:?}", e)))?;

    Ok(())
}

#[allow(dead_code)]
fn load_file_from_esp(path: &CStr16) -> Result<Vec<u8>, JvmError> {
    let sfs = boot::get_image_file_system(boot::image_handle())
        .map_err(|e| JvmError::IoError(format!("get_image_file_system: {:?}", e)))?;

    let mut fs = FileSystem::new(sfs);
    let data = fs
        .read(path)
        .map_err(|e| JvmError::IoError(format!("read: {:?}", e)))?;
    Ok(data)
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    uefi::println!();
    uefi::println!("  Duke UEFI JVM Runtime");
    uefi::println!();

    match load_and_run() {
        Ok(()) => {
            uefi::println!();
            uefi::println!("[duke] Execution finished.");
        }
        Err(JvmError::SystemExit(code)) => {
            uefi::println!();
            uefi::println!("[duke] System.exit({})", code);
        }
        Err(e) => {
            uefi::println!();
            uefi::println!("[duke] ERROR: {}", e);
        }
    }

    boot::stall(Duration::from_secs(10));
    Status::SUCCESS
}

fn load_and_run() -> Result<(), JvmError> {
    let mut vm = Vm::new(UefiNatives::new());
    load_classes_from_esp(&mut vm)?;

    let args_arr = vm.heap.alloc_array(String::from("java/lang/String"), 0)?;
    let class_name = String::from("BootMenu");
    vm.execute(
        &class_name,
        "main",
        alloc::vec![JvmValue::ArrayRef(args_arr)],
    )?;
    Ok(())
}

fn load_classes_from_esp<N: NativeBridge>(vm: &mut Vm<N>) -> Result<(), JvmError> {
    let sfs = boot::get_image_file_system(boot::image_handle())
        .map_err(|e| JvmError::IoError(format!("get_image_file_system: {:?}", e)))?;
    let mut fs = FileSystem::new(sfs);

    let class_dir = uefi::cstr16!("\\EFI\\duke");
    let entries: Vec<String> = match fs.read_dir(class_dir) {
        Ok(iter) => iter
            .filter_map(|r| r.ok())
            .filter(|info| !info.is_directory())
            .map(|info| format!("{}", info.file_name()))
            .filter(|name| name.ends_with(".class") || name.ends_with(".jar"))
            .collect(),
        Err(_) => alloc::vec![String::from("BootMenu.class")],
    };

    for file_name in &entries {
        let full_path = format!("\\EFI\\duke\\{}", file_name);
        let Ok(wide_path) = CString16::try_from(full_path.as_str()) else {
            continue;
        };
        let data = fs
            .read(&*wide_path)
            .map_err(|e| JvmError::IoError(format!("read {}: {:?}", file_name, e)))?;

        if file_name.ends_with(".jar") {
            match ZipArchive::new(&data) {
                Ok(archive) => {
                    let class_names: Vec<String> =
                        archive.class_entries().map(|e| e.name.clone()).collect();
                    for entry_name in &class_names {
                        let entry = archive.entries().iter().find(|e| &e.name == entry_name);
                        if let Some(entry) = entry {
                            match archive.read_entry(entry) {
                                Ok(class_data) => match classfile::parse_class(&class_data) {
                                    Ok(class) => {
                                        let cn = class.class_name().unwrap_or("?");
                                        info!(
                                            "Loaded from {}: {} ({} bytes)",
                                            file_name,
                                            cn,
                                            class_data.len()
                                        );
                                        vm.load_class(class);
                                    }
                                    Err(e) => {
                                        info!(
                                            "Failed to parse {} in {}: {}",
                                            entry_name, file_name, e
                                        );
                                    }
                                },
                                Err(e) => {
                                    info!(
                                        "Failed to read {} from {}: {}",
                                        entry_name, file_name, e
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    info!("Failed to open JAR {}: {}", file_name, e);
                }
            }
        } else {
            match classfile::parse_class(&data) {
                Ok(class) => {
                    let cn = class.class_name().unwrap_or("?");
                    info!("Loaded class: {} ({} bytes)", cn, data.len());
                    vm.load_class(class);
                }
                Err(e) => {
                    info!("Failed to parse {}: {}", file_name, e);
                }
            }
        }
    }

    Ok(())
}

fn read_esp_file(path: &str) -> Result<Vec<u8>, JvmError> {
    let wide = CString16::try_from(path)
        .map_err(|_| JvmError::IoError(String::from("invalid path encoding")))?;
    let sfs = boot::get_image_file_system(boot::image_handle())
        .map_err(|e| JvmError::IoError(format!("get_image_file_system: {:?}", e)))?;
    let mut fs = FileSystem::new(sfs);
    fs.read(&*wide)
        .map_err(|e| JvmError::IoError(format!("read: {:?}", e)))
}

fn list_esp_directory(path: &str) -> Result<Vec<String>, JvmError> {
    let wide = CString16::try_from(path)
        .map_err(|_| JvmError::IoError(String::from("invalid path encoding")))?;
    let sfs = boot::get_image_file_system(boot::image_handle())
        .map_err(|e| JvmError::IoError(format!("get_image_file_system: {:?}", e)))?;
    let mut fs = FileSystem::new(sfs);
    match fs.read_dir(&*wide) {
        Ok(iter) => Ok(iter
            .filter_map(|r| r.ok())
            .map(|info| format!("{}", info.file_name()))
            .filter(|n| n != "." && n != "..")
            .collect()),
        Err(e) => Err(JvmError::IoError(format!("read_dir: {:?}", e))),
    }
}

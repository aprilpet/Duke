use alloc::string::String;
use alloc::vec::Vec;

use crate::types::JvmError;

const EOCD_SIGNATURE: u32 = 0x06054b50;
const CD_SIGNATURE: u32 = 0x02014b50;
const LOCAL_HEADER_SIGNATURE: u32 = 0x04034b50;

pub struct ZipEntry {
    pub name: String,
    pub compression_method: u16,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub local_header_offset: u32,
}

pub struct ZipArchive<'a> {
    data: &'a [u8],
    entries: Vec<ZipEntry>,
}

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    (data[offset] as u16) | ((data[offset + 1] as u16) << 8)
}

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    (data[offset] as u32)
        | ((data[offset + 1] as u32) << 8)
        | ((data[offset + 2] as u32) << 16)
        | ((data[offset + 3] as u32) << 24)
}

impl<'a> ZipArchive<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, JvmError> {
        let eocd_offset = Self::find_eocd(data)?;

        let cd_offset = read_u32_le(data, eocd_offset + 16) as usize;
        let cd_entry_count = read_u16_le(data, eocd_offset + 10) as usize;

        let mut entries = Vec::with_capacity(cd_entry_count);
        let mut pos = cd_offset;

        for _ in 0..cd_entry_count {
            if pos + 46 > data.len() {
                break;
            }
            let sig = read_u32_le(data, pos);
            if sig != CD_SIGNATURE {
                break;
            }

            let compression_method = read_u16_le(data, pos + 10);
            let compressed_size = read_u32_le(data, pos + 20);
            let uncompressed_size = read_u32_le(data, pos + 24);
            let name_len = read_u16_le(data, pos + 28) as usize;
            let extra_len = read_u16_le(data, pos + 30) as usize;
            let comment_len = read_u16_le(data, pos + 32) as usize;
            let local_header_offset = read_u32_le(data, pos + 42);

            if pos + 46 + name_len > data.len() {
                break;
            }
            let name_bytes = &data[pos + 46..pos + 46 + name_len];
            let name = core::str::from_utf8(name_bytes)
                .map(String::from)
                .unwrap_or_default();

            entries.push(ZipEntry {
                name,
                compression_method,
                compressed_size,
                uncompressed_size,
                local_header_offset,
            });

            pos += 46 + name_len + extra_len + comment_len;
        }

        Ok(Self { data, entries })
    }

    fn find_eocd(data: &[u8]) -> Result<usize, JvmError> {
        if data.len() < 22 {
            return Err(JvmError::IoError(String::from("too small for ZIP")));
        }

        let search_start = if data.len() > 22 + 65535 {
            data.len() - 22 - 65535
        } else {
            0
        };

        let mut i = data.len() - 22;
        loop {
            if read_u32_le(data, i) == EOCD_SIGNATURE {
                return Ok(i);
            }
            if i <= search_start {
                break;
            }
            i -= 1;
        }

        Err(JvmError::IoError(String::from(
            "EOCD not found — not a valid ZIP/JAR",
        )))
    }

    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    pub fn read_entry(&self, entry: &ZipEntry) -> Result<Vec<u8>, JvmError> {
        let offset = entry.local_header_offset as usize;

        if offset + 30 > self.data.len() {
            return Err(JvmError::IoError(String::from(
                "invalid local header offset",
            )));
        }

        let sig = read_u32_le(self.data, offset);
        if sig != LOCAL_HEADER_SIGNATURE {
            return Err(JvmError::IoError(String::from(
                "bad local header signature",
            )));
        }

        let name_len = read_u16_le(self.data, offset + 26) as usize;
        let extra_len = read_u16_le(self.data, offset + 28) as usize;
        let data_start = offset + 30 + name_len + extra_len;
        let data_end = data_start + entry.compressed_size as usize;

        if data_end > self.data.len() {
            return Err(JvmError::IoError(String::from(
                "entry data beyond end of file",
            )));
        }

        let compressed = &self.data[data_start..data_end];

        match entry.compression_method {
            0 => Ok(compressed.to_vec()),
            8 => self.inflate(compressed),
            m => Err(JvmError::IoError(alloc::format!(
                "unsupported ZIP compression method: {}",
                m
            ))),
        }
    }

    fn inflate(&self, compressed: &[u8]) -> Result<Vec<u8>, JvmError> {
        #[cfg(feature = "deflate")]
        {
            miniz_oxide::inflate::decompress_to_vec(compressed)
                .map_err(|e| JvmError::IoError(alloc::format!("deflate error: {:?}", e)))
        }
        #[cfg(not(feature = "deflate"))]
        {
            let _ = compressed;
            Err(JvmError::IoError(String::from(
                "DEFLATE not supported — rebuild with 'deflate' feature",
            )))
        }
    }

    pub fn class_entries(&self) -> impl Iterator<Item = &ZipEntry> {
        self.entries
            .iter()
            .filter(|e| e.name.ends_with(".class") && !e.name.contains("META-INF"))
    }
}

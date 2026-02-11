use alloc::string::String;
use alloc::vec::Vec;

use crate::types::JvmError;

#[derive(Debug, Clone)]
pub enum CpEntry {
    Unused,
    Utf8(String),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class {
        name_index: u16,
    },
    StringRef {
        string_index: u16,
    },
    Fieldref {
        class_index: u16,
        name_and_type_index: u16,
    },
    Methodref {
        class_index: u16,
        name_and_type_index: u16,
    },
    InterfaceMethodref {
        class_index: u16,
        name_and_type_index: u16,
    },
    NameAndType {
        name_index: u16,
        descriptor_index: u16,
    },
    MethodHandle {
        reference_kind: u8,
        reference_index: u16,
    },
    MethodType {
        descriptor_index: u16,
    },
    InvokeDynamic {
        bootstrap_method_attr_index: u16,
        name_and_type_index: u16,
    },
}

#[derive(Debug, Clone)]
pub struct ExceptionTableEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

#[derive(Debug, Clone)]
pub struct CodeAttribute {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: Vec<u8>,
    pub exception_table: Vec<ExceptionTableEntry>,
}

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub code: Option<CodeAttribute>,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
}

pub const ACC_PUBLIC: u16 = 0x0001;
pub const ACC_STATIC: u16 = 0x0008;
pub const ACC_NATIVE: u16 = 0x0100;

#[derive(Debug, Clone)]
pub struct BootstrapMethodEntry {
    pub method_ref: u16,
    pub arguments: Vec<u16>,
}

#[derive(Debug, Clone)]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: Vec<CpEntry>,
    pub access_flags: u16,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces: Vec<u16>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub bootstrap_methods: Vec<BootstrapMethodEntry>,
}

impl ClassFile {
    pub fn get_utf8(&self, index: u16) -> Result<&str, JvmError> {
        match self.constant_pool.get(index as usize) {
            Some(CpEntry::Utf8(s)) => Ok(s.as_str()),
            _ => Err(JvmError::ClassFormatError(alloc::format!(
                "expected Utf8 at cp#{}",
                index
            ))),
        }
    }

    pub fn get_class_name(&self, index: u16) -> Result<&str, JvmError> {
        match self.constant_pool.get(index as usize) {
            Some(CpEntry::Class { name_index }) => self.get_utf8(*name_index),
            _ => Err(JvmError::ClassFormatError(alloc::format!(
                "expected Class at cp#{}",
                index
            ))),
        }
    }

    pub fn class_name(&self) -> Result<&str, JvmError> {
        self.get_class_name(self.this_class)
    }

    pub fn super_class_name(&self) -> Option<&str> {
        if self.super_class == 0 {
            None
        } else {
            self.get_class_name(self.super_class).ok()
        }
    }

    pub fn find_method(&self, name: &str, descriptor: &str) -> Option<&MethodInfo> {
        self.methods.iter().find(|m| {
            self.get_utf8(m.name_index).ok() == Some(name)
                && self.get_utf8(m.descriptor_index).ok() == Some(descriptor)
        })
    }

    pub fn find_method_by_name(&self, name: &str) -> Option<&MethodInfo> {
        self.methods
            .iter()
            .find(|m| self.get_utf8(m.name_index).ok() == Some(name))
    }

    pub fn resolve_name_and_type(&self, index: u16) -> Result<(&str, &str), JvmError> {
        match self.constant_pool.get(index as usize) {
            Some(CpEntry::NameAndType {
                name_index,
                descriptor_index,
            }) => Ok((
                self.get_utf8(*name_index)?,
                self.get_utf8(*descriptor_index)?,
            )),
            _ => Err(JvmError::ClassFormatError(alloc::format!(
                "expected NameAndType at cp#{}",
                index
            ))),
        }
    }
}

struct ClassReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ClassReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, JvmError> {
        if self.pos >= self.data.len() {
            return Err(JvmError::ClassFormatError(String::from("unexpected EOF")));
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u16(&mut self) -> Result<u16, JvmError> {
        let hi = self.read_u8()? as u16;
        let lo = self.read_u8()? as u16;
        Ok((hi << 8) | lo)
    }

    fn read_u32(&mut self) -> Result<u32, JvmError> {
        let hi = self.read_u16()? as u32;
        let lo = self.read_u16()? as u32;
        Ok((hi << 16) | lo)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], JvmError> {
        if self.pos + len > self.data.len() {
            return Err(JvmError::ClassFormatError(String::from("unexpected EOF")));
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn skip(&mut self, n: usize) -> Result<(), JvmError> {
        if self.pos + n > self.data.len() {
            return Err(JvmError::ClassFormatError(String::from("unexpected EOF")));
        }
        self.pos += n;
        Ok(())
    }
}

pub fn parse_class(data: &[u8]) -> Result<ClassFile, JvmError> {
    let mut r = ClassReader::new(data);

    let magic = r.read_u32()?;
    if magic != 0xCAFEBABE {
        return Err(JvmError::ClassFormatError(alloc::format!(
            "bad magic: 0x{:08X}",
            magic
        )));
    }

    let minor_version = r.read_u16()?;
    let major_version = r.read_u16()?;

    let cp_count = r.read_u16()?;
    let mut constant_pool: Vec<CpEntry> = Vec::with_capacity(cp_count as usize);
    constant_pool.push(CpEntry::Unused);

    let mut i = 1u16;
    while i < cp_count {
        let tag = r.read_u8()?;
        match tag {
            1 => {
                let len = r.read_u16()? as usize;
                let bytes = r.read_bytes(len)?;
                let s = core::str::from_utf8(bytes)
                    .map_err(|_| JvmError::ClassFormatError(String::from("invalid utf8 in cp")))?;
                constant_pool.push(CpEntry::Utf8(String::from(s)));
            }
            3 => {
                let val = r.read_u32()? as i32;
                constant_pool.push(CpEntry::Integer(val));
            }
            4 => {
                let bits = r.read_u32()?;
                constant_pool.push(CpEntry::Float(f32::from_bits(bits)));
            }
            5 => {
                let hi = r.read_u32()? as u64;
                let lo = r.read_u32()? as u64;
                constant_pool.push(CpEntry::Long(((hi << 32) | lo) as i64));
                constant_pool.push(CpEntry::Unused);
                i += 2;
                continue;
            }
            6 => {
                let hi = r.read_u32()? as u64;
                let lo = r.read_u32()? as u64;
                constant_pool.push(CpEntry::Double(f64::from_bits((hi << 32) | lo)));
                constant_pool.push(CpEntry::Unused);
                i += 2;
                continue;
            }
            7 => {
                let name_index = r.read_u16()?;
                constant_pool.push(CpEntry::Class { name_index });
            }
            8 => {
                let string_index = r.read_u16()?;
                constant_pool.push(CpEntry::StringRef { string_index });
            }
            9 => {
                let class_index = r.read_u16()?;
                let name_and_type_index = r.read_u16()?;
                constant_pool.push(CpEntry::Fieldref {
                    class_index,
                    name_and_type_index,
                });
            }
            10 => {
                let class_index = r.read_u16()?;
                let name_and_type_index = r.read_u16()?;
                constant_pool.push(CpEntry::Methodref {
                    class_index,
                    name_and_type_index,
                });
            }
            11 => {
                let class_index = r.read_u16()?;
                let name_and_type_index = r.read_u16()?;
                constant_pool.push(CpEntry::InterfaceMethodref {
                    class_index,
                    name_and_type_index,
                });
            }
            12 => {
                let name_index = r.read_u16()?;
                let descriptor_index = r.read_u16()?;
                constant_pool.push(CpEntry::NameAndType {
                    name_index,
                    descriptor_index,
                });
            }
            15 => {
                let reference_kind = r.read_u8()?;
                let reference_index = r.read_u16()?;
                constant_pool.push(CpEntry::MethodHandle {
                    reference_kind,
                    reference_index,
                });
            }
            16 => {
                let descriptor_index = r.read_u16()?;
                constant_pool.push(CpEntry::MethodType { descriptor_index });
            }
            18 => {
                let bootstrap_method_attr_index = r.read_u16()?;
                let name_and_type_index = r.read_u16()?;
                constant_pool.push(CpEntry::InvokeDynamic {
                    bootstrap_method_attr_index,
                    name_and_type_index,
                });
            }
            _ => {
                return Err(JvmError::ClassFormatError(alloc::format!(
                    "unknown cp tag: {}",
                    tag
                )));
            }
        }
        i += 1;
    }

    let access_flags = r.read_u16()?;
    let this_class = r.read_u16()?;
    let super_class = r.read_u16()?;

    let iface_count = r.read_u16()?;
    let mut interfaces = Vec::with_capacity(iface_count as usize);
    for _ in 0..iface_count {
        interfaces.push(r.read_u16()?);
    }

    let fields_count = r.read_u16()?;
    let mut fields = Vec::with_capacity(fields_count as usize);
    for _ in 0..fields_count {
        let access_flags = r.read_u16()?;
        let name_index = r.read_u16()?;
        let descriptor_index = r.read_u16()?;
        let attr_count = r.read_u16()?;
        for _ in 0..attr_count {
            let _name = r.read_u16()?;
            let len = r.read_u32()? as usize;
            r.skip(len)?;
        }
        fields.push(FieldInfo {
            access_flags,
            name_index,
            descriptor_index,
        });
    }

    let methods_count = r.read_u16()?;
    let mut methods = Vec::with_capacity(methods_count as usize);
    for _ in 0..methods_count {
        let access_flags = r.read_u16()?;
        let name_index = r.read_u16()?;
        let descriptor_index = r.read_u16()?;
        let attr_count = r.read_u16()?;
        let mut code = None;

        for _ in 0..attr_count {
            let attr_name_index = r.read_u16()?;
            let attr_len = r.read_u32()? as usize;

            let is_code = matches!(
                constant_pool.get(attr_name_index as usize),
                Some(CpEntry::Utf8(s)) if s == "Code"
            );

            if is_code {
                let max_stack = r.read_u16()?;
                let max_locals = r.read_u16()?;
                let code_len = r.read_u32()? as usize;
                let code_bytes = r.read_bytes(code_len)?;

                let exc_table_len = r.read_u16()?;
                let mut exception_table = Vec::with_capacity(exc_table_len as usize);
                for _ in 0..exc_table_len {
                    exception_table.push(ExceptionTableEntry {
                        start_pc: r.read_u16()?,
                        end_pc: r.read_u16()?,
                        handler_pc: r.read_u16()?,
                        catch_type: r.read_u16()?,
                    });
                }

                let sub_attr_count = r.read_u16()?;
                for _ in 0..sub_attr_count {
                    let _name = r.read_u16()?;
                    let len = r.read_u32()? as usize;
                    r.skip(len)?;
                }

                code = Some(CodeAttribute {
                    max_stack,
                    max_locals,
                    code: code_bytes.to_vec(),
                    exception_table,
                });
            } else {
                r.skip(attr_len)?;
            }
        }

        methods.push(MethodInfo {
            access_flags,
            name_index,
            descriptor_index,
            code,
        });
    }

    let attr_count = r.read_u16()?;
    let mut bootstrap_methods = Vec::new();
    for _ in 0..attr_count {
        let attr_name_index = r.read_u16()?;
        let attr_len = r.read_u32()? as usize;

        let is_bootstrap = matches!(
            constant_pool.get(attr_name_index as usize),
            Some(CpEntry::Utf8(s)) if s == "BootstrapMethods"
        );

        if is_bootstrap {
            let num_methods = r.read_u16()?;
            for _ in 0..num_methods {
                let method_ref = r.read_u16()?;
                let num_args = r.read_u16()?;
                let mut arguments = Vec::with_capacity(num_args as usize);
                for _ in 0..num_args {
                    arguments.push(r.read_u16()?);
                }
                bootstrap_methods.push(BootstrapMethodEntry {
                    method_ref,
                    arguments,
                });
            }
        } else {
            r.skip(attr_len)?;
        }
    }

    Ok(ClassFile {
        minor_version,
        major_version,
        constant_pool,
        access_flags,
        this_class,
        super_class,
        interfaces,
        fields,
        methods,
        bootstrap_methods,
    })
}

pub fn count_descriptor_args(descriptor: &str) -> usize {
    let mut count = 0;
    let bytes = descriptor.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b')' => break,
            b'L' => {
                count += 1;
                while i < bytes.len() && bytes[i] != b';' {
                    i += 1;
                }
                i += 1;
            }
            b'[' => {
                while i < bytes.len() && bytes[i] == b'[' {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'L' {
                    while i < bytes.len() && bytes[i] != b';' {
                        i += 1;
                    }
                    i += 1;
                } else {
                    i += 1;
                }
                count += 1;
            }
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' => {
                count += 1;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    count
}

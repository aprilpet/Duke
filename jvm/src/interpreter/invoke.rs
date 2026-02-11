use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use shared::classfile::{
    self,
    CpEntry,
};
use shared::opcodes::INVOKESTATIC;
use shared::types::{
    JvmError,
    JvmValue,
};

use super::{
    Frame,
    Vm,
    jvm_value_to_string,
};
use crate::native::NativeBridge;

impl<N: NativeBridge> Vm<N> {
    pub(crate) fn do_getstatic(&mut self, f: &mut Frame, idx: u16) -> Result<(), JvmError> {
        let class = &self.classes[f.class_idx];
        if let CpEntry::Fieldref {
            class_index,
            name_and_type_index,
        } = &class.constant_pool[idx as usize]
        {
            let class_name = class.get_class_name(*class_index)?;
            let (field_name, _desc) = class.resolve_name_and_type(*name_and_type_index)?;

            if class_name == "java/lang/System" && field_name == "out" {
                let id = self
                    .heap
                    .alloc_object(String::from("java/io/PrintStream"))?;
                f.push(JvmValue::ObjectRef(id));
            } else if class_name == "java/lang/System" && field_name == "err" {
                let id = self
                    .heap
                    .alloc_object(String::from("java/io/PrintStream"))?;
                f.push(JvmValue::ObjectRef(id));
            } else {
                let key = format!("{}.{}", class_name, field_name);
                if let Some(val) = self.statics.get(&key) {
                    f.push(val.clone());
                } else {
                    let result = self.natives.call_native(
                        class_name,
                        &format!("getstatic_{}", field_name),
                        "",
                        &[],
                    )?;
                    f.push(result.unwrap_or(JvmValue::Null));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn do_getfield(&mut self, f: &mut Frame, idx: u16) -> Result<(), JvmError> {
        let obj_ref = f.pop()?.as_object_ref()?;
        let class = &self.classes[f.class_idx];
        if let CpEntry::Fieldref {
            name_and_type_index,
            ..
        } = &class.constant_pool[idx as usize]
        {
            let (field_name, _) = class.resolve_name_and_type(*name_and_type_index)?;
            let obj = self.heap.get_object(obj_ref)?;
            let val = obj
                .fields
                .get(field_name)
                .cloned()
                .unwrap_or(JvmValue::Int(0));
            f.push(val);
        }
        Ok(())
    }

    pub(crate) fn do_putfield(&mut self, f: &mut Frame, idx: u16) -> Result<(), JvmError> {
        let val = f.pop()?;
        let obj_ref = f.pop()?.as_object_ref()?;
        let class = &self.classes[f.class_idx];
        if let CpEntry::Fieldref {
            name_and_type_index,
            ..
        } = &class.constant_pool[idx as usize]
        {
            let (field_name, _) = class.resolve_name_and_type(*name_and_type_index)?;
            let field_owned = String::from(field_name);
            let obj = self.heap.get_object_mut(obj_ref)?;
            obj.fields.insert(field_owned, val);
        }
        Ok(())
    }

    pub(crate) fn do_invokedynamic(&mut self, f: &mut Frame, idx: u16) -> Result<(), JvmError> {
        let class = &self.classes[f.class_idx];

        let (bootstrap_idx, name_and_type_idx) = match &class.constant_pool[idx as usize] {
            CpEntry::InvokeDynamic {
                bootstrap_method_attr_index,
                name_and_type_index,
            } => (*bootstrap_method_attr_index, *name_and_type_index),
            _ => {
                return Err(JvmError::ClassFormatError(format!(
                    "expected InvokeDynamic at cp#{}",
                    idx
                )));
            }
        };

        let (method_name, descriptor) = class.resolve_name_and_type(name_and_type_idx)?;
        let method_name = String::from(method_name);
        let descriptor = String::from(descriptor);

        if method_name == "makeConcatWithConstants" {
            let arg_count = classfile::count_descriptor_args(&descriptor);
            let mut args = Vec::with_capacity(arg_count);
            for _ in 0..arg_count {
                args.push(f.pop()?);
            }
            args.reverse();

            let recipe = {
                let bsm = &class.bootstrap_methods[bootstrap_idx as usize];
                if let Some(&recipe_idx) = bsm.arguments.first() {
                    match &class.constant_pool[recipe_idx as usize] {
                        CpEntry::StringRef { string_index } => {
                            String::from(class.get_utf8(*string_index).unwrap_or(""))
                        }
                        CpEntry::Utf8(s) => s.clone(),
                        _ => String::new(),
                    }
                } else {
                    String::new()
                }
            };

            let mut result = String::new();
            let mut arg_iter = args.iter();
            for byte in recipe.as_bytes() {
                if *byte == 1 {
                    if let Some(arg) = arg_iter.next() {
                        result.push_str(&jvm_value_to_string(arg));
                    }
                } else {
                    result.push(*byte as char);
                }
            }
            for arg in arg_iter {
                result.push_str(&jvm_value_to_string(arg));
            }

            f.push(JvmValue::StringRef(result));
            Ok(())
        } else {
            Err(JvmError::UnsupportedOpcode(0xBA))
        }
    }

    pub(crate) fn do_invoke(&mut self, f: &mut Frame, op: u8, idx: u16) -> Result<(), JvmError> {
        let (class_name, method_name, descriptor) = {
            let class = &self.classes[f.class_idx];
            let (ci, nti) = match &class.constant_pool[idx as usize] {
                CpEntry::Methodref {
                    class_index,
                    name_and_type_index,
                } => (*class_index, *name_and_type_index),
                CpEntry::InterfaceMethodref {
                    class_index,
                    name_and_type_index,
                } => (*class_index, *name_and_type_index),
                _ => {
                    return Err(JvmError::ClassFormatError(format!(
                        "expected Methodref at cp#{}",
                        idx
                    )));
                }
            };
            let cn = String::from(class.get_class_name(ci)?);
            let (mn, desc) = class.resolve_name_and_type(nti)?;
            (cn, String::from(mn), String::from(desc))
        };

        let arg_count = classfile::count_descriptor_args(&descriptor);
        let has_receiver = op != INVOKESTATIC;
        let total = arg_count + if has_receiver { 1 } else { 0 };

        let mut args = Vec::with_capacity(total);
        for _ in 0..total {
            args.push(f.pop()?);
        }
        args.reverse();

        // System methods
        if class_name == "java/lang/System" && method_name == "exit" {
            let code = args.first().and_then(|v| v.as_int().ok()).unwrap_or(0);
            return Err(JvmError::SystemExit(code));
        }

        if class_name == "java/lang/System" && method_name == "currentTimeMillis" {
            f.push(JvmValue::Long(0));
            return Ok(());
        }

        if class_name == "java/lang/System" && method_name == "arraycopy" {
            if args.len() >= 5 {
                let src_ref = args[0].as_array_ref()?;
                let src_pos = args[1].as_int()? as usize;
                let dst_ref = args[2].as_array_ref()?;
                let dst_pos = args[3].as_int()? as usize;
                let length = args[4].as_int()? as usize;
                let values: Vec<JvmValue> = {
                    let src = self.heap.get_array(src_ref)?;
                    src.elements[src_pos..src_pos + length].to_vec()
                };
                let dst = self.heap.get_array_mut(dst_ref)?;
                for i in 0..length {
                    dst.elements[dst_pos + i] = values[i].clone();
                }
            }
            return Ok(());
        }

        // PrintStream
        if class_name == "java/io/PrintStream"
            && (method_name == "println" || method_name == "print")
        {
            let print_args = if has_receiver { &args[1..] } else { &args };
            self.natives
                .call_native("efi/Console", &method_name, &descriptor, print_args)?;
            return Ok(());
        }

        if class_name == "java/io/PrintStream"
            && (method_name == "format" || method_name == "printf")
        {
            let print_args = if has_receiver { &args[1..] } else { &args };
            if let Some(JvmValue::StringRef(fmt)) = print_args.first() {
                let arr_vals = match print_args.get(1) {
                    Some(JvmValue::ArrayRef(arr_id)) => {
                        let arr = self.heap.get_array(*arr_id)?;
                        arr.elements.clone()
                    }
                    _ => Vec::new(),
                };
                let result = self.do_string_format(fmt, &arr_vals)?;
                self.natives.call_native(
                    "efi/Console",
                    "print",
                    "(Ljava/lang/String;)V",
                    &[JvmValue::StringRef(result)],
                )?;
            }
            if has_receiver {
                f.push(args[0].clone());
            }
            return Ok(());
        }

        // StringBuilder
        if class_name == "java/lang/StringBuilder" {
            let result = self.handle_string_builder(&method_name, &descriptor, &args)?;
            if let Some(val) = result {
                f.push(val);
            }
            return Ok(());
        }

        // String methods
        if class_name == "java/lang/String" {
            if self.handle_string_method(f, &method_name, &descriptor, &args)? {
                return Ok(());
            }
        }

        // Integer methods
        if class_name == "java/lang/Integer" {
            if self.handle_integer_method(f, &method_name, &descriptor, &args)? {
                return Ok(());
            }
        }

        // Boxing (Boolean, Byte, Short, Character, Long)
        if self.handle_boxing(f, &class_name, &method_name, &args)? {
            return Ok(());
        }

        // Math
        if class_name == "java/lang/Math" {
            return self.handle_math(f, &method_name, &args);
        }

        // Unknown <init> — skip
        if method_name == "<init>" && self.find_class_index(&class_name).is_none() {
            return Ok(());
        }

        // Generic dispatch
        if self.find_class_index(&class_name).is_some() {
            let result = self.execute(&class_name, &method_name, args)?;
            if let Some(val) = result {
                f.push(val);
            }
        } else {
            let result = self
                .natives
                .call_native(&class_name, &method_name, &descriptor, &args)?;
            if let Some(val) = result {
                f.push(val);
            }
        }

        Ok(())
    }
}

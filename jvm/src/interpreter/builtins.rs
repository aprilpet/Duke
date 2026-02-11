use alloc::format;
use alloc::string::{
    String,
    ToString,
};
use alloc::vec::Vec;

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
    pub(crate) fn handle_string_method(
        &mut self,
        f: &mut Frame,
        method_name: &str,
        descriptor: &str,
        args: &[JvmValue],
    ) -> Result<bool, JvmError> {
        match method_name {
            "valueOf" => {
                let s = match descriptor {
                    "(Z)Ljava/lang/String;" => {
                        let v = args.first().and_then(|a| a.as_int().ok()).unwrap_or(0);
                        if v != 0 {
                            String::from("true")
                        } else {
                            String::from("false")
                        }
                    }
                    "(C)Ljava/lang/String;" => {
                        let v = args.first().and_then(|a| a.as_int().ok()).unwrap_or(0);
                        let c = char::from_u32(v as u32).unwrap_or('\0');
                        let mut s = String::new();
                        s.push(c);
                        s
                    }
                    _ => {
                        if let Some(v) = args.last() {
                            jvm_value_to_string(v)
                        } else {
                            String::from("null")
                        }
                    }
                };
                f.push(JvmValue::StringRef(s));
                Ok(true)
            }

            "format" => {
                let (format_str, arr_arg) = if let Some(JvmValue::StringRef(s)) = args.first() {
                    (s.clone(), args.get(1))
                } else {
                    let fmt = args
                        .get(1)
                        .and_then(|v| match v {
                            JvmValue::StringRef(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    (fmt, args.get(2))
                };
                let format_args = match arr_arg {
                    Some(JvmValue::ArrayRef(arr_id)) => {
                        let arr = self.heap.get_array(*arr_id)?;
                        arr.elements.clone()
                    }
                    _ => Vec::new(),
                };
                let result = self.do_string_format(&format_str, &format_args)?;
                f.push(JvmValue::StringRef(result));
                Ok(true)
            }

            "concat" => {
                let result = match (args.first(), args.get(1)) {
                    (Some(JvmValue::StringRef(a)), Some(JvmValue::StringRef(b))) => {
                        format!("{}{}", a, b)
                    }
                    (Some(JvmValue::StringRef(a)), _) => a.clone(),
                    _ => String::new(),
                };
                f.push(JvmValue::StringRef(result));
                Ok(true)
            }

            "replace" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    match (args.get(1), args.get(2)) {
                        (Some(JvmValue::Int(old_ch)), Some(JvmValue::Int(new_ch))) => {
                            let old_c = char::from_u32(*old_ch as u32).unwrap_or('\0');
                            let new_c = char::from_u32(*new_ch as u32).unwrap_or('\0');
                            f.push(JvmValue::StringRef(s.replace(old_c, &new_c.to_string())));
                        }
                        (Some(JvmValue::StringRef(old)), Some(JvmValue::StringRef(new))) => {
                            f.push(JvmValue::StringRef(s.replace(old.as_str(), new.as_str())));
                        }
                        _ => f.push(JvmValue::StringRef(s.clone())),
                    }
                } else {
                    f.push(JvmValue::StringRef(String::new()));
                }
                Ok(true)
            }

            "length" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    f.push(JvmValue::Int(s.len() as i32));
                } else {
                    f.push(JvmValue::Int(0));
                }
                Ok(true)
            }

            "charAt" => {
                if let (Some(JvmValue::StringRef(s)), Some(JvmValue::Int(idx))) =
                    (args.first(), args.get(1))
                {
                    let ch = s.as_bytes().get(*idx as usize).copied().unwrap_or(0);
                    f.push(JvmValue::Int(ch as i32));
                } else {
                    f.push(JvmValue::Int(0));
                }
                Ok(true)
            }

            "equals" => {
                let result = match (args.first(), args.get(1)) {
                    (Some(JvmValue::StringRef(a)), Some(JvmValue::StringRef(b))) => a == b,
                    (Some(JvmValue::Null), Some(JvmValue::Null)) => true,
                    _ => false,
                };
                f.push(JvmValue::Int(if result { 1 } else { 0 }));
                Ok(true)
            }

            "hashCode" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    let mut h: i32 = 0;
                    for b in s.bytes() {
                        h = h.wrapping_mul(31).wrapping_add(b as i32);
                    }
                    f.push(JvmValue::Int(h));
                } else {
                    f.push(JvmValue::Int(0));
                }
                Ok(true)
            }

            "substring" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    let begin = args.get(1).and_then(|v| v.as_int().ok()).unwrap_or(0) as usize;
                    let end = args
                        .get(2)
                        .and_then(|v| v.as_int().ok())
                        .map(|v| v as usize)
                        .unwrap_or(s.len());
                    let sub = if begin <= end && end <= s.len() {
                        String::from(&s[begin..end])
                    } else {
                        String::new()
                    };
                    f.push(JvmValue::StringRef(sub));
                } else {
                    f.push(JvmValue::StringRef(String::new()));
                }
                Ok(true)
            }

            "indexOf" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    let result = match args.get(1) {
                        Some(JvmValue::Int(ch)) => {
                            let c = char::from_u32(*ch as u32).unwrap_or('\0');
                            s.find(c).map(|i| i as i32).unwrap_or(-1)
                        }
                        Some(JvmValue::StringRef(needle)) => {
                            s.find(needle.as_str()).map(|i| i as i32).unwrap_or(-1)
                        }
                        _ => -1,
                    };
                    f.push(JvmValue::Int(result));
                } else {
                    f.push(JvmValue::Int(-1));
                }
                Ok(true)
            }

            "contains" => {
                let result = match (args.first(), args.get(1)) {
                    (Some(JvmValue::StringRef(s)), Some(JvmValue::StringRef(needle))) => {
                        s.contains(needle.as_str())
                    }
                    _ => false,
                };
                f.push(JvmValue::Int(if result { 1 } else { 0 }));
                Ok(true)
            }

            "isEmpty" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    f.push(JvmValue::Int(if s.is_empty() { 1 } else { 0 }));
                } else {
                    f.push(JvmValue::Int(1));
                }
                Ok(true)
            }

            "startsWith" => {
                let result = match (args.first(), args.get(1)) {
                    (Some(JvmValue::StringRef(s)), Some(JvmValue::StringRef(prefix))) => {
                        s.starts_with(prefix.as_str())
                    }
                    _ => false,
                };
                f.push(JvmValue::Int(if result { 1 } else { 0 }));
                Ok(true)
            }

            "endsWith" => {
                let result = match (args.first(), args.get(1)) {
                    (Some(JvmValue::StringRef(s)), Some(JvmValue::StringRef(suffix))) => {
                        s.ends_with(suffix.as_str())
                    }
                    _ => false,
                };
                f.push(JvmValue::Int(if result { 1 } else { 0 }));
                Ok(true)
            }

            "toCharArray" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    let arr_id = self.heap.alloc_array(String::from("char"), s.len())?;
                    {
                        let arr = self.heap.get_array_mut(arr_id)?;
                        for (i, b) in s.bytes().enumerate() {
                            arr.elements[i] = JvmValue::Int(b as i32);
                        }
                    }
                    f.push(JvmValue::ArrayRef(arr_id));
                } else {
                    let arr_id = self.heap.alloc_array(String::from("char"), 0)?;
                    f.push(JvmValue::ArrayRef(arr_id));
                }
                Ok(true)
            }

            "compareTo" => {
                let result = match (args.first(), args.get(1)) {
                    (Some(JvmValue::StringRef(a)), Some(JvmValue::StringRef(b))) => {
                        match a.as_str().cmp(b.as_str()) {
                            core::cmp::Ordering::Less => -1,
                            core::cmp::Ordering::Equal => 0,
                            core::cmp::Ordering::Greater => 1,
                        }
                    }
                    _ => 0,
                };
                f.push(JvmValue::Int(result));
                Ok(true)
            }

            "trim" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    f.push(JvmValue::StringRef(String::from(s.trim())));
                } else {
                    f.push(JvmValue::StringRef(String::new()));
                }
                Ok(true)
            }

            "toLowerCase" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    let mut out = String::new();
                    for c in s.chars() {
                        for lc in c.to_lowercase() {
                            out.push(lc);
                        }
                    }
                    f.push(JvmValue::StringRef(out));
                } else {
                    f.push(JvmValue::StringRef(String::new()));
                }
                Ok(true)
            }

            "toUpperCase" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    let mut out = String::new();
                    for c in s.chars() {
                        for uc in c.to_uppercase() {
                            out.push(uc);
                        }
                    }
                    f.push(JvmValue::StringRef(out));
                } else {
                    f.push(JvmValue::StringRef(String::new()));
                }
                Ok(true)
            }

            _ => Ok(false),
        }
    }

    pub(crate) fn handle_integer_method(
        &mut self,
        f: &mut Frame,
        method_name: &str,
        _descriptor: &str,
        args: &[JvmValue],
    ) -> Result<bool, JvmError> {
        match method_name {
            "parseInt" => {
                if let Some(JvmValue::StringRef(s)) = args.first() {
                    let radix = args.get(1).and_then(|v| v.as_int().ok()).unwrap_or(10);
                    match i32::from_str_radix(s.trim(), radix as u32) {
                        Ok(v) => f.push(JvmValue::Int(v)),
                        Err(_) => {
                            return Err(JvmError::NativeMethodError(format!(
                                "NumberFormatException: {}",
                                s
                            )));
                        }
                    }
                } else {
                    f.push(JvmValue::Int(0));
                }
                Ok(true)
            }

            "valueOf" => {
                if let Some(JvmValue::Int(v)) = args.first() {
                    let id = self.heap.alloc_object(String::from("java/lang/Integer"))?;
                    {
                        let obj = self.heap.get_object_mut(id)?;
                        obj.fields.insert(String::from("value"), JvmValue::Int(*v));
                    }
                    f.push(JvmValue::ObjectRef(id));
                } else if let Some(JvmValue::StringRef(s)) = args.first() {
                    match s.trim().parse::<i32>() {
                        Ok(v) => {
                            let id = self.heap.alloc_object(String::from("java/lang/Integer"))?;
                            {
                                let obj = self.heap.get_object_mut(id)?;
                                obj.fields.insert(String::from("value"), JvmValue::Int(v));
                            }
                            f.push(JvmValue::ObjectRef(id));
                        }
                        Err(_) => f.push(JvmValue::Null),
                    }
                } else {
                    f.push(JvmValue::Null);
                }
                Ok(true)
            }

            "intValue" => {
                if let Some(JvmValue::ObjectRef(id)) = args.first() {
                    let obj = self.heap.get_object(*id)?;
                    let v = obj.fields.get("value").cloned().unwrap_or(JvmValue::Int(0));
                    f.push(v);
                } else if let Some(JvmValue::Int(v)) = args.first() {
                    f.push(JvmValue::Int(*v));
                } else {
                    f.push(JvmValue::Int(0));
                }
                Ok(true)
            }

            "toString" => {
                if let Some(JvmValue::Int(v)) = args.first() {
                    f.push(JvmValue::StringRef(format!("{}", v)));
                } else if let Some(JvmValue::ObjectRef(id)) = args.first() {
                    let obj = self.heap.get_object(*id)?;
                    if let Some(JvmValue::Int(v)) = obj.fields.get("value") {
                        f.push(JvmValue::StringRef(format!("{}", v)));
                    } else {
                        f.push(JvmValue::StringRef(String::from("0")));
                    }
                } else {
                    f.push(JvmValue::StringRef(String::from("0")));
                }
                Ok(true)
            }

            _ => Ok(false),
        }
    }

    pub(crate) fn handle_boxing(
        &mut self,
        f: &mut Frame,
        class_name: &str,
        method_name: &str,
        args: &[JvmValue],
    ) -> Result<bool, JvmError> {
        if method_name == "valueOf" {
            let boxing_classes = [
                "java/lang/Boolean",
                "java/lang/Byte",
                "java/lang/Short",
                "java/lang/Character",
                "java/lang/Long",
            ];
            if boxing_classes.contains(&class_name) {
                if let Some(v) = args.first() {
                    let id = self.heap.alloc_object(String::from(class_name))?;
                    {
                        let obj = self.heap.get_object_mut(id)?;
                        obj.fields.insert(String::from("value"), v.clone());
                    }
                    f.push(JvmValue::ObjectRef(id));
                } else {
                    f.push(JvmValue::Null);
                }
                return Ok(true);
            }
        }

        let unbox_methods = [
            "intValue",
            "longValue",
            "shortValue",
            "byteValue",
            "charValue",
        ];
        if unbox_methods.contains(&method_name) {
            if let Some(JvmValue::ObjectRef(id)) = args.first() {
                let obj = self.heap.get_object(*id)?;
                let v = obj.fields.get("value").cloned().unwrap_or(JvmValue::Int(0));
                f.push(v);
            } else if let Some(v) = args.first() {
                f.push(v.clone());
            } else {
                f.push(JvmValue::Int(0));
            }
            return Ok(true);
        }

        Ok(false)
    }

    pub(crate) fn handle_string_builder(
        &mut self,
        method_name: &str,
        _descriptor: &str,
        args: &[JvmValue],
    ) -> Result<Option<JvmValue>, JvmError> {
        match method_name {
            "<init>" => Ok(None),
            "append" => {
                let obj_ref = args[0].as_object_ref()?;
                let current = {
                    let obj = self.heap.get_object(obj_ref)?;
                    obj.fields
                        .get("value")
                        .and_then(|v| {
                            if let JvmValue::StringRef(s) = v {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default()
                };
                let appended = if args.len() > 1 {
                    let piece = jvm_value_to_string(&args[1]);
                    format!("{}{}", current, piece)
                } else {
                    current
                };
                let obj = self.heap.get_object_mut(obj_ref)?;
                obj.fields
                    .insert(String::from("value"), JvmValue::StringRef(appended));
                Ok(Some(JvmValue::ObjectRef(obj_ref)))
            }
            "toString" => {
                let obj_ref = args[0].as_object_ref()?;
                let obj = self.heap.get_object(obj_ref)?;
                let s = obj
                    .fields
                    .get("value")
                    .and_then(|v| {
                        if let JvmValue::StringRef(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                Ok(Some(JvmValue::StringRef(s)))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn handle_math(
        &self,
        f: &mut Frame,
        method_name: &str,
        args: &[JvmValue],
    ) -> Result<(), JvmError> {
        match method_name {
            "abs" => match args.first() {
                Some(JvmValue::Int(v)) => f.push(JvmValue::Int(v.wrapping_abs())),
                Some(JvmValue::Long(v)) => f.push(JvmValue::Long(v.wrapping_abs())),
                Some(JvmValue::Float(v)) => f.push(JvmValue::Float(v.abs())),
                Some(JvmValue::Double(v)) => f.push(JvmValue::Double(v.abs())),
                _ => f.push(JvmValue::Int(0)),
            },
            "max" => match (args.first(), args.get(1)) {
                (Some(JvmValue::Int(a)), Some(JvmValue::Int(b))) => {
                    f.push(JvmValue::Int(if *a > *b { *a } else { *b }));
                }
                (Some(JvmValue::Long(a)), Some(JvmValue::Long(b))) => {
                    f.push(JvmValue::Long(if *a > *b { *a } else { *b }));
                }
                _ => f.push(JvmValue::Int(0)),
            },
            "min" => match (args.first(), args.get(1)) {
                (Some(JvmValue::Int(a)), Some(JvmValue::Int(b))) => {
                    f.push(JvmValue::Int(if *a < *b { *a } else { *b }));
                }
                (Some(JvmValue::Long(a)), Some(JvmValue::Long(b))) => {
                    f.push(JvmValue::Long(if *a < *b { *a } else { *b }));
                }
                _ => f.push(JvmValue::Int(0)),
            },
            _ => {
                f.push(JvmValue::Int(0));
            }
        }
        Ok(())
    }

    pub(crate) fn unbox_if_needed(&self, val: &JvmValue) -> JvmValue {
        match val {
            JvmValue::ObjectRef(id) => {
                if let Ok(obj) = self.heap.get_object(*id) {
                    if let Some(v) = obj.fields.get("value") {
                        return v.clone();
                    }
                }
                val.clone()
            }
            _ => val.clone(),
        }
    }

    pub(crate) fn format_arg_as_string(&self, val: &JvmValue) -> String {
        let unboxed = self.unbox_if_needed(val);
        jvm_value_to_string(&unboxed)
    }

    pub(crate) fn do_string_format(
        &self,
        format_str: &str,
        args: &[JvmValue],
    ) -> Result<String, JvmError> {
        let mut result = String::new();
        let bytes = format_str.as_bytes();
        let mut i = 0;
        let mut arg_idx = 0;

        while i < bytes.len() {
            if bytes[i] == b'%' && i + 1 < bytes.len() {
                i += 1;
                while i < bytes.len()
                    && (bytes[i] == b'-'
                        || bytes[i] == b'+'
                        || bytes[i] == b' '
                        || bytes[i] == b'0'
                        || bytes[i] == b'#'
                        || bytes[i] == b'.'
                        || (bytes[i] >= b'0' && bytes[i] <= b'9'))
                {
                    i += 1;
                }
                if i >= bytes.len() {
                    break;
                }
                match bytes[i] {
                    b's' => {
                        if let Some(arg) = args.get(arg_idx) {
                            result.push_str(&self.format_arg_as_string(arg));
                        }
                        arg_idx += 1;
                    }
                    b'd' => {
                        if let Some(arg) = args.get(arg_idx) {
                            let val = self.unbox_if_needed(arg);
                            result.push_str(&jvm_value_to_string(&val));
                        }
                        arg_idx += 1;
                    }
                    b'f' => {
                        if let Some(arg) = args.get(arg_idx) {
                            let val = self.unbox_if_needed(arg);
                            match &val {
                                JvmValue::Float(v) => result.push_str(&format!("{:.6}", v)),
                                JvmValue::Double(v) => result.push_str(&format!("{:.6}", v)),
                                _ => result.push_str(&jvm_value_to_string(&val)),
                            }
                        }
                        arg_idx += 1;
                    }
                    b'x' => {
                        if let Some(arg) = args.get(arg_idx) {
                            let val = self.unbox_if_needed(arg);
                            if let JvmValue::Int(v) = val {
                                result.push_str(&format!("{:x}", v));
                            } else {
                                result.push_str(&jvm_value_to_string(&val));
                            }
                        }
                        arg_idx += 1;
                    }
                    b'X' => {
                        if let Some(arg) = args.get(arg_idx) {
                            let val = self.unbox_if_needed(arg);
                            if let JvmValue::Int(v) = val {
                                result.push_str(&format!("{:X}", v));
                            } else {
                                result.push_str(&jvm_value_to_string(&val));
                            }
                        }
                        arg_idx += 1;
                    }
                    b'o' => {
                        if let Some(arg) = args.get(arg_idx) {
                            let val = self.unbox_if_needed(arg);
                            if let JvmValue::Int(v) = val {
                                result.push_str(&format!("{:o}", v));
                            } else {
                                result.push_str(&jvm_value_to_string(&val));
                            }
                        }
                        arg_idx += 1;
                    }
                    b'c' => {
                        if let Some(arg) = args.get(arg_idx) {
                            let val = self.unbox_if_needed(arg);
                            if let JvmValue::Int(v) = val {
                                if let Some(c) = char::from_u32(v as u32) {
                                    result.push(c);
                                }
                            }
                        }
                        arg_idx += 1;
                    }
                    b'b' => {
                        if let Some(arg) = args.get(arg_idx) {
                            let s = match arg {
                                JvmValue::Null => "false",
                                JvmValue::Int(0) => "false",
                                _ => "true",
                            };
                            result.push_str(s);
                        }
                        arg_idx += 1;
                    }
                    b'n' => {
                        result.push('\n');
                    }
                    b'%' => {
                        result.push('%');
                    }
                    other => {
                        result.push('%');
                        result.push(other as char);
                    }
                }
                i += 1;
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        }

        Ok(result)
    }
}

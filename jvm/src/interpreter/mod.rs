use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use shared::classfile::{
    ACC_NATIVE,
    ClassFile,
    ExceptionTableEntry,
};
use shared::types::{
    JvmError,
    JvmValue,
};

use crate::heap::Heap;
use crate::native::NativeBridge;

mod builtins;
mod exec;
mod invoke;

pub(crate) enum ExecAction {
    Continue,
    ReturnVal(JvmValue),
    ReturnVoid,
    Throw(String, JvmValue),
}

pub(crate) struct Frame {
    pub(crate) stack: Vec<JvmValue>,
    pub(crate) locals: Vec<JvmValue>,
    pub(crate) code: Vec<u8>,
    pub(crate) pc: usize,
    pub(crate) class_idx: usize,
    pub(crate) exception_table: Vec<ExceptionTableEntry>,
}

impl Frame {
    pub(crate) fn read_u8(&mut self) -> u8 {
        let v = self.code[self.pc];
        self.pc += 1;
        v
    }

    pub(crate) fn read_i16(&mut self) -> i16 {
        let hi = self.code[self.pc] as i16;
        let lo = self.code[self.pc + 1] as i16;
        self.pc += 2;
        (hi << 8) | (lo & 0xFF)
    }

    pub(crate) fn read_u16(&mut self) -> u16 {
        let hi = self.code[self.pc] as u16;
        let lo = self.code[self.pc + 1] as u16;
        self.pc += 2;
        (hi << 8) | lo
    }

    pub(crate) fn read_i32(&mut self) -> i32 {
        let b1 = self.code[self.pc] as i32;
        let b2 = self.code[self.pc + 1] as i32;
        let b3 = self.code[self.pc + 2] as i32;
        let b4 = self.code[self.pc + 3] as i32;
        self.pc += 4;
        (b1 << 24) | (b2 << 16) | (b3 << 8) | b4
    }

    pub(crate) fn push(&mut self, val: JvmValue) {
        self.stack.push(val);
    }

    pub(crate) fn pop(&mut self) -> Result<JvmValue, JvmError> {
        self.stack.pop().ok_or(JvmError::StackUnderflow)
    }

    pub(crate) fn pop_int(&mut self) -> Result<i32, JvmError> {
        self.pop()?.as_int()
    }

    pub(crate) fn pop_long(&mut self) -> Result<i64, JvmError> {
        self.pop()?.as_long()
    }

    pub(crate) fn pop_float(&mut self) -> Result<f32, JvmError> {
        match self.pop()? {
            JvmValue::Float(v) => Ok(v),
            JvmValue::Int(v) => Ok(v as f32),
            _ => Err(JvmError::TypeError(String::from("expected float"))),
        }
    }

    pub(crate) fn pop_double(&mut self) -> Result<f64, JvmError> {
        match self.pop()? {
            JvmValue::Double(v) => Ok(v),
            JvmValue::Float(v) => Ok(v as f64),
            JvmValue::Int(v) => Ok(v as f64),
            JvmValue::Long(v) => Ok(v as f64),
            _ => Err(JvmError::TypeError(String::from("expected double"))),
        }
    }
}

pub struct Vm<N: NativeBridge> {
    pub(crate) classes: Vec<ClassFile>,
    pub heap: Heap,
    pub natives: N,
    pub(crate) statics: BTreeMap<String, JvmValue>,
}

impl<N: NativeBridge> Vm<N> {
    pub fn new(natives: N) -> Self {
        Self {
            classes: Vec::new(),
            heap: Heap::new(),
            natives,
            statics: BTreeMap::new(),
        }
    }

    pub fn load_class(&mut self, class: ClassFile) {
        self.classes.push(class);
    }

    pub(crate) fn find_class_index(&self, name: &str) -> Option<usize> {
        self.classes
            .iter()
            .position(|c| c.class_name().ok() == Some(name))
    }

    pub(crate) fn is_subclass(&self, child: &str, parent: &str) -> bool {
        if child == parent {
            return true;
        }
        let well_known = [
            "java/lang/Object",
            "java/lang/Throwable",
            "java/lang/Exception",
            "java/lang/RuntimeException",
            "java/lang/NullPointerException",
            "java/lang/ArithmeticException",
            "java/lang/ArrayIndexOutOfBoundsException",
            "java/lang/ClassCastException",
            "java/lang/IllegalArgumentException",
            "java/lang/UnsupportedOperationException",
            "java/lang/IndexOutOfBoundsException",
        ];
        if child == parent {
            return true;
        }
        if parent == "java/lang/Object" {
            return true;
        }
        if parent == "java/lang/Throwable" && well_known.contains(&child) {
            return true;
        }
        if parent == "java/lang/Exception" {
            return child != "java/lang/Throwable" && well_known.contains(&child);
        }
        if parent == "java/lang/RuntimeException" {
            let runtime_excs = [
                "java/lang/NullPointerException",
                "java/lang/ArithmeticException",
                "java/lang/ArrayIndexOutOfBoundsException",
                "java/lang/ClassCastException",
                "java/lang/IllegalArgumentException",
                "java/lang/UnsupportedOperationException",
                "java/lang/IndexOutOfBoundsException",
            ];
            return runtime_excs.contains(&child);
        }
        if parent == "java/lang/IndexOutOfBoundsException"
            && child == "java/lang/ArrayIndexOutOfBoundsException"
        {
            return true;
        }
        if let Some(idx) = self.find_class_index(child) {
            if let Some(super_name) = self.classes[idx].super_class_name() {
                let sn = String::from(super_name);
                return self.is_subclass(&sn, parent);
            }
        }
        false
    }

    pub fn execute(
        &mut self,
        class_name: &str,
        method_name: &str,
        args: Vec<JvmValue>,
    ) -> Result<Option<JvmValue>, JvmError> {
        let class_idx = match self.find_class_index(class_name) {
            Some(idx) => idx,
            None => {
                return self.natives.call_native(class_name, method_name, "", &args);
            }
        };

        let class = &self.classes[class_idx];
        let method = class
            .find_method_by_name(method_name)
            .ok_or_else(|| JvmError::MethodNotFound(format!("{}::{}", class_name, method_name)))?;

        if method.access_flags & ACC_NATIVE != 0 {
            let desc = class.get_utf8(method.descriptor_index).unwrap_or("()V");
            return self
                .natives
                .call_native(class_name, method_name, desc, &args);
        }

        let code_attr = method.code.as_ref().ok_or_else(|| {
            JvmError::MethodNotFound(format!("{}::{} has no Code", class_name, method_name))
        })?;

        let mut locals = alloc::vec![JvmValue::Int(0); code_attr.max_locals as usize];
        for (i, arg) in args.into_iter().enumerate() {
            if i < locals.len() {
                locals[i] = arg;
            }
        }

        let mut frame = Frame {
            stack: Vec::with_capacity(code_attr.max_stack as usize),
            locals,
            code: code_attr.code.clone(),
            pc: 0,
            class_idx,
            exception_table: code_attr.exception_table.clone(),
        };

        self.interpret(&mut frame)
    }

    fn find_exception_handler(&self, frame: &Frame, pc: usize, exc_class: &str) -> Option<u16> {
        for entry in &frame.exception_table {
            if pc >= entry.start_pc as usize && pc < entry.end_pc as usize {
                if entry.catch_type == 0 {
                    return Some(entry.handler_pc);
                }
                let class = &self.classes[frame.class_idx];
                if let Ok(catch_name) = class.get_class_name(entry.catch_type) {
                    if self.is_subclass(exc_class, catch_name) {
                        return Some(entry.handler_pc);
                    }
                }
            }
        }
        None
    }

    fn interpret(&mut self, f: &mut Frame) -> Result<Option<JvmValue>, JvmError> {
        loop {
            let op_pc = f.pc;
            let op = f.read_u8();

            let result = self.exec_one(f, op, op_pc);

            match result {
                Ok(action) => match action {
                    ExecAction::Continue => {}
                    ExecAction::ReturnVal(v) => return Ok(Some(v)),
                    ExecAction::ReturnVoid => return Ok(None),
                    ExecAction::Throw(exc_class, exc_obj) => {
                        if let Some(handler_pc) = self.find_exception_handler(f, op_pc, &exc_class)
                        {
                            f.stack.clear();
                            f.push(exc_obj);
                            f.pc = handler_pc as usize;
                        } else {
                            return Err(JvmError::NativeMethodError(format!(
                                "Unhandled exception: {}",
                                exc_class
                            )));
                        }
                    }
                },
                Err(e) => {
                    let exc_class = match &e {
                        JvmError::NullPointerException => Some("java/lang/NullPointerException"),
                        JvmError::DivisionByZero => Some("java/lang/ArithmeticException"),
                        JvmError::ArrayIndexOutOfBounds(_, _) => {
                            Some("java/lang/ArrayIndexOutOfBoundsException")
                        }
                        _ => None,
                    };
                    if let Some(ec) = exc_class {
                        if let Some(handler_pc) = self.find_exception_handler(f, op_pc, ec) {
                            let exc_id = self.heap.alloc_object(String::from(ec))?;
                            {
                                let exc_obj = self.heap.get_object_mut(exc_id)?;
                                exc_obj.fields.insert(
                                    String::from("detailMessage"),
                                    JvmValue::StringRef(format!("{}", e)),
                                );
                            }
                            f.stack.clear();
                            f.push(JvmValue::ObjectRef(exc_id));
                            f.pc = handler_pc as usize;
                            continue;
                        }
                    }
                    return Err(e);
                }
            }
        }
    }
}

pub fn jvm_value_to_string(val: &JvmValue) -> String {
    match val {
        JvmValue::Int(i) => format!("{}", i),
        JvmValue::Long(l) => format!("{}", l),
        JvmValue::Float(f) => format!("{}", f),
        JvmValue::Double(d) => format!("{}", d),
        JvmValue::StringRef(s) => s.clone(),
        JvmValue::Null => String::from("null"),
        JvmValue::ObjectRef(id) => format!("Object@{}", id),
        JvmValue::ArrayRef(id) => format!("Array@{}", id),
        JvmValue::ReturnAddress(pc) => format!("RetAddr@{}", pc),
    }
}

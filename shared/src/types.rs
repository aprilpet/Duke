use alloc::string::String;
use core::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum JvmValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Null,
    ObjectRef(u32),
    ArrayRef(u32),
    StringRef(String),
    ReturnAddress(usize),
}

impl JvmValue {
    pub fn as_int(&self) -> Result<i32, JvmError> {
        match self {
            JvmValue::Int(v) => Ok(*v),
            _ => Err(JvmError::TypeError(String::from("expected int"))),
        }
    }

    pub fn as_long(&self) -> Result<i64, JvmError> {
        match self {
            JvmValue::Long(v) => Ok(*v),
            _ => Err(JvmError::TypeError(String::from("expected long"))),
        }
    }

    pub fn as_string(&self) -> Result<&str, JvmError> {
        match self {
            JvmValue::StringRef(s) => Ok(s.as_str()),
            _ => Err(JvmError::TypeError(String::from("expected string"))),
        }
    }

    pub fn as_object_ref(&self) -> Result<u32, JvmError> {
        match self {
            JvmValue::ObjectRef(id) => Ok(*id),
            _ => Err(JvmError::TypeError(String::from("expected object ref"))),
        }
    }

    pub fn as_array_ref(&self) -> Result<u32, JvmError> {
        match self {
            JvmValue::ArrayRef(id) => Ok(*id),
            _ => Err(JvmError::TypeError(String::from("expected array ref"))),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, JvmValue::Null)
    }
}

#[derive(Debug)]
pub enum JvmError {
    ClassFormatError(String),
    StackOverflow,
    StackUnderflow,
    TypeError(String),
    UnsupportedOpcode(u8),
    MethodNotFound(String),
    ClassNotFound(String),
    NativeMethodError(String),
    ArrayIndexOutOfBounds(i32, usize),
    NullPointerException,
    OutOfMemory,
    DivisionByZero,
    IoError(String),
    SystemExit(i32),
}

impl fmt::Display for JvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JvmError::ClassFormatError(msg) => write!(f, "ClassFormatError: {}", msg),
            JvmError::StackOverflow => write!(f, "StackOverflow"),
            JvmError::StackUnderflow => write!(f, "StackUnderflow"),
            JvmError::TypeError(msg) => write!(f, "TypeError: {}", msg),
            JvmError::UnsupportedOpcode(op) => write!(f, "UnsupportedOpcode: 0x{:02X}", op),
            JvmError::MethodNotFound(msg) => write!(f, "MethodNotFound: {}", msg),
            JvmError::ClassNotFound(msg) => write!(f, "ClassNotFound: {}", msg),
            JvmError::NativeMethodError(msg) => write!(f, "NativeMethodError: {}", msg),
            JvmError::ArrayIndexOutOfBounds(idx, len) => {
                write!(f, "ArrayIndexOutOfBounds: index {} len {}", idx, len)
            }
            JvmError::NullPointerException => write!(f, "NullPointerException"),
            JvmError::OutOfMemory => write!(f, "OutOfMemory"),
            JvmError::DivisionByZero => write!(f, "ArithmeticException: / by zero"),
            JvmError::IoError(msg) => write!(f, "IoError: {}", msg),
            JvmError::SystemExit(code) => write!(f, "SystemExit: {}", code),
        }
    }
}

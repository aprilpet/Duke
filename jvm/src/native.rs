use shared::types::{
    JvmError,
    JvmValue,
};

pub trait NativeBridge {
    fn call_native(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
        args: &[JvmValue],
    ) -> Result<Option<JvmValue>, JvmError>;
}

pub struct NoopNatives;

impl NativeBridge for NoopNatives {
    fn call_native(
        &mut self,
        class_name: &str,
        method_name: &str,
        _descriptor: &str,
        _args: &[JvmValue],
    ) -> Result<Option<JvmValue>, JvmError> {
        Err(JvmError::NativeMethodError(alloc::format!(
            "no native bridge for {}::{}",
            class_name,
            method_name
        )))
    }
}

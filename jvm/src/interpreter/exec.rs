use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use shared::classfile::CpEntry;
use shared::opcodes::*;
use shared::types::{
    JvmError,
    JvmValue,
};

use super::{
    ExecAction,
    Frame,
    Vm,
};
use crate::native::NativeBridge;

impl<N: NativeBridge> Vm<N> {
    pub(crate) fn exec_one(
        &mut self,
        f: &mut Frame,
        op: u8,
        op_pc: usize,
    ) -> Result<ExecAction, JvmError> {
        match op {
            NOP => {}
            ACONST_NULL => f.push(JvmValue::Null),
            ICONST_M1 => f.push(JvmValue::Int(-1)),
            ICONST_0 => f.push(JvmValue::Int(0)),
            ICONST_1 => f.push(JvmValue::Int(1)),
            ICONST_2 => f.push(JvmValue::Int(2)),
            ICONST_3 => f.push(JvmValue::Int(3)),
            ICONST_4 => f.push(JvmValue::Int(4)),
            ICONST_5 => f.push(JvmValue::Int(5)),
            LCONST_0 => f.push(JvmValue::Long(0)),
            LCONST_1 => f.push(JvmValue::Long(1)),
            FCONST_0 => f.push(JvmValue::Float(0.0)),
            FCONST_1 => f.push(JvmValue::Float(1.0)),
            FCONST_2 => f.push(JvmValue::Float(2.0)),
            DCONST_0 => f.push(JvmValue::Double(0.0)),
            DCONST_1 => f.push(JvmValue::Double(1.0)),

            BIPUSH => {
                let v = f.read_u8() as i8 as i32;
                f.push(JvmValue::Int(v));
            }
            SIPUSH => {
                let v = f.read_i16() as i32;
                f.push(JvmValue::Int(v));
            }
            LDC => {
                let idx = f.read_u8() as u16;
                self.push_ldc(f, idx)?;
            }
            LDC_W => {
                let idx = f.read_u16();
                self.push_ldc(f, idx)?;
            }
            LDC2_W => {
                let idx = f.read_u16();
                let class = &self.classes[f.class_idx];
                match &class.constant_pool[idx as usize] {
                    CpEntry::Long(v) => f.push(JvmValue::Long(*v)),
                    CpEntry::Double(v) => f.push(JvmValue::Double(*v)),
                    _ => {
                        return Err(JvmError::ClassFormatError(format!(
                            "bad ldc2_w at cp#{}",
                            idx
                        )));
                    }
                }
            }

            ILOAD | ALOAD | LLOAD | FLOAD | DLOAD => {
                let idx = f.read_u8() as usize;
                f.push(f.locals[idx].clone());
            }
            ILOAD_0 | ALOAD_0 | FLOAD_0 | DLOAD_0 | LLOAD_0 => f.push(f.locals[0].clone()),
            ILOAD_1 | ALOAD_1 | FLOAD_1 | DLOAD_1 | LLOAD_1 => f.push(f.locals[1].clone()),
            ILOAD_2 | ALOAD_2 | FLOAD_2 | DLOAD_2 | LLOAD_2 => f.push(f.locals[2].clone()),
            ILOAD_3 | ALOAD_3 | FLOAD_3 | DLOAD_3 | LLOAD_3 => f.push(f.locals[3].clone()),

            IALOAD | AALOAD | BALOAD | CALOAD | SALOAD | LALOAD | FALOAD | DALOAD => {
                let index = f.pop_int()?;
                let arr_ref = f.pop()?.as_array_ref()?;
                let arr = self.heap.get_array(arr_ref)?;
                if index < 0 || index as usize >= arr.elements.len() {
                    return Err(JvmError::ArrayIndexOutOfBounds(index, arr.elements.len()));
                }
                f.push(arr.elements[index as usize].clone());
            }

            ISTORE | ASTORE | LSTORE | FSTORE | DSTORE => {
                let idx = f.read_u8() as usize;
                let v = f.pop()?;
                f.locals[idx] = v;
            }
            ISTORE_0 | ASTORE_0 | FSTORE_0 | DSTORE_0 | LSTORE_0 => {
                let v = f.pop()?;
                f.locals[0] = v;
            }
            ISTORE_1 | ASTORE_1 | FSTORE_1 | DSTORE_1 | LSTORE_1 => {
                let v = f.pop()?;
                f.locals[1] = v;
            }
            ISTORE_2 | ASTORE_2 | FSTORE_2 | DSTORE_2 | LSTORE_2 => {
                let v = f.pop()?;
                f.locals[2] = v;
            }
            ISTORE_3 | ASTORE_3 | FSTORE_3 | DSTORE_3 | LSTORE_3 => {
                let v = f.pop()?;
                f.locals[3] = v;
            }

            IASTORE | BASTORE | CASTORE | SASTORE | LASTORE | FASTORE | DASTORE => {
                let val = f.pop()?;
                let index = f.pop_int()?;
                let arr_ref = f.pop()?.as_array_ref()?;
                let arr = self.heap.get_array_mut(arr_ref)?;
                if index < 0 || index as usize >= arr.elements.len() {
                    return Err(JvmError::ArrayIndexOutOfBounds(index, arr.elements.len()));
                }
                arr.elements[index as usize] = val;
            }
            AASTORE => {
                let val = f.pop()?;
                let index = f.pop_int()?;
                let arr_ref = f.pop()?.as_array_ref()?;
                let arr = self.heap.get_array_mut(arr_ref)?;
                if index < 0 || index as usize >= arr.elements.len() {
                    return Err(JvmError::ArrayIndexOutOfBounds(index, arr.elements.len()));
                }
                arr.elements[index as usize] = val;
            }

            POP => {
                f.pop()?;
            }
            POP2 => {
                f.pop()?;
                f.pop()?;
            }
            DUP => {
                let v = f.pop()?;
                f.push(v.clone());
                f.push(v);
            }
            DUP_X1 => {
                let v1 = f.pop()?;
                let v2 = f.pop()?;
                f.push(v1.clone());
                f.push(v2);
                f.push(v1);
            }
            DUP_X2 => {
                let v1 = f.pop()?;
                let v2 = f.pop()?;
                let v3 = f.pop()?;
                f.push(v1.clone());
                f.push(v3);
                f.push(v2);
                f.push(v1);
            }
            DUP2 => {
                let v1 = f.pop()?;
                let v2 = f.pop()?;
                f.push(v2.clone());
                f.push(v1.clone());
                f.push(v2);
                f.push(v1);
            }
            DUP2_X1 => {
                let v1 = f.pop()?;
                let v2 = f.pop()?;
                let v3 = f.pop()?;
                f.push(v2.clone());
                f.push(v1.clone());
                f.push(v3);
                f.push(v2);
                f.push(v1);
            }
            DUP2_X2 => {
                let v1 = f.pop()?;
                let v2 = f.pop()?;
                let v3 = f.pop()?;
                let v4 = f.pop()?;
                f.push(v2.clone());
                f.push(v1.clone());
                f.push(v4);
                f.push(v3);
                f.push(v2);
                f.push(v1);
            }
            SWAP => {
                let b = f.pop()?;
                let a = f.pop()?;
                f.push(b);
                f.push(a);
            }

            IADD => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(a.wrapping_add(b)));
            }
            LADD => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(a.wrapping_add(b)));
            }
            FADD => {
                let b = f.pop_float()?;
                let a = f.pop_float()?;
                f.push(JvmValue::Float(a + b));
            }
            DADD => {
                let b = f.pop_double()?;
                let a = f.pop_double()?;
                f.push(JvmValue::Double(a + b));
            }

            ISUB => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(a.wrapping_sub(b)));
            }
            LSUB => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(a.wrapping_sub(b)));
            }
            FSUB => {
                let b = f.pop_float()?;
                let a = f.pop_float()?;
                f.push(JvmValue::Float(a - b));
            }
            DSUB => {
                let b = f.pop_double()?;
                let a = f.pop_double()?;
                f.push(JvmValue::Double(a - b));
            }

            IMUL => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(a.wrapping_mul(b)));
            }
            LMUL => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(a.wrapping_mul(b)));
            }
            FMUL => {
                let b = f.pop_float()?;
                let a = f.pop_float()?;
                f.push(JvmValue::Float(a * b));
            }
            DMUL => {
                let b = f.pop_double()?;
                let a = f.pop_double()?;
                f.push(JvmValue::Double(a * b));
            }

            IDIV => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                if b == 0 {
                    return Err(JvmError::DivisionByZero);
                }
                f.push(JvmValue::Int(a.wrapping_div(b)));
            }
            LDIV => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                if b == 0 {
                    return Err(JvmError::DivisionByZero);
                }
                f.push(JvmValue::Long(a.wrapping_div(b)));
            }
            FDIV => {
                let b = f.pop_float()?;
                let a = f.pop_float()?;
                f.push(JvmValue::Float(a / b));
            }
            DDIV => {
                let b = f.pop_double()?;
                let a = f.pop_double()?;
                f.push(JvmValue::Double(a / b));
            }

            IREM => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                if b == 0 {
                    return Err(JvmError::DivisionByZero);
                }
                f.push(JvmValue::Int(a.wrapping_rem(b)));
            }
            LREM => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                if b == 0 {
                    return Err(JvmError::DivisionByZero);
                }
                f.push(JvmValue::Long(a.wrapping_rem(b)));
            }
            FREM => {
                let b = f.pop_float()?;
                let a = f.pop_float()?;
                f.push(JvmValue::Float(a % b));
            }
            DREM => {
                let b = f.pop_double()?;
                let a = f.pop_double()?;
                f.push(JvmValue::Double(a % b));
            }

            INEG => {
                let v = f.pop_int()?;
                f.push(JvmValue::Int(v.wrapping_neg()));
            }
            LNEG => {
                let v = f.pop_long()?;
                f.push(JvmValue::Long(v.wrapping_neg()));
            }
            FNEG => {
                let v = f.pop_float()?;
                f.push(JvmValue::Float(-v));
            }
            DNEG => {
                let v = f.pop_double()?;
                f.push(JvmValue::Double(-v));
            }

            ISHL => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(a.wrapping_shl(b as u32 & 0x1f)));
            }
            LSHL => {
                let b = f.pop_int()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(a.wrapping_shl(b as u32 & 0x3f)));
            }
            ISHR => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(a.wrapping_shr(b as u32 & 0x1f)));
            }
            LSHR => {
                let b = f.pop_int()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(a.wrapping_shr(b as u32 & 0x3f)));
            }
            IUSHR => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(
                    ((a as u32).wrapping_shr(b as u32 & 0x1f)) as i32,
                ));
            }
            LUSHR => {
                let b = f.pop_int()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(
                    ((a as u64).wrapping_shr(b as u32 & 0x3f)) as i64,
                ));
            }
            IAND => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(a & b));
            }
            LAND => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(a & b));
            }
            IOR => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(a | b));
            }
            LOR => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(a | b));
            }
            IXOR => {
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                f.push(JvmValue::Int(a ^ b));
            }
            LXOR => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                f.push(JvmValue::Long(a ^ b));
            }
            IINC => {
                let idx = f.read_u8() as usize;
                let inc = f.read_u8() as i8 as i32;
                if let JvmValue::Int(v) = &mut f.locals[idx] {
                    *v = v.wrapping_add(inc);
                }
            }

            I2L => {
                let v = f.pop_int()?;
                f.push(JvmValue::Long(v as i64));
            }
            I2F => {
                let v = f.pop_int()?;
                f.push(JvmValue::Float(v as f32));
            }
            I2D => {
                let v = f.pop_int()?;
                f.push(JvmValue::Double(v as f64));
            }
            L2I => {
                let v = f.pop_long()?;
                f.push(JvmValue::Int(v as i32));
            }
            L2F => {
                let v = f.pop_long()?;
                f.push(JvmValue::Float(v as f32));
            }
            L2D => {
                let v = f.pop_long()?;
                f.push(JvmValue::Double(v as f64));
            }
            F2I => {
                let v = f.pop_float()?;
                f.push(JvmValue::Int(v as i32));
            }
            F2L => {
                let v = f.pop_float()?;
                f.push(JvmValue::Long(v as i64));
            }
            F2D => {
                let v = f.pop_float()?;
                f.push(JvmValue::Double(v as f64));
            }
            D2I => {
                let v = f.pop_double()?;
                f.push(JvmValue::Int(v as i32));
            }
            D2L => {
                let v = f.pop_double()?;
                f.push(JvmValue::Long(v as i64));
            }
            D2F => {
                let v = f.pop_double()?;
                f.push(JvmValue::Float(v as f32));
            }
            I2B => {
                let v = f.pop_int()?;
                f.push(JvmValue::Int(v as i8 as i32));
            }
            I2C => {
                let v = f.pop_int()?;
                f.push(JvmValue::Int(v as u16 as i32));
            }
            I2S => {
                let v = f.pop_int()?;
                f.push(JvmValue::Int(v as i16 as i32));
            }

            LCMP => {
                let b = f.pop_long()?;
                let a = f.pop_long()?;
                let r = if a > b {
                    1
                } else if a == b {
                    0
                } else {
                    -1
                };
                f.push(JvmValue::Int(r));
            }
            FCMPL => {
                let b = f.pop_float()?;
                let a = f.pop_float()?;
                let r = if a.is_nan() || b.is_nan() {
                    -1
                } else if a > b {
                    1
                } else if a == b {
                    0
                } else {
                    -1
                };
                f.push(JvmValue::Int(r));
            }
            FCMPG => {
                let b = f.pop_float()?;
                let a = f.pop_float()?;
                let r = if a.is_nan() || b.is_nan() {
                    1
                } else if a > b {
                    1
                } else if a == b {
                    0
                } else {
                    -1
                };
                f.push(JvmValue::Int(r));
            }
            DCMPL => {
                let b = f.pop_double()?;
                let a = f.pop_double()?;
                let r = if a.is_nan() || b.is_nan() {
                    -1
                } else if a > b {
                    1
                } else if a == b {
                    0
                } else {
                    -1
                };
                f.push(JvmValue::Int(r));
            }
            DCMPG => {
                let b = f.pop_double()?;
                let a = f.pop_double()?;
                let r = if a.is_nan() || b.is_nan() {
                    1
                } else if a > b {
                    1
                } else if a == b {
                    0
                } else {
                    -1
                };
                f.push(JvmValue::Int(r));
            }

            IFEQ => {
                let off = f.read_i16();
                let v = f.pop_int()?;
                if v == 0 {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IFNE => {
                let off = f.read_i16();
                let v = f.pop_int()?;
                if v != 0 {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IFLT => {
                let off = f.read_i16();
                let v = f.pop_int()?;
                if v < 0 {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IFGE => {
                let off = f.read_i16();
                let v = f.pop_int()?;
                if v >= 0 {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IFGT => {
                let off = f.read_i16();
                let v = f.pop_int()?;
                if v > 0 {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IFLE => {
                let off = f.read_i16();
                let v = f.pop_int()?;
                if v <= 0 {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }

            IF_ICMPEQ => {
                let off = f.read_i16();
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                if a == b {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IF_ICMPNE => {
                let off = f.read_i16();
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                if a != b {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IF_ICMPLT => {
                let off = f.read_i16();
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                if a < b {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IF_ICMPGE => {
                let off = f.read_i16();
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                if a >= b {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IF_ICMPGT => {
                let off = f.read_i16();
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                if a > b {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IF_ICMPLE => {
                let off = f.read_i16();
                let b = f.pop_int()?;
                let a = f.pop_int()?;
                if a <= b {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }

            IF_ACMPEQ => {
                let off = f.read_i16();
                let b = f.pop()?;
                let a = f.pop()?;
                if self.refs_equal(&a, &b) {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IF_ACMPNE => {
                let off = f.read_i16();
                let b = f.pop()?;
                let a = f.pop()?;
                if !self.refs_equal(&a, &b) {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IFNULL => {
                let off = f.read_i16();
                let v = f.pop()?;
                if v.is_null() {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }
            IFNONNULL => {
                let off = f.read_i16();
                let v = f.pop()?;
                if !v.is_null() {
                    f.pc = (op_pc as isize + off as isize) as usize;
                }
            }

            GOTO => {
                let off = f.read_i16();
                f.pc = (op_pc as isize + off as isize) as usize;
            }
            GOTO_W => {
                let off = f.read_i32();
                f.pc = (op_pc as isize + off as isize) as usize;
            }

            TABLESWITCH => {
                let base = op_pc + 1;
                f.pc = (base + 3) & !3;
                let default_off = f.read_i32();
                let low = f.read_i32();
                let high = f.read_i32();
                let index = f.pop_int()?;
                if index >= low && index <= high {
                    let entry = (index - low) as usize;
                    f.pc = (base + 3) & !3;
                    f.pc += 12 + entry * 4;
                    let off = f.read_i32();
                    f.pc = (op_pc as isize + off as isize) as usize;
                } else {
                    f.pc = (op_pc as isize + default_off as isize) as usize;
                }
            }
            LOOKUPSWITCH => {
                let base = op_pc + 1;
                f.pc = (base + 3) & !3;
                let default_off = f.read_i32();
                let npairs = f.read_i32();
                let key = f.pop_int()?;
                let pairs_start = f.pc;
                let mut found = false;
                for i in 0..npairs as usize {
                    f.pc = pairs_start + i * 8;
                    let match_val = f.read_i32();
                    let off = f.read_i32();
                    if key == match_val {
                        f.pc = (op_pc as isize + off as isize) as usize;
                        found = true;
                        break;
                    }
                }
                if !found {
                    f.pc = (op_pc as isize + default_off as isize) as usize;
                }
            }

            IRETURN | LRETURN | FRETURN | DRETURN | ARETURN => {
                return Ok(ExecAction::ReturnVal(f.pop()?));
            }
            RETURN => {
                return Ok(ExecAction::ReturnVoid);
            }

            GETSTATIC => {
                let idx = f.read_u16();
                self.do_getstatic(f, idx)?;
            }
            PUTSTATIC => {
                let idx = f.read_u16();
                let val = f.pop()?;
                let class = &self.classes[f.class_idx];
                if let CpEntry::Fieldref {
                    class_index,
                    name_and_type_index,
                } = &class.constant_pool[idx as usize]
                {
                    let cn = class.get_class_name(*class_index)?;
                    let (field_name, _) = class.resolve_name_and_type(*name_and_type_index)?;
                    let key = format!("{}.{}", cn, field_name);
                    self.statics.insert(key, val);
                }
            }
            GETFIELD => {
                let idx = f.read_u16();
                self.do_getfield(f, idx)?;
            }
            PUTFIELD => {
                let idx = f.read_u16();
                self.do_putfield(f, idx)?;
            }

            INVOKEVIRTUAL | INVOKESPECIAL | INVOKESTATIC => {
                let idx = f.read_u16();
                self.do_invoke(f, op, idx)?;
            }

            INVOKEINTERFACE => {
                let idx = f.read_u16();
                let _count = f.read_u8();
                let _zero = f.read_u8();
                self.do_invoke(f, INVOKEVIRTUAL, idx)?;
            }

            INVOKEDYNAMIC => {
                let idx = f.read_u16();
                let _zero = f.read_u16();
                self.do_invokedynamic(f, idx)?;
            }

            NEW => {
                let idx = f.read_u16();
                let class = &self.classes[f.class_idx];
                let name = class.get_class_name(idx)?;
                let cn = String::from(name);
                let id = self.heap.alloc_object(cn)?;
                f.push(JvmValue::ObjectRef(id));
            }

            NEWARRAY => {
                let atype = f.read_u8();
                let count = f.pop_int()?;
                let elem = match atype {
                    4 => "boolean",
                    5 => "char",
                    6 => "float",
                    7 => "double",
                    8 => "byte",
                    9 => "short",
                    10 => "int",
                    11 => "long",
                    _ => {
                        return Err(JvmError::ClassFormatError(format!(
                            "bad newarray type {}",
                            atype
                        )));
                    }
                };
                let id = self.heap.alloc_array(String::from(elem), count as usize)?;
                f.push(JvmValue::ArrayRef(id));
            }
            ANEWARRAY => {
                let _class_idx = f.read_u16();
                let count = f.pop_int()?;
                let id = self
                    .heap
                    .alloc_array(String::from("object"), count as usize)?;
                f.push(JvmValue::ArrayRef(id));
            }
            MULTIANEWARRAY => {
                let _class_idx = f.read_u16();
                let dimensions = f.read_u8() as usize;
                let mut counts = Vec::with_capacity(dimensions);
                for _ in 0..dimensions {
                    counts.push(f.pop_int()?);
                }
                counts.reverse();
                let id = self
                    .heap
                    .alloc_array(String::from("object"), counts[0] as usize)?;
                f.push(JvmValue::ArrayRef(id));
            }
            ARRAYLENGTH => {
                let arr_ref = f.pop()?.as_array_ref()?;
                let arr = self.heap.get_array(arr_ref)?;
                f.push(JvmValue::Int(arr.elements.len() as i32));
            }

            ATHROW => {
                let exc_val = f.pop()?;
                let exc_class = match &exc_val {
                    JvmValue::ObjectRef(id) => {
                        let obj = self.heap.get_object(*id)?;
                        obj.class_name.clone()
                    }
                    _ => String::from("java/lang/Throwable"),
                };
                return Ok(ExecAction::Throw(exc_class, exc_val));
            }

            CHECKCAST => {
                let idx = f.read_u16();
                let val = f.pop()?;
                if !val.is_null() {
                    let class = &self.classes[f.class_idx];
                    let target_name = class.get_class_name(idx)?;
                    let target_owned = String::from(target_name);
                    let ok = match &val {
                        JvmValue::ObjectRef(id) => {
                            let obj = self.heap.get_object(*id)?;
                            self.is_subclass(&obj.class_name, &target_owned)
                        }
                        _ => true,
                    };
                    if !ok {
                        let exc_id = self
                            .heap
                            .alloc_object(String::from("java/lang/ClassCastException"))?;
                        return Ok(ExecAction::Throw(
                            String::from("java/lang/ClassCastException"),
                            JvmValue::ObjectRef(exc_id),
                        ));
                    }
                }
                f.push(val);
            }
            INSTANCEOF => {
                let idx = f.read_u16();
                let val = f.pop()?;
                if val.is_null() {
                    f.push(JvmValue::Int(0));
                } else {
                    let class = &self.classes[f.class_idx];
                    let target_name = class.get_class_name(idx)?;
                    let target_owned = String::from(target_name);
                    let result = match &val {
                        JvmValue::ObjectRef(id) => {
                            let obj = self.heap.get_object(*id)?;
                            self.is_subclass(&obj.class_name, &target_owned)
                        }
                        _ => false,
                    };
                    f.push(JvmValue::Int(if result { 1 } else { 0 }));
                }
            }

            MONITORENTER | MONITOREXIT => {
                f.pop()?;
            }

            WIDE => {
                let wide_op = f.read_u8();
                match wide_op {
                    ILOAD | LLOAD | FLOAD | DLOAD | ALOAD => {
                        let idx = f.read_u16() as usize;
                        f.push(f.locals[idx].clone());
                    }
                    ISTORE | LSTORE | FSTORE | DSTORE | ASTORE => {
                        let idx = f.read_u16() as usize;
                        let v = f.pop()?;
                        f.locals[idx] = v;
                    }
                    IINC => {
                        let idx = f.read_u16() as usize;
                        let inc = f.read_i16() as i32;
                        if let JvmValue::Int(v) = &mut f.locals[idx] {
                            *v = v.wrapping_add(inc);
                        }
                    }
                    _ => return Err(JvmError::UnsupportedOpcode(wide_op)),
                }
            }

            _ => return Err(JvmError::UnsupportedOpcode(op)),
        }
        Ok(ExecAction::Continue)
    }

    fn refs_equal(&self, a: &JvmValue, b: &JvmValue) -> bool {
        match (a, b) {
            (JvmValue::Null, JvmValue::Null) => true,
            (JvmValue::ObjectRef(x), JvmValue::ObjectRef(y)) => x == y,
            (JvmValue::ArrayRef(x), JvmValue::ArrayRef(y)) => x == y,
            _ => false,
        }
    }

    fn push_ldc(&self, f: &mut Frame, idx: u16) -> Result<(), JvmError> {
        let class = &self.classes[f.class_idx];
        match &class.constant_pool[idx as usize] {
            CpEntry::Integer(v) => f.push(JvmValue::Int(*v)),
            CpEntry::Float(v) => f.push(JvmValue::Float(*v)),
            CpEntry::Long(v) => f.push(JvmValue::Long(*v)),
            CpEntry::Double(v) => f.push(JvmValue::Double(*v)),
            CpEntry::StringRef { string_index } => {
                let s = class.get_utf8(*string_index)?;
                f.push(JvmValue::StringRef(String::from(s)));
            }
            CpEntry::Class { .. } => {
                f.push(JvmValue::Null);
            }
            _ => {
                return Err(JvmError::ClassFormatError(format!(
                    "unsupported ldc at cp#{}",
                    idx
                )));
            }
        }
        Ok(())
    }
}

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use shared::types::{
    JvmError,
    JvmValue,
};

#[derive(Debug, Clone)]
pub struct JvmObject {
    pub class_name: String,
    pub fields: BTreeMap<String, JvmValue>,
}

#[derive(Debug, Clone)]
pub struct JvmArray {
    pub element_type: String,
    pub elements: Vec<JvmValue>,
}

enum HeapSlot<T> {
    Live(T),
    Free(Option<u32>),
}

struct SlabHeap<T> {
    slots: Vec<HeapSlot<T>>,
    free_head: Option<u32>,
}

impl<T> SlabHeap<T> {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
        }
    }

    fn alloc(&mut self, val: T) -> u32 {
        if let Some(idx) = self.free_head {
            let next = match &self.slots[idx as usize] {
                HeapSlot::Free(n) => *n,
                _ => None,
            };
            self.slots[idx as usize] = HeapSlot::Live(val);
            self.free_head = next;
            idx
        } else {
            let idx = self.slots.len() as u32;
            self.slots.push(HeapSlot::Live(val));
            idx
        }
    }

    fn get(&self, id: u32) -> Result<&T, JvmError> {
        match self.slots.get(id as usize) {
            Some(HeapSlot::Live(v)) => Ok(v),
            _ => Err(JvmError::NullPointerException),
        }
    }

    fn get_mut(&mut self, id: u32) -> Result<&mut T, JvmError> {
        match self.slots.get_mut(id as usize) {
            Some(HeapSlot::Live(v)) => Ok(v),
            _ => Err(JvmError::NullPointerException),
        }
    }

    #[allow(dead_code)]
    fn free(&mut self, id: u32) {
        if (id as usize) < self.slots.len() {
            self.slots[id as usize] = HeapSlot::Free(self.free_head);
            self.free_head = Some(id);
        }
    }
}

pub struct Heap {
    objects: SlabHeap<JvmObject>,
    arrays: SlabHeap<JvmArray>,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            objects: SlabHeap::new(),
            arrays: SlabHeap::new(),
        }
    }

    pub fn alloc_object(&mut self, class_name: String) -> Result<u32, JvmError> {
        Ok(self.objects.alloc(JvmObject {
            class_name,
            fields: BTreeMap::new(),
        }))
    }

    pub fn get_object(&self, id: u32) -> Result<&JvmObject, JvmError> {
        self.objects.get(id)
    }

    pub fn get_object_mut(&mut self, id: u32) -> Result<&mut JvmObject, JvmError> {
        self.objects.get_mut(id)
    }

    #[allow(dead_code)]
    pub fn free_object(&mut self, id: u32) {
        self.objects.free(id);
    }

    pub fn alloc_array(&mut self, element_type: String, size: usize) -> Result<u32, JvmError> {
        let default = match element_type.as_str() {
            "int" | "byte" | "char" | "short" | "boolean" => JvmValue::Int(0),
            "long" => JvmValue::Long(0),
            "float" => JvmValue::Float(0.0),
            "double" => JvmValue::Double(0.0),
            _ => JvmValue::Null,
        };
        Ok(self.arrays.alloc(JvmArray {
            element_type,
            elements: alloc::vec![default; size],
        }))
    }

    pub fn get_array(&self, id: u32) -> Result<&JvmArray, JvmError> {
        self.arrays.get(id)
    }

    pub fn get_array_mut(&mut self, id: u32) -> Result<&mut JvmArray, JvmError> {
        self.arrays.get_mut(id)
    }

    #[allow(dead_code)]
    pub fn free_array(&mut self, id: u32) {
        self.arrays.free(id);
    }
}

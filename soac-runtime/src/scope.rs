use alloc::vec::Vec;
use core::ptr::null_mut;
use pyo3_ffi::PyObject;

#[derive(Clone, Copy)]
pub struct ScopeLayout {
    names: &'static [&'static str],
}

impl ScopeLayout {
    pub const fn new(names: &'static [&'static str]) -> Self {
        Self { names }
    }

    pub const fn len(&self) -> usize {
        self.names.len()
    }

    pub fn slot(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|entry| *entry == name)
    }

    pub fn name(&self, slot: usize) -> Option<&'static str> {
        self.names.get(slot).copied()
    }
}

pub struct Scope {
    layout: &'static ScopeLayout,
    slots: Vec<*mut PyObject>,
}

impl Scope {
    pub fn new(layout: &'static ScopeLayout) -> Self {
        let slots = alloc::vec![null_mut(); layout.len()];
        Self { layout, slots }
    }

    pub fn layout(&self) -> &'static ScopeLayout {
        self.layout
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn get_slot(&self, slot: usize) -> *mut PyObject {
        self.slots[slot]
    }

    pub fn set_slot(&mut self, slot: usize, value: *mut PyObject) {
        self.slots[slot] = value;
    }

    pub fn get(&self, name: &str) -> Option<*mut PyObject> {
        let slot = self.layout.slot(name)?;
        Some(self.slots[slot])
    }

    pub fn set(&mut self, name: &str, value: *mut PyObject) -> bool {
        match self.layout.slot(name) {
            Some(slot) => {
                self.slots[slot] = value;
                true
            }
            None => false,
        }
    }
}

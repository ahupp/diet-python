use std::collections::HashMap;

use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::module_symbols::ModuleSymbols;

#[derive(Debug)]
pub struct Scope {
    values: Vec<Option<*mut ffi::PyObject>>,
    name_to_index: HashMap<String, usize>,
}

impl Default for Scope {
    fn default() -> Self {
        Scope {
            values: Vec::new(),
            name_to_index: HashMap::new(),
        }
    }
}

impl Scope {
    pub fn new(symbols: &ModuleSymbols, extra_names: &[&str]) -> Self {
        let values = vec![None; symbols.globals.len() + extra_names.len()];
        let mut name_to_index = HashMap::new();
        for (name, meta) in &symbols.globals {
            name_to_index.insert(name.clone(), meta.index);
        }
        let mut next_index = symbols.globals.len();
        for &name in extra_names {
            name_to_index.insert(name.to_string(), next_index);
            next_index += 1;
        }
        Scope {
            values,
            name_to_index,
        }
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.name_to_index.get(name).copied()
    }

    pub fn get_by_index(&self, index: usize) -> Option<*mut ffi::PyObject> {
        self.values[index]
    }

    pub fn set_by_index(&mut self, index: usize, mut value: Option<*mut ffi::PyObject>) {
        unsafe {
            if let Some(v) = value {
                ffi::Py_XINCREF(v);
            }
            std::mem::swap(&mut self.values[index], &mut value);
            if let Some(old) = value {
                ffi::Py_XDECREF(old);
            }
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        for &ptr in &self.values {
            if let Some(ptr) = ptr {
                unsafe { ffi::Py_XDECREF(ptr) };
            }
        }
    }
}

pub struct ScopeStack {
    builtins: Py<PyModule>,
    globals: Scope,
}

impl ScopeStack {
    pub fn new(builtins: Py<PyModule>, globals: Scope) -> Self {
        ScopeStack { builtins, globals }
    }

    pub fn get_by_name(&self, py: Python<'_>, name: &str) -> Option<PyObject> {
        if let Some(idx) = self.index_of(name) {
            if let Some(obj) = self.get_by_index(idx) {
                Some(unsafe { PyObject::from_borrowed_ptr(py, obj) })
            } else {
                Some(py.None())
            }
        } else {
            self.builtins
                .as_ref(py)
                .getattr(name)
                .ok()
                .map(|o| o.into())
        }
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.globals.index_of(name)
    }

    pub fn get_by_index(&self, index: usize) -> Option<*mut ffi::PyObject> {
        self.globals.get_by_index(index)
    }

    pub fn set_by_index(&mut self, index: usize, value: Option<*mut ffi::PyObject>) {
        self.globals.set_by_index(index, value)
    }

    pub fn into_globals(self) -> Scope {
        self.globals
    }
}


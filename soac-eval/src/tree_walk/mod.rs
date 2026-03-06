use pyo3::ffi;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

mod eval;
mod runtime;
mod scope;

pub use eval::{compile_clif_vectorcall, register_clif_vectorcall};
pub use runtime::RuntimeFns;
pub use scope::{
    ScopeInstance, ScopeLayout, scope_assign_name, scope_clear_objects, scope_delete_name,
    scope_lookup_name, scope_to_dict, scope_traverse_objects,
};

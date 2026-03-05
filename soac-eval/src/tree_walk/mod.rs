use pyo3::ffi;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Once;

mod eval;
mod runtime;
mod scope;

pub use eval::{
    compile_clif_wrapper_code_extra, install_eval_frame_hook, register_clif_wrapper_code_extra,
};
pub use runtime::RuntimeFns;
pub use scope::{
    ScopeInstance, ScopeLayout, scope_assign_name, scope_clear_objects, scope_delete_name,
    scope_lookup_name, scope_to_dict, scope_traverse_objects,
};

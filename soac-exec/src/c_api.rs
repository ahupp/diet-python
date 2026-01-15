use crate::evaluate::evaluate_call_from_py;
use diet_python::min_ast;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use std::sync::Once;

#[repr(C)]
struct FunctionCallable {
    ob_base: ffi::PyObject,
    func: *const min_ast::FunctionDef,
}

unsafe extern "C" fn function_call(
    self_: *mut ffi::PyObject,
    args: *mut ffi::PyObject,
    kwargs: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    Python::with_gil(|py| {
        let callable = self_ as *mut FunctionCallable;
        let func = &*(*callable).func;
        let args_tuple = py.from_borrowed_ptr::<PyTuple>(args);
        let kwargs_dict = if kwargs.is_null() {
            None
        } else {
            Some(py.from_borrowed_ptr::<PyDict>(kwargs))
        };
        match evaluate_call_from_py(py, func, args_tuple, kwargs_dict) {
            Ok(obj) => obj.into_ptr(),
            Err(err) => {
                err.restore(py);
                std::ptr::null_mut()
            }
        }
    })
}

#[allow(clippy::uninit_assumed_init)]
static mut FUNCTION_CALLABLE_TYPE: ffi::PyTypeObject = ffi::PyTypeObject {
    ob_base: ffi::PyVarObject {
        ob_base: ffi::PyObject_HEAD_INIT,
        ob_size: 0,
    },
    tp_name: b"soac_exec.FunctionCallable\0".as_ptr() as *const _,
    tp_basicsize: std::mem::size_of::<FunctionCallable>() as ffi::Py_ssize_t,
    tp_itemsize: 0,
    tp_call: Some(function_call),
    tp_flags: ffi::Py_TPFLAGS_DEFAULT,
    tp_new: Some(ffi::PyType_GenericNew),
    ..unsafe { std::mem::zeroed() }
};

unsafe fn init_type() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = ffi::PyType_Ready(std::ptr::addr_of_mut!(FUNCTION_CALLABLE_TYPE));
    });
}

pub fn callable_from_functiondef(func: &min_ast::FunctionDef) -> *mut ffi::PyObject {
    unsafe {
        init_type();
        let obj = ffi::_PyObject_New(std::ptr::addr_of_mut!(FUNCTION_CALLABLE_TYPE))
            as *mut FunctionCallable;
        if obj.is_null() {
            return std::ptr::null_mut();
        }
        (*obj).func = func as *const _;
        obj as *mut ffi::PyObject
    }
}

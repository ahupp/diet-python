use pyo3::ffi;
use pyo3::prelude::*;
use soac_exec::cranelift;
use std::{env, ffi::CString};

unsafe extern "C" {
    static mut stdout: *mut libc::FILE;
}

fn main() {
    let module_name = env::args()
        .nth(1)
        .unwrap_or_else(|| "jitmodule".to_string());

    Python::with_gil(|_| unsafe {
        let c_str = CString::new("hello, world").unwrap();
        let msg = ffi::PyUnicode_FromString(c_str.as_ptr());

        let func = cranelift::build_jit(msg, stdout);

        let module_name_c = CString::new(module_name).unwrap();
        let module = ffi::PyModule_New(module_name_c.as_ptr());

        let func_name = CString::new("run").unwrap();
        let func_name_ptr = func_name.into_raw();
        let method = Box::into_raw(Box::new(ffi::PyMethodDef {
            ml_name: func_name_ptr,
            ml_meth: ffi::PyMethodDefPointer { PyCFunction: func },
            ml_flags: ffi::METH_NOARGS,
            ml_doc: std::ptr::null(),
        }));
        let func_obj = ffi::PyCFunction_NewEx(method, std::ptr::null_mut(), std::ptr::null_mut());
        ffi::PyModule_AddObject(module, func_name_ptr, func_obj);

        ffi::PyObject_CallNoArgs(func_obj);

        ffi::Py_DecRef(msg);
        ffi::Py_DecRef(module);
        libc::fflush(stdout);
    });
}

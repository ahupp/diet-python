use super::*;

pub struct RuntimeFns {
    pub(crate) builtins_globals: *mut ffi::PyObject,
    pub(crate) builtins_locals: *mut ffi::PyObject,
    pub(crate) dp_globals: *mut ffi::PyObject,
    pub(crate) dp_locals: *mut ffi::PyObject,
}

impl RuntimeFns {
    pub unsafe fn new(
        builtins: *mut ffi::PyObject,
        dp_module: *mut ffi::PyObject,
    ) -> Result<Self, ()> {
        unsafe {
            let builtins_globals =
                ffi::PyDict_GetItemString(builtins, CString::new("globals").unwrap().as_ptr());
            let builtins_locals =
                ffi::PyDict_GetItemString(builtins, CString::new("locals").unwrap().as_ptr());
            if builtins_globals.is_null() || builtins_locals.is_null() {
                ffi::PyErr_SetString(
                    ffi::PyExc_RuntimeError,
                    b"missing builtins globals/locals\0".as_ptr() as *const c_char,
                );
                return Err(());
            }
            let dp_globals =
                ffi::PyObject_GetAttrString(dp_module, CString::new("globals").unwrap().as_ptr());
            if dp_globals.is_null() {
                return Err(());
            }
            let dp_locals =
                ffi::PyObject_GetAttrString(dp_module, CString::new("locals").unwrap().as_ptr());
            if dp_locals.is_null() {
                ffi::Py_DECREF(dp_globals);
                return Err(());
            }
            ffi::Py_INCREF(builtins_globals);
            ffi::Py_INCREF(builtins_locals);
            Ok(Self {
                builtins_globals,
                builtins_locals,
                dp_globals,
                dp_locals,
            })
        }
    }

    fn inc_ref_all(&self) {
        unsafe {
            ffi::Py_INCREF(self.builtins_globals);
            ffi::Py_INCREF(self.builtins_locals);
            ffi::Py_INCREF(self.dp_globals);
            ffi::Py_INCREF(self.dp_locals);
        }
    }
}

impl Drop for RuntimeFns {
    fn drop(&mut self) {
        unsafe {
            ffi::Py_DECREF(self.builtins_globals);
            ffi::Py_DECREF(self.builtins_locals);
            ffi::Py_DECREF(self.dp_globals);
            ffi::Py_DECREF(self.dp_locals);
        }
    }
}

impl Clone for RuntimeFns {
    fn clone(&self) -> Self {
        self.inc_ref_all();
        Self {
            builtins_globals: self.builtins_globals,
            builtins_locals: self.builtins_locals,
            dp_globals: self.dp_globals,
            dp_locals: self.dp_locals,
        }
    }
}

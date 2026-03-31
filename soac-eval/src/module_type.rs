use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;
use soac_blockpy::block_py::BlockPyModule;
use soac_blockpy::passes::CodegenBlockPyPass;
use std::ffi::{c_int, c_void};
use std::ptr;

#[derive(Clone)]
pub struct SoacExtModuleData {
    pub lowered_module: BlockPyModule<CodegenBlockPyPass>,
    pub module_name: String,
    pub package_name: String,
}

#[repr(C)]
struct SoacExtModuleState {
    data: *mut SoacExtModuleData,
}

unsafe extern "C" fn soac_ext_module_traverse(
    _module: *mut ffi::PyObject,
    _visit: ffi::visitproc,
    _arg: *mut c_void,
) -> c_int {
    0
}

unsafe extern "C" fn soac_ext_module_clear(module: *mut ffi::PyObject) -> c_int {
    let state = unsafe { ffi::PyModule_GetState(module) }.cast::<SoacExtModuleState>();
    if state.is_null() {
        return 0;
    }
    let data = unsafe { (*state).data };
    if !data.is_null() {
        unsafe {
            drop(Box::from_raw(data));
            (*state).data = ptr::null_mut();
        }
    }
    0
}

unsafe extern "C" fn soac_ext_module_free(module: *mut c_void) {
    unsafe {
        soac_ext_module_clear(module.cast());
    }
}

static mut SOAC_EXT_MODULE_DEF: ffi::PyModuleDef = ffi::PyModuleDef {
    m_base: ffi::PyModuleDef_HEAD_INIT,
    m_name: c"_soac_ext.module_state".as_ptr(),
    m_doc: ptr::null(),
    m_size: std::mem::size_of::<SoacExtModuleState>() as ffi::Py_ssize_t,
    m_methods: ptr::null_mut(),
    m_slots: ptr::null_mut(),
    m_traverse: Some(soac_ext_module_traverse),
    m_clear: Some(soac_ext_module_clear),
    m_free: Some(soac_ext_module_free),
};

fn soac_ext_module_def() -> *mut ffi::PyModuleDef {
    ptr::addr_of_mut!(SOAC_EXT_MODULE_DEF)
}

fn soac_ext_module_state(module: &Bound<'_, PyAny>) -> PyResult<*mut SoacExtModuleState> {
    unsafe {
        let module_def = ffi::PyModule_GetDef(module.as_ptr());
        if module_def != soac_ext_module_def() {
            return Err(PyTypeError::new_err(
                "expected a module created via _soac_ext.create_module",
            ));
        }
        let state = ffi::PyModule_GetState(module.as_ptr()).cast::<SoacExtModuleState>();
        if state.is_null() {
            if ffi::PyErr_Occurred().is_null() {
                Err(PyRuntimeError::new_err(
                    "missing _soac_ext module state for transformed module",
                ))
            } else {
                Err(PyErr::fetch(module.py()))
            }
        } else {
            Ok(state)
        }
    }
}

pub struct SoacExtModule;

impl SoacExtModule {
    pub fn new(
        py: Python<'_>,
        spec: &Bound<'_, PyAny>,
        lowered_module: BlockPyModule<CodegenBlockPyPass>,
    ) -> PyResult<Py<PyAny>> {
        let module_name = spec
            .getattr("name")?
            .extract::<String>()
            .map_err(|_| PyTypeError::new_err("expected a module spec with a string 'name'"))?;
        let package_name = spec
            .getattr("parent")?
            .extract::<String>()
            .map_err(|_| PyTypeError::new_err("expected a module spec with a string 'parent'"))?;
        let module = unsafe {
            Bound::from_owned_ptr_or_err(
                py,
                ffi::PyModule_FromDefAndSpec(soac_ext_module_def(), spec.as_ptr()),
            )?
        };
        if unsafe { ffi::PyModule_ExecDef(module.as_ptr(), soac_ext_module_def()) } != 0 {
            return Err(PyErr::fetch(py));
        }
        let state = soac_ext_module_state(&module)?;
        unsafe {
            if !(*state).data.is_null() {
                return Err(PyRuntimeError::new_err(
                    "transformed module state was unexpectedly initialized twice",
                ));
            }
            (*state).data = Box::into_raw(Box::new(SoacExtModuleData {
                lowered_module,
                module_name,
                package_name,
            }));
        }
        Ok(module.unbind())
    }

    pub fn with_data<R>(
        module: &Bound<'_, PyAny>,
        f: impl FnOnce(&SoacExtModuleData) -> PyResult<R>,
    ) -> PyResult<R> {
        let state = soac_ext_module_state(module)?;
        unsafe {
            let data = (*state).data;
            if data.is_null() {
                return Err(PyRuntimeError::new_err(
                    "missing transformed-module lowering data in module state",
                ));
            }
            f(&*data)
        }
    }
}

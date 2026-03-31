use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;
use soac_blockpy::block_py::BlockPyModule;
use soac_blockpy::passes::CodegenBlockPyPass;
use std::ffi::{c_int, c_void};
use std::mem::MaybeUninit;
use std::ptr;

pub struct SoacExtModuleDataRef<'a> {
    pub lowered_module: &'a BlockPyModule<CodegenBlockPyPass>,
    pub module_name: &'a str,
    pub package_name: &'a str,
}

#[repr(C)]
struct SoacExtModuleState {
    initialized: bool,
    lowered_module: MaybeUninit<BlockPyModule<CodegenBlockPyPass>>,
    module_name: MaybeUninit<String>,
    package_name: MaybeUninit<String>,
}

impl SoacExtModuleState {
    unsafe fn init(
        &mut self,
        lowered_module: BlockPyModule<CodegenBlockPyPass>,
        module_name: String,
        package_name: String,
    ) -> PyResult<()> {
        if self.initialized {
            return Err(PyRuntimeError::new_err(
                "transformed module state was unexpectedly initialized twice",
            ));
        }
        self.lowered_module.write(lowered_module);
        self.module_name.write(module_name);
        self.package_name.write(package_name);
        self.initialized = true;
        Ok(())
    }

    unsafe fn clear(&mut self) {
        if !self.initialized {
            return;
        }
        unsafe {
            ptr::drop_in_place(self.lowered_module.as_mut_ptr());
            ptr::drop_in_place(self.module_name.as_mut_ptr());
            ptr::drop_in_place(self.package_name.as_mut_ptr());
        }
        self.initialized = false;
    }

    unsafe fn data(&self) -> PyResult<SoacExtModuleDataRef<'_>> {
        if !self.initialized {
            return Err(PyRuntimeError::new_err(
                "missing transformed-module lowering data in module state",
            ));
        }
        Ok(SoacExtModuleDataRef {
            lowered_module: unsafe { self.lowered_module.assume_init_ref() },
            module_name: unsafe { self.module_name.assume_init_ref().as_str() },
            package_name: unsafe { self.package_name.assume_init_ref().as_str() },
        })
    }
}

unsafe extern "C" fn soac_ext_module_clear(module: *mut ffi::PyObject) -> c_int {
    let state = unsafe { ffi::PyModule_GetState(module) }.cast::<SoacExtModuleState>();
    if state.is_null() {
        return 0;
    }
    unsafe { (*state).clear() };
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
    // This state stores only Rust-owned lowering data and strings, not PyObject refs.
    m_traverse: None,
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
            (*state).init(lowered_module, module_name, package_name)?;
        }
        Ok(module.unbind())
    }

    pub fn with_data<R>(
        module: &Bound<'_, PyAny>,
        f: impl FnOnce(SoacExtModuleDataRef<'_>) -> PyResult<R>,
    ) -> PyResult<R> {
        let state = soac_ext_module_state(module)?;
        unsafe { f((*state).data()?) }
    }
}

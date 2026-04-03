use crate::module_constants::ModuleCodegenConstants;
use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;
use soac_blockpy::block_py::{BlockPyFunction, BlockPyModule, FunctionId};
use soac_blockpy::passes::CodegenBlockPyPass;
use std::collections::HashMap;
use std::ffi::{c_int, c_void};
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::Arc;

pub struct SoacExtModuleDataRef<'a> {
    pub shared_state: &'a SharedModuleState,
}

pub struct SharedModuleState {
    pub lowered_module: BlockPyModule<CodegenBlockPyPass>,
    pub module_name: String,
    pub package_name: String,
    pub codegen_constants: ModuleCodegenConstants,
    function_index_by_id: HashMap<FunctionId, usize>,
    module_constant_objs: Vec<Py<PyAny>>,
}

impl SharedModuleState {
    pub fn lookup_function(
        &self,
        function_id: FunctionId,
    ) -> Option<&BlockPyFunction<CodegenBlockPyPass>> {
        let function_index = self.function_index_by_id.get(&function_id).copied()?;
        let function = self.lowered_module.callable_defs.get(function_index)?;
        assert_eq!(function.function_id, function_id);
        Some(function)
    }

    pub(crate) fn module_constant_ptrs(&self) -> Vec<*mut ffi::PyObject> {
        self.module_constant_objs.iter().map(|obj| obj.as_ptr()).collect()
    }
}

fn build_function_index_by_id(
    module: &BlockPyModule<CodegenBlockPyPass>,
) -> PyResult<HashMap<FunctionId, usize>> {
    let mut function_index_by_id = HashMap::with_capacity(module.callable_defs.len());
    for (function_index, function) in module.callable_defs.iter().enumerate() {
        if function_index_by_id
            .insert(function.function_id, function_index)
            .is_some()
        {
            return Err(PyRuntimeError::new_err(format!(
                "duplicate function id {} in shared module state ({})",
                function.function_id.0, function.names.qualname
            )));
        }
    }
    Ok(function_index_by_id)
}

#[repr(C)]
struct SoacExtModuleState {
    initialized: bool,
    shared_state: MaybeUninit<Arc<SharedModuleState>>,
}

impl SoacExtModuleState {
    unsafe fn init(
        &mut self,
        py: Python<'_>,
        lowered_module: BlockPyModule<CodegenBlockPyPass>,
        module_name: String,
        package_name: String,
    ) -> PyResult<()> {
        if self.initialized {
            return Err(PyRuntimeError::new_err(
                "transformed module state was unexpectedly initialized twice",
            ));
        }
        let function_index_by_id = build_function_index_by_id(&lowered_module)?;
        let codegen_constants = ModuleCodegenConstants::collect_from_module(&lowered_module);
        let module_constant_objs = codegen_constants.build_python_constants(py)?;
        self.shared_state.write(Arc::new(SharedModuleState {
            lowered_module,
            module_name,
            package_name,
            codegen_constants,
            function_index_by_id,
            module_constant_objs,
        }));
        self.initialized = true;
        Ok(())
    }

    unsafe fn clear(&mut self) {
        if !self.initialized {
            return;
        }
        unsafe { ptr::drop_in_place(self.shared_state.as_mut_ptr()) };
        self.initialized = false;
    }

    unsafe fn data(&self) -> PyResult<SoacExtModuleDataRef<'_>> {
        if !self.initialized {
            return Err(PyRuntimeError::new_err(
                "missing transformed-module lowering data in module state",
            ));
        }
        Ok(SoacExtModuleDataRef {
            shared_state: unsafe { self.shared_state.assume_init_ref().as_ref() },
        })
    }

    unsafe fn clone_shared_state(&self) -> PyResult<Arc<SharedModuleState>> {
        if !self.initialized {
            return Err(PyRuntimeError::new_err(
                "missing transformed-module lowering data in module state",
            ));
        }
        Ok(unsafe { self.shared_state.assume_init_ref().clone() })
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

unsafe extern "C" fn soac_ext_module_traverse(
    module: *mut ffi::PyObject,
    visit: ffi::visitproc,
    arg: *mut c_void,
) -> c_int {
    let state = unsafe { ffi::PyModule_GetState(module) }.cast::<SoacExtModuleState>();
    if state.is_null() || unsafe { !(*state).initialized } {
        return 0;
    }
    let shared_state = unsafe { (*state).shared_state.assume_init_ref().as_ref() };
    for obj in &shared_state.module_constant_objs {
        let rc = unsafe { visit(obj.as_ptr(), arg) };
        if rc != 0 {
            return rc;
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
            (*state).init(py, lowered_module, module_name, package_name)?;
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

    pub fn clone_shared_state(module: &Bound<'_, PyAny>) -> PyResult<Arc<SharedModuleState>> {
        let state = soac_ext_module_state(module)?;
        unsafe { (*state).clone_shared_state() }
    }
}

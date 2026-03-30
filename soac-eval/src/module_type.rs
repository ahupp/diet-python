use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyAnyMethods, PyDict, PyModule, PyTuple, PyType};
use soac_blockpy::block_py::BlockPyModule;
use soac_blockpy::passes::CodegenBlockPyPass;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static SOAC_EXT_MODULE_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();
// These module objects are instances of a dynamic Python ModuleType subclass,
// not a #[pyclass] with embedded Rust storage, so the lowered BlockPyModule
// lives in a side table keyed by module object identity.
static LOWERED_MODULE_REGISTRY: OnceLock<Mutex<HashMap<usize, BlockPyModule<CodegenBlockPyPass>>>> =
    OnceLock::new();

fn lowered_module_registry() -> &'static Mutex<HashMap<usize, BlockPyModule<CodegenBlockPyPass>>> {
    LOWERED_MODULE_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn module_registry_key(module: &Bound<'_, PyAny>) -> usize {
    module.as_ptr() as usize
}

fn soac_ext_module_type<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyType>> {
    let module_type = SOAC_EXT_MODULE_TYPE.get_or_try_init(py, || -> PyResult<Py<PyType>> {
        let builtins = PyModule::import(py, "builtins")?;
        let type_fn = builtins.getattr("type")?;
        let types = PyModule::import(py, "types")?;
        let module_type = types.getattr("ModuleType")?;
        let bases = PyTuple::new(py, [module_type])?;
        let namespace = PyDict::new(py);
        namespace.set_item("__module__", "_soac_ext")?;
        namespace.set_item("__qualname__", "SoacExtModule")?;
        let cls = type_fn.call1(("SoacExtModule", bases, namespace))?;
        let cls = cls.cast_into::<PyType>()?;
        Ok(cls.unbind())
    })?;
    Ok(module_type.bind(py).clone())
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
        let module = soac_ext_module_type(py)?.call1((module_name,))?;
        module.setattr("__spec__", spec)?;
        module.setattr("__package__", spec.getattr("parent")?)?;
        module.setattr("__loader__", spec.getattr("loader")?)?;
        let origin = spec.getattr("origin")?;
        if !origin.is_none() {
            module.setattr("__file__", origin)?;
        }
        let submodule_search_locations = spec.getattr("submodule_search_locations")?;
        if !submodule_search_locations.is_none() {
            module.setattr("__path__", submodule_search_locations)?;
        }
        lowered_module_registry()
            .lock()
            .map_err(|_| PyRuntimeError::new_err("failed to lock lowered module registry"))?
            .insert(module_registry_key(&module), lowered_module);
        Ok(module.unbind())
    }

    pub fn lowered_module(
        module: &Bound<'_, PyAny>,
    ) -> PyResult<BlockPyModule<CodegenBlockPyPass>> {
        let module_type = soac_ext_module_type(module.py())?;
        if !module.is_instance(module_type.as_any())? {
            return Err(PyTypeError::new_err(
                "expected an _soac_ext-created module instance",
            ));
        }
        lowered_module_registry()
            .lock()
            .map_err(|_| PyRuntimeError::new_err("failed to lock lowered module registry"))?
            .get(&module_registry_key(module))
            .cloned()
            .ok_or_else(|| {
                PyRuntimeError::new_err("missing lowered module for custom module instance")
            })
    }
}

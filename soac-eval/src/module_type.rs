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
static LOWERED_MODULE_REGISTRY: OnceLock<Mutex<HashMap<usize, SoacExtModuleData>>> =
    OnceLock::new();

fn lowered_module_registry() -> &'static Mutex<HashMap<usize, SoacExtModuleData>> {
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

#[derive(Clone)]
pub struct SoacExtModuleData {
    pub lowered_module: BlockPyModule<CodegenBlockPyPass>,
    pub module_name: String,
    pub package_name: String,
}

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
        let module = soac_ext_module_type(py)?.call1((module_name.as_str(),))?;
        lowered_module_registry()
            .lock()
            .map_err(|_| PyRuntimeError::new_err("failed to lock lowered module registry"))?
            .insert(
                module_registry_key(&module),
                SoacExtModuleData {
                    lowered_module,
                    module_name,
                    package_name,
                },
            );
        Ok(module.unbind())
    }

    pub fn data(module: &Bound<'_, PyAny>) -> PyResult<SoacExtModuleData> {
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

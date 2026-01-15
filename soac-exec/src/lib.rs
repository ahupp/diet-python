use pyo3::exceptions::{PyAttributeError, PyImportError, PyRuntimeError, PySyntaxError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};
use pyo3::PyErr;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex, Once};

pub mod c_api;
pub mod cranelift;
pub mod enum_evolution;
pub mod evaluate;
pub mod module_instance;
pub mod module_symbols;
pub mod oython_ffi;
pub mod scope;

use crate::module_instance::ModuleInstance;
use crate::scope::ScopeStack;

#[repr(C)]
struct StrictModule {
    ob_base: ffi::PyObject,
    inst: *mut Arc<ModuleInstance>,
    executed: bool,
}

unsafe extern "C" fn strict_module_dealloc(obj: *mut ffi::PyObject) {
    let module = obj as *mut StrictModule;
    let inst_ptr = (*module).inst;
    if !inst_ptr.is_null() {
        drop(Box::from_raw(inst_ptr));
    }
    ffi::PyObject_Free(obj as *mut std::ffi::c_void);
}

unsafe extern "C" fn strict_module_getattro(
    obj: *mut ffi::PyObject,
    name: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    Python::with_gil(|py| {
        let name_obj = py.from_borrowed_ptr::<PyAny>(name);
        let Ok(name_str) = name_obj.extract::<&str>() else {
            PyAttributeError::new_err("attribute name must be str").restore(py);
            return std::ptr::null_mut();
        };
        let module = obj as *mut StrictModule;
        let inst_arc = &*(*module).inst;
        let inst = unsafe { &*Arc::as_ptr(inst_arc) };
        if let Some(idx) = inst.globals.index_of(name_str) {
            match inst.globals.get_by_index(idx) {
                Some(ptr) => {
                    ffi::Py_XINCREF(ptr);
                    ptr
                }
                None => {
                    let none = py.None();
                    unsafe { ffi::Py_INCREF(none.as_ptr()) };
                    none.as_ptr()
                }
            }
        } else {
            PyAttributeError::new_err(name_str.to_string()).restore(py);
            std::ptr::null_mut()
        }
    })
}

unsafe extern "C" fn strict_module_setattro(
    obj: *mut ffi::PyObject,
    name: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> std::os::raw::c_int {
    Python::with_gil(|py| {
        let name_obj = py.from_borrowed_ptr::<PyAny>(name);
        let Ok(name_str) = name_obj.extract::<&str>() else {
            PyAttributeError::new_err("attribute name must be str").restore(py);
            return -1;
        };
        let module = obj as *mut StrictModule;
        if unsafe { (*module).executed } {
            PyAttributeError::new_err("cannot assign to strict module").restore(py);
            return -1;
        }
        let inst_ptr = unsafe { (*module).inst };
        let inst_arc = unsafe { &*inst_ptr };
        let inst = unsafe { &mut *(Arc::as_ptr(inst_arc) as *mut ModuleInstance) };
        if let Some(idx) = inst.globals.index_of(name_str) {
            inst.globals.set_by_index(idx, Some(value));
            0
        } else {
            PyAttributeError::new_err(name_str.to_string()).restore(py);
            -1
        }
    })
}

#[allow(clippy::uninit_assumed_init)]
static mut STRICT_MODULE_TYPE: ffi::PyTypeObject = ffi::PyTypeObject {
    ob_base: ffi::PyVarObject {
        ob_base: ffi::PyObject_HEAD_INIT,
        ob_size: 0,
    },
    tp_name: b"soac_exec.StrictModule\0".as_ptr() as *const _,
    tp_basicsize: std::mem::size_of::<StrictModule>() as isize,
    tp_itemsize: 0,
    tp_dealloc: Some(strict_module_dealloc),
    tp_getattro: Some(strict_module_getattro),
    tp_setattro: Some(strict_module_setattro),
    tp_flags: ffi::Py_TPFLAGS_DEFAULT,
    tp_dictoffset: 0,
    tp_doc: std::ptr::null(),
    tp_traverse: None,
    tp_clear: None,
    tp_free: None,
    ..unsafe { std::mem::zeroed() }
};

unsafe fn init_strict_type() -> PyResult<()> {
    if (STRICT_MODULE_TYPE.tp_flags & ffi::Py_TPFLAGS_READY) != 0 {
        return Ok(());
    }
    if ffi::PyType_Ready(std::ptr::addr_of_mut!(STRICT_MODULE_TYPE)) < 0 {
        return Err(PyErr::fetch(Python::assume_gil_acquired()));
    }
    Ok(())
}

#[allow(clippy::uninit_assumed_init)]
static mut MODULE_SLOTS: [ffi::PyModuleDef_Slot; 2] = [
    ffi::PyModuleDef_Slot {
        slot: ffi::Py_mod_create,
        value: module_create as *mut _,
    },
    ffi::PyModuleDef_Slot {
        slot: 0,
        value: std::ptr::null_mut(),
    },
];

unsafe extern "C" fn module_create(
    _spec: *mut ffi::PyObject,
    _def: *mut ffi::PyModuleDef,
) -> *mut ffi::PyObject {
    if init_strict_type().is_err() {
        return std::ptr::null_mut();
    }
    let m = ffi::_PyObject_New(std::ptr::addr_of_mut!(STRICT_MODULE_TYPE)) as *mut StrictModule;
    if m.is_null() {
        return std::ptr::null_mut();
    }
    (*m).inst = std::ptr::null_mut();
    (*m).executed = false;
    m as *mut ffi::PyObject
}

#[allow(clippy::uninit_assumed_init)]
static mut MODULE_DEF: ffi::PyModuleDef = ffi::PyModuleDef {
    m_base: ffi::PyModuleDef_HEAD_INIT,
    m_name: b"soac_exec.module\0".as_ptr() as *const _,
    m_doc: std::ptr::null(),
    m_size: 0,
    m_methods: std::ptr::null_mut(),
    m_slots: std::ptr::addr_of_mut!(MODULE_SLOTS) as *mut ffi::PyModuleDef_Slot,
    m_traverse: None,
    m_clear: None,
    m_free: None,
};

static INIT_MODULE_DEF: Once = Once::new();

fn init_module_def() {
    unsafe {
        INIT_MODULE_DEF.call_once(|| {
            ffi::PyModuleDef_Init(std::ptr::addr_of_mut!(MODULE_DEF));
        });
    }
}

#[pyclass]
struct CraneLoaderExt {
    finder: PyObject,
    modules: Mutex<HashMap<String, Arc<ModuleInstance>>>,
}

#[pymethods]
impl CraneLoaderExt {
    #[new]
    fn new(_py: Python<'_>, finder: PyObject) -> PyResult<Self> {
        Ok(Self {
            finder,
            modules: Mutex::new(HashMap::new()),
        })
    }

    fn is_strict_module(&self, path: &str) -> PyResult<bool> {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return Ok(false),
        };
        let mut reader = BufReader::new(file);
        let mut first_line = String::new();
        if reader.read_line(&mut first_line).is_ok() {
            Ok(first_line.trim() == "# __strict___")
        } else {
            Ok(false)
        }
    }

    fn create_module(&self, py: Python<'_>, spec: &PyAny) -> PyResult<PyObject> {
        let fullname: String = spec.getattr("name")?.extract()?;
        let path: String = spec.getattr("origin")?.extract()?;
        let instance = {
            let mut modules = self.modules.lock().unwrap();
            if let Some(inst) = modules.get(&fullname) {
                Arc::clone(inst)
            } else {
                let source = fs::read_to_string(&path).map_err(|err| {
                    PyRuntimeError::new_err(format!("failed to read {path}: {err}"))
                })?;
                let ast = match std::panic::catch_unwind(|| {
                    diet_python::transform_min_ast(&source, None)
                }) {
                    Ok(Ok(ast)) => ast,
                    Ok(Err(err)) => return Err(PySyntaxError::new_err(err.to_string())),
                    Err(payload) => {
                        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                            (*s).to_string()
                        } else if let Some(s) = payload.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "panic during AST transform".to_string()
                        };
                        return Err(PyRuntimeError::new_err(msg));
                    }
                };
                let inst = Arc::new(module_instance::ModuleInstance::new(ast));
                modules.insert(fullname.clone(), Arc::clone(&inst));
                inst
            }
        };

        init_module_def();
        unsafe {
            let module = ffi::PyModule_FromDefAndSpec2(
                std::ptr::addr_of_mut!(MODULE_DEF),
                spec.as_ptr(),
                ffi::PYTHON_API_VERSION as i32,
            );
            if module.is_null() {
                return Err(PyErr::fetch(py));
            }
            let module_obj = module as *mut StrictModule;
            (*module_obj).inst = Box::into_raw(Box::new(instance));
            Ok(PyObject::from_owned_ptr(py, module))
        }
    }

    fn exec_module(&self, py: Python<'_>, module: &PyAny) -> PyResult<()> {
        unsafe {
            let m = module.as_ptr() as *mut StrictModule;
            let inst_ptr = (*m).inst;
            let inst_arc = &*inst_ptr;
            let inst = &mut *(Arc::as_ptr(inst_arc) as *mut ModuleInstance);
            let builtins = py.import("builtins")?;
            let dp: PyObject = PyModule::from_code(
                py,
                diet_python::intrinsics::DP_SOURCE,
                "__dp__.py",
                "__dp__",
            )?
            .into();
            let getattr: PyObject = builtins.getattr("getattr")?.into();
            let globals = std::mem::take(&mut inst.globals);
            let mut stack = ScopeStack::new(builtins.into(), globals);
            if let Some(idx) = stack.index_of("__dp__") {
                stack.set_by_index(idx, Some(dp.as_ptr()));
            }
            if let Some(idx) = stack.index_of("getattr") {
                stack.set_by_index(idx, Some(getattr.as_ptr()));
            }
            match crate::evaluate::evaluate_module(py, &mut stack, &inst.ast.body) {
                Ok(()) => {
                    inst.globals = stack.into_globals();
                    (*m).executed = true;
                    Ok(())
                }
                Err(err) => {
                    inst.globals = stack.into_globals();
                    let import_err = PyErr::new::<PyImportError, _>("module execution failed");
                    import_err.set_cause(py, Some(err));
                    Err(import_err)
                }
            }
        }
    }
}

#[pymodule]
fn soac_exec(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<CraneLoaderExt>()?;
    Ok(())
}

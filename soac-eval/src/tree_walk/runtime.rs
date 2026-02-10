use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

pub struct RuntimeFns {
    pub(crate) builtins_globals: Py<PyAny>,
    pub(crate) builtins_locals: Py<PyAny>,
    pub(crate) dp_globals: Py<PyAny>,
    pub(crate) dp_locals: Py<PyAny>,
}

impl RuntimeFns {
    pub fn new(
        builtins: &Bound<'_, PyDict>,
        dp_module: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let builtins_globals = builtins
            .get_item("globals")?
            .ok_or_else(|| PyRuntimeError::new_err("missing builtins globals"))?
            .unbind();
        let builtins_locals = builtins
            .get_item("locals")?
            .ok_or_else(|| PyRuntimeError::new_err("missing builtins locals"))?
            .unbind();
        let dp_globals = dp_module.getattr("globals")?.unbind();
        let dp_locals = dp_module.getattr("locals")?.unbind();
        Ok(Self {
            builtins_globals,
            builtins_locals,
            dp_globals,
            dp_locals,
        })
    }
}

impl Clone for RuntimeFns {
    fn clone(&self) -> Self {
        // RuntimeFns is only cloned while executing Python code with the GIL held.
        let py = unsafe { Python::assume_attached() };
        Self {
            builtins_globals: unsafe {
                Bound::from_borrowed_ptr(py, self.builtins_globals.as_ptr()).unbind()
            },
            builtins_locals: unsafe {
                Bound::from_borrowed_ptr(py, self.builtins_locals.as_ptr()).unbind()
            },
            dp_globals: unsafe { Bound::from_borrowed_ptr(py, self.dp_globals.as_ptr()).unbind() },
            dp_locals: unsafe { Bound::from_borrowed_ptr(py, self.dp_locals.as_ptr()).unbind() },
        }
    }
}

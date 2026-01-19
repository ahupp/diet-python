use dp_transform::transform_to_string_without_attribute_lowering_cpython;
use pyo3::exceptions::{PyRuntimeError, PySyntaxError};
use pyo3::prelude::*;
use std::fs;

mod eval;

#[pyfunction]
fn transform_source(source: &str, ensure: Option<bool>) -> PyResult<String> {
    let ensure = ensure.unwrap_or(true);
    match transform_to_string_without_attribute_lowering_cpython(source, ensure) {
        Ok(output) => Ok(output),
        Err(err) => Err(pyo3::exceptions::PySyntaxError::new_err(err.to_string())),
    }
}

#[pyfunction]
fn eval_source(py: Python<'_>, path: &str) -> PyResult<Py<PyAny>> {
 
    let source = fs::read_to_string(path)
        .map_err(|err| PyRuntimeError::new_err(format!("failed to read {path}: {err}")))?;
    let result = match std::panic::catch_unwind(|| eval::eval_source_impl(py, path, &source)) {
        Ok(result) => result,
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
    result.map_err(PySyntaxError::new_err)
}

#[pymodule]
fn diet_python(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    dp_transform::init_logging();
    module.add_function(wrap_pyfunction!(transform_source, module)?)?;
    module.add_function(wrap_pyfunction!(eval_source, module)?)?;
    Ok(())
}

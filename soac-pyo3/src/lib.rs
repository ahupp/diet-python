#![allow(unsafe_op_in_unsafe_fn)]

use dp_transform::{Options, transform_str_to_ruff_with_options};
use log::trace;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::fs;

mod eval;

#[pyfunction]
fn transform_source(source: &str, ensure: Option<bool>) -> PyResult<String> {
    let preview = source.get(..100).unwrap_or(source);
    trace!("transform_source: {}", preview);
    let options = Options {
        inject_import: ensure.unwrap_or(true),
        lower_attributes: false,
        ..Options::default()
    };
    match transform_str_to_ruff_with_options(source, options) {
        Ok(output) => {
            if std::env::var_os("DIET_PYTHON_VALIDATE_MIN_AST").as_deref() == Some("1".as_ref())
                && std::env::var_os("DIET_PYTHON_MODE").as_deref() == Some("eval".as_ref())
            {
                eval::transform_to_min_ast(source)
                    .map_err(eval::TransformToMinAstError::to_py_err)?;
            }
            Ok(output.to_string())
        }
        Err(err) => Err(pyo3::exceptions::PySyntaxError::new_err(err.to_string())),
    }
}

fn eval_source_common(
    py: Python<'_>,
    path: &str,
    name: Option<&str>,
    package: Option<&str>,
) -> PyResult<Py<PyAny>> {
    trace!("eval_source: {}", path);

    let source = fs::read_to_string(path)
        .map_err(|err| PyRuntimeError::new_err(format!("failed to read {path}: {err}")))?;
    match std::panic::catch_unwind(|| match name {
        Some(name) => eval::eval_source_impl_with_name(py, path, &source, name, package),
        None => eval::eval_source_impl(py, path, &source),
    }) {
        Ok(result) => result,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic during eval source".to_string()
            };
            Err(PyRuntimeError::new_err(msg))
        }
    }
}

#[pyfunction]
fn eval_source(py: Python<'_>, path: &str) -> PyResult<Py<PyAny>> {
    eval_source_common(py, path, None, None)
}

#[pyfunction]
fn eval_source_with_name(
    py: Python<'_>,
    path: &str,
    name: &str,
    package: Option<&str>,
) -> PyResult<Py<PyAny>> {
    eval_source_common(py, path, Some(name), package)
}

#[pyfunction]
fn eval_source_with_spec(
    py: Python<'_>,
    path: &str,
    name: &str,
    package: Option<&str>,
    spec: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    trace!("eval_source: {}", path);

    let source = fs::read_to_string(path)
        .map_err(|err| PyRuntimeError::new_err(format!("failed to read {path}: {err}")))?;
    let spec_ptr = spec.as_ptr();
    match std::panic::catch_unwind(|| {
        eval::eval_source_impl_with_spec(py, path, &source, name, package, spec_ptr)
    }) {
        Ok(result) => result,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "panic during eval source".to_string()
            };
            Err(PyRuntimeError::new_err(msg))
        }
    }
}

#[pymodule]
fn diet_python(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    dp_transform::init_logging();
    module.add_function(wrap_pyfunction!(transform_source, module)?)?;
    module.add_function(wrap_pyfunction!(eval_source, module)?)?;
    module.add_function(wrap_pyfunction!(eval_source_with_name, module)?)?;
    module.add_function(wrap_pyfunction!(eval_source_with_spec, module)?)?;
    Ok(())
}

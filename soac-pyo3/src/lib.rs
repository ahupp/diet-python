#![allow(unsafe_op_in_unsafe_fn)]

use dp_transform::{Options, transform_str_to_ruff_with_options};
use log::trace;
use pyo3::exceptions::PyRuntimeError;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyDict;
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
    let spec_obj = spec.clone().unbind();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eval::eval_source_impl_with_spec(py, path, &source, name, package, spec_obj)
    })) {
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
fn jit_run_bb_plan(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
    globals_obj: &Bound<'_, PyAny>,
    args: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    eval::jit_run_bb_plan_impl(py, module_name, qualname, globals_obj, args)
}

#[pyfunction]
fn jit_has_bb_plan(module_name: &str, qualname: &str) -> bool {
    eval::jit_has_bb_plan_impl(module_name, qualname)
}

#[pyfunction]
fn jit_render_bb_plan(py: Python<'_>, module_name: &str, qualname: &str) -> PyResult<String> {
    eval::jit_render_bb_plan_impl(py, module_name, qualname)
}

#[pyfunction]
fn jit_render_bb_with_cfg_plan(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
) -> PyResult<Py<PyDict>> {
    let (clif, cfg_dot) = eval::jit_render_bb_with_cfg_plan_impl(py, module_name, qualname)?;
    let payload = PyDict::new(py);
    payload.set_item("clif", clif)?;
    payload.set_item("cfg_dot", cfg_dot)?;
    Ok(payload.unbind())
}

#[pyfunction]
fn register_clif_wrapper(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<()> {
    unsafe {
        soac_eval::tree_walk::register_clif_wrapper_code_extra(func.as_ptr()).map_err(|_| {
            if ffi::PyErr_Occurred().is_null() {
                PyRuntimeError::new_err("failed to register CLIF wrapper code extra")
            } else {
                PyErr::fetch(py)
            }
        })
    }
}

#[pymodule]
fn diet_python(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    dp_transform::init_logging();
    module.add_function(wrap_pyfunction!(transform_source, module)?)?;
    module.add_function(wrap_pyfunction!(eval_source, module)?)?;
    module.add_function(wrap_pyfunction!(eval_source_with_name, module)?)?;
    module.add_function(wrap_pyfunction!(eval_source_with_spec, module)?)?;
    module.add_function(wrap_pyfunction!(jit_run_bb_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_has_bb_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_render_bb_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_render_bb_with_cfg_plan, module)?)?;
    module.add_function(wrap_pyfunction!(register_clif_wrapper, module)?)?;
    Ok(())
}

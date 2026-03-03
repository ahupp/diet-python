#![allow(unsafe_op_in_unsafe_fn)]

use dp_transform::basic_block::normalize_bb_module_for_codegen;
use dp_transform::{Options, transform_str_to_ruff_with_options};
use log::trace;
use pyo3::exceptions::PyRuntimeError;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyDict;

mod eval;

fn lower_source(source: &str, ensure: Option<bool>) -> PyResult<dp_transform::LoweringResult> {
    let options = Options {
        inject_import: ensure.unwrap_or(true),
        lower_attributes: false,
        ..Options::default()
    };
    transform_str_to_ruff_with_options(source, options)
        .map_err(|err| pyo3::exceptions::PySyntaxError::new_err(err.to_string()))
}

#[pyfunction]
fn transform_source(source: &str, ensure: Option<bool>) -> PyResult<String> {
    let preview = source.get(..100).unwrap_or(source);
    trace!("transform_source: {}", preview);
    Ok(lower_source(source, ensure)?.to_string())
}

#[pyfunction]
fn transform_source_with_name(
    source: &str,
    module_name: &str,
    ensure: Option<bool>,
) -> PyResult<String> {
    let preview = source.get(..100).unwrap_or(source);
    trace!("transform_source_with_name({module_name}): {}", preview);
    let output = lower_source(source, ensure)?;
    if let Some(bb_module) = output.bb_module.as_ref() {
        let normalized = normalize_bb_module_for_codegen(bb_module);
        soac_eval::jit::register_bb_module_plans(module_name, &normalized).map_err(|err| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "failed to register BB plans for {module_name}: {err}"
            ))
        })?;
    }
    Ok(output.to_string())
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
fn jit_block_param_names(
    module_name: &str,
    qualname: &str,
    entry_label: &str,
) -> PyResult<Vec<String>> {
    eval::jit_block_param_names_impl(module_name, qualname, entry_label)
}

#[pyfunction]
fn jit_debug_plan(module_name: &str, qualname: &str) -> PyResult<String> {
    eval::jit_debug_plan_impl(module_name, qualname)
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
    module.add_function(wrap_pyfunction!(transform_source_with_name, module)?)?;
    module.add_function(wrap_pyfunction!(jit_run_bb_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_has_bb_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_block_param_names, module)?)?;
    module.add_function(wrap_pyfunction!(jit_debug_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_render_bb_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_render_bb_with_cfg_plan, module)?)?;
    module.add_function(wrap_pyfunction!(register_clif_wrapper, module)?)?;
    Ok(())
}

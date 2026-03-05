#![allow(unsafe_op_in_unsafe_fn)]

use dp_transform::basic_block::normalize_bb_module_for_codegen;
use dp_transform::{Options, transform_str_to_ruff_with_options};
use log::{info, trace};
use pyo3::exceptions::PyRuntimeError;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::time::Instant;

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
        soac_eval::jit::register_clif_module_plans(module_name, &normalized).map_err(|err| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "failed to register BB plans for {module_name}: {err}"
            ))
        })?;
        // Modules executed via `python -m pkg` are transformed under
        // loader fullname `pkg.__main__` but run with `__name__ == "__main__"`.
        // BB function wrappers pass `__name__` into __dp_def_fn/__dp_def_coro,
        // so register an alias under "__main__" to keep plan lookup consistent.
        if module_name.ends_with(".__main__") && module_name != "__main__" {
            soac_eval::jit::register_clif_module_plans("__main__", &normalized).map_err(|err| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "failed to register BB plans alias for __main__ from {module_name}: {err}"
                ))
            })?;
        }
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
fn register_clif_wrapper(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    module_name: &str,
    qualname: &str,
    sig_obj: &Bound<'_, PyAny>,
    state_order_obj: &Bound<'_, PyAny>,
    closure_obj: &Bound<'_, PyAny>,
    build_entry_args_obj: &Bound<'_, PyAny>,
) -> PyResult<()> {
    unsafe {
        soac_eval::tree_walk::register_clif_wrapper_code_extra(
            func.as_ptr(),
            module_name,
            qualname,
            sig_obj.as_ptr(),
            state_order_obj.as_ptr(),
            closure_obj.as_ptr(),
            build_entry_args_obj.as_ptr(),
        )
        .map_err(|_| {
            if ffi::PyErr_Occurred().is_null() {
                PyRuntimeError::new_err("failed to register CLIF wrapper code extra")
            } else {
                PyErr::fetch(py)
            }
        })
    }
}

#[pyfunction]
fn jit_compile_clif_wrapper(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<()> {
    let module_name = func
        .getattr("__module__")
        .ok()
        .and_then(|value| value.extract::<String>().ok())
        .unwrap_or_else(|| "<unknown-module>".to_string());
    let qualname = func
        .getattr("__qualname__")
        .ok()
        .and_then(|value| value.extract::<String>().ok())
        .unwrap_or_else(|| "<unknown-qualname>".to_string());
    let start = Instant::now();
    unsafe {
        soac_eval::tree_walk::compile_clif_wrapper_code_extra(func.as_ptr()).map_err(|_| {
            if ffi::PyErr_Occurred().is_null() {
                PyRuntimeError::new_err("failed to eagerly compile CLIF wrapper")
            } else {
                PyErr::fetch(py)
            }
        })?;
    }
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    info!(
        "soac_jit_eager_compile module={} qualname={} elapsed_ms={elapsed_ms:.3}",
        module_name, qualname
    );
    Ok(())
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
    module.add_function(wrap_pyfunction!(jit_compile_clif_wrapper, module)?)?;
    Ok(())
}

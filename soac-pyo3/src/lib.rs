#![allow(unsafe_op_in_unsafe_fn)]

use dp_transform::{Options, transform_str_to_ruff_with_options};
use log::{info, trace};
use pyo3::exceptions::PyRuntimeError;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use std::time::Instant;

mod eval;

fn lower_source(source: &str, ensure: Option<bool>) -> PyResult<dp_transform::LoweringResult> {
    let _ = ensure;
    let options = Options {
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
        let normalized = dp_transform::basic_block::prepare_bb_module_for_codegen(bb_module);
        soac_eval::jit::register_clif_module_plans(module_name, &normalized).map_err(|err| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "failed to register BB plans for {module_name}: {err}"
            ))
        })?;
        // Modules executed via `python -m pkg` are transformed under
        // loader fullname `pkg.__main__` but run with `__name__ == "__main__"`.
        // BB function wrappers pass `__name__` into __dp_make_function/__dp_def_coro,
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
fn jit_has_bb_plan(module_name: &str, function_id: usize) -> bool {
    eval::jit_has_bb_plan_impl(module_name, function_id)
}

#[pyfunction]
fn jit_block_param_names(
    module_name: &str,
    function_id: usize,
    entry_label: &str,
) -> PyResult<Vec<String>> {
    eval::jit_block_param_names_impl(module_name, function_id, entry_label)
}

#[pyfunction]
fn jit_debug_plan(module_name: &str, function_id: usize) -> PyResult<String> {
    eval::jit_debug_plan_impl(module_name, function_id)
}

#[pyfunction]
fn jit_render_bb_with_cfg_plan(
    py: Python<'_>,
    module_name: &str,
    function_id: usize,
) -> PyResult<Py<PyDict>> {
    let (clif, cfg_dot, vcode_disasm) =
        eval::jit_render_bb_with_cfg_plan_impl(py, module_name, function_id)?;
    let payload = PyDict::new(py);
    payload.set_item("clif", clif)?;
    payload.set_item("cfg_dot", cfg_dot)?;
    payload.set_item("vcode_disasm", vcode_disasm)?;
    Ok(payload.unbind())
}

fn register_clif_vectorcall_impl(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    module_name: &str,
    function_id: usize,
    metadata: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    if metadata.len() != 8 {
        return Err(PyRuntimeError::new_err(
            "register_clif_vectorcall metadata must be an 8-tuple",
        ));
    }
    let state_order_obj = metadata.get_item(0)?.unbind();
    let params_obj = metadata.get_item(1)?.unbind();
    let closure_values_obj = metadata.get_item(2)?.unbind();
    let closure_layout_obj = metadata.get_item(3)?.unbind();
    let deleted_obj = metadata.get_item(4)?.unbind();
    let no_default_obj = metadata.get_item(5)?.unbind();
    let bind_kind = metadata.get_item(6)?.extract::<i32>()?;
    let materialize_entry_obj = metadata.get_item(7)?.unbind();
    let state_order_bound = state_order_obj.bind(py);
    let params_bound = params_obj.bind(py);
    let closure_values_bound = closure_values_obj.bind(py);
    let closure_layout_bound = closure_layout_obj.bind(py);
    let deleted_bound = deleted_obj.bind(py);
    let no_default_bound = no_default_obj.bind(py);
    let materialize_entry_bound = materialize_entry_obj.bind(py);
    unsafe {
        soac_eval::tree_walk::register_clif_vectorcall(
            func.as_ptr(),
            module_name,
            function_id,
            state_order_bound.as_ptr(),
            if params_bound.is_none() {
                std::ptr::null_mut()
            } else {
                params_bound.as_ptr()
            },
            if closure_values_bound.is_none() {
                std::ptr::null_mut()
            } else {
                closure_values_bound.as_ptr()
            },
            if closure_layout_bound.is_none() {
                std::ptr::null_mut()
            } else {
                closure_layout_bound.as_ptr()
            },
            deleted_bound.as_ptr(),
            no_default_bound.as_ptr(),
            bind_kind,
            if materialize_entry_bound.is_none() {
                std::ptr::null_mut()
            } else {
                materialize_entry_bound.as_ptr()
            },
        )
        .map_err(|_| {
            if ffi::PyErr_Occurred().is_null() {
                PyRuntimeError::new_err("failed to register CLIF vectorcall")
            } else {
                PyErr::fetch(py)
            }
        })
    }
}

#[pyfunction]
fn register_clif_vectorcall(
    py: Python<'_>,
    func: Py<PyAny>,
    module_name: String,
    function_id: usize,
    metadata: Py<PyTuple>,
) -> PyResult<()> {
    let func = func.bind(py);
    let metadata = metadata.bind(py);
    register_clif_vectorcall_impl(py, &func, &module_name, function_id, &metadata)
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
        soac_eval::tree_walk::compile_clif_vectorcall(func.as_ptr()).map_err(|_| {
            if ffi::PyErr_Occurred().is_null() {
                PyRuntimeError::new_err("failed to eagerly compile CLIF entry")
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
    module.add_function(wrap_pyfunction!(jit_has_bb_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_block_param_names, module)?)?;
    module.add_function(wrap_pyfunction!(jit_debug_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_render_bb_with_cfg_plan, module)?)?;
    module.add_function(wrap_pyfunction!(register_clif_vectorcall, module)?)?;
    module.add_function(wrap_pyfunction!(jit_compile_clif_wrapper, module)?)?;
    Ok(())
}

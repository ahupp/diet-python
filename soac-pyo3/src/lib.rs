#![allow(unsafe_op_in_unsafe_fn)]

use dp_transform::block_py::{BlockPyFunction, BlockPyModule};
use dp_transform::passes::PreparedBbBlockPyPass;
use dp_transform::{Options, transform_str_to_ruff_with_options};
use log::{info, trace};
use pyo3::exceptions::{
    PyAttributeError, PyNotImplementedError, PyRuntimeError, PyTypeError, PyValueError,
};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyFunction, PyModule, PyTuple};
use serde_json::json;
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
    if let Some(bb_codegen) = output.get_pass::<BlockPyModule<PreparedBbBlockPyPass>>("bb_codegen")
    {
        soac_eval::jit::register_clif_module_plans(module_name, bb_codegen).map_err(|err| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "failed to register BB plans for {module_name}: {err}"
            ))
        })?;
        // Modules executed via `python -m pkg` are transformed under
        // loader fullname `pkg.__main__` but run with `__name__ == "__main__"`.
        // BB runtime wrappers resolve plans from module globals, so register an
        // alias under "__main__" to keep lookup consistent for `python -m`.
        if module_name.ends_with(".__main__") && module_name != "__main__" {
            soac_eval::jit::register_clif_module_plans("__main__", bb_codegen).map_err(|err| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "failed to register BB plans alias for __main__ from {module_name}: {err}"
                ))
            })?;
        }
    }
    Ok(output.to_string())
}

#[pyfunction]
fn debug_pass_shape(
    py: Python<'_>,
    source: &str,
    pass_name: &str,
    ensure: Option<bool>,
) -> PyResult<Py<PyDict>> {
    let output = lower_source(source, ensure)?;
    let summary = output
        .summarize_pass_shape(pass_name)
        .ok_or_else(|| PyRuntimeError::new_err(format!("no tracked pass named {pass_name}")))?;
    let payload = PyDict::new(py);
    payload.set_item("contains_await", summary.contains_await)?;
    payload.set_item("contains_yield", summary.contains_yield)?;
    payload.set_item("contains_dp_add", summary.contains_dp_add)?;
    Ok(payload.unbind())
}

#[pyfunction]
fn inspect_pipeline(source: &str, ensure: Option<bool>) -> PyResult<String> {
    let output = lower_source(source, ensure)?;
    let mut steps = vec![json!({
        "key": "input_source",
        "label": "input source",
        "text": source,
    })];
    for name in output.pass_names() {
        let text = output
            .render_pass_text(name)
            .unwrap_or_else(|| format!("; no text renderer for pass {name}"));
        steps.push(json!({
            "key": name,
            "label": name,
            "text": text,
        }));
    }
    Ok(json!({ "steps": steps }).to_string())
}

fn import_dp_module<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyModule>> {
    PyModule::import(py, "__dp__")
}

fn make_lazy_clif_entry<'py>(
    py: Python<'py>,
    dp: &Bound<'py, PyModule>,
    async_entry: bool,
    function_name: &str,
    module_globals: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let module_globals = module_globals
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("module_globals must be a dict"))?;
    let code = dp
        .getattr("_bb_entry_template_code")?
        .call1((async_entry,))?;
    unsafe {
        let func = ffi::PyFunction_New(code.as_ptr(), module_globals.as_ptr());
        if func.is_null() {
            return Err(PyErr::fetch(py));
        }
        let func = Bound::from_owned_ptr(py, func);
        func.setattr("__name__", function_name)?;
        Ok(func)
    }
}

fn register_clif_vectorcall_raw(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    module_name: &str,
    function_id: usize,
    state_order: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
    param_defaults: Option<&Bound<'_, PyAny>>,
    closure_values: Option<&Bound<'_, PyAny>>,
    closure_layout: Option<&Bound<'_, PyAny>>,
    deleted_value: &Bound<'_, PyAny>,
    bind_kind: i32,
    materialize_result: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    unsafe {
        soac_eval::tree_walk::register_clif_vectorcall(
            func.as_ptr(),
            module_name,
            function_id,
            state_order.as_ptr(),
            params.map_or(std::ptr::null_mut(), |value| value.as_ptr()),
            param_defaults.map_or(std::ptr::null_mut(), |value| value.as_ptr()),
            closure_values.map_or(std::ptr::null_mut(), |value| value.as_ptr()),
            closure_layout.map_or(std::ptr::null_mut(), |value| value.as_ptr()),
            deleted_value.as_ptr(),
            bind_kind,
            materialize_result.map_or(std::ptr::null_mut(), |value| value.as_ptr()),
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

fn eager_clif_compile_requested() -> bool {
    std::env::var("DIET_PYTHON_JIT_COMPILE_MODE")
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("eager"))
        .unwrap_or(false)
}

fn maybe_eager_compile_clif_entry(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    module_name: &str,
    plan_name: &str,
) -> PyResult<()> {
    if !eager_clif_compile_requested() {
        return Ok(());
    }
    let start = Instant::now();
    let compile_result = unsafe {
        soac_eval::tree_walk::compile_clif_vectorcall(func.as_ptr()).map_err(|_| {
            if ffi::PyErr_Occurred().is_null() {
                PyRuntimeError::new_err("failed to eagerly compile CLIF entry")
            } else {
                PyErr::fetch(py)
            }
        })
    };
    match compile_result {
        Ok(()) => {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            info!(
                "soac_jit_eager_compile module={} qualname={} elapsed_ms={elapsed_ms:.3}",
                module_name, plan_name
            );
            Ok(())
        }
        Err(err) if err.is_instance_of::<PyNotImplementedError>(py) => Err(err),
        Err(err) => Err(PyRuntimeError::new_err(format!(
            "failed to eagerly compile CLIF entry for {module_name}.{plan_name}: {err}"
        ))),
    }
}

fn register_lazy_clif_vectorcall(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    module_name: &str,
    function_id: usize,
    plan_name: &str,
    state_order: &Bound<'_, PyAny>,
    params: Option<&Bound<'_, PyAny>>,
    param_defaults: Option<&Bound<'_, PyAny>>,
    closure_values: Option<&Bound<'_, PyAny>>,
    closure_layout: Option<&Bound<'_, PyAny>>,
    deleted_value: &Bound<'_, PyAny>,
    bind_kind: i32,
    materialize_result: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    match register_clif_vectorcall_raw(
        py,
        func,
        module_name,
        function_id,
        state_order,
        params,
        param_defaults,
        closure_values,
        closure_layout,
        deleted_value,
        bind_kind,
        materialize_result,
    ) {
        Ok(()) => maybe_eager_compile_clif_entry(py, func, module_name, plan_name),
        Err(err) if err.is_instance_of::<PyNotImplementedError>(py) => Err(err),
        Err(err) => Err(PyRuntimeError::new_err(format!(
            "failed to register lazy CLIF vectorcall for {module_name}.{plan_name}: {err}"
        ))),
    }
}

fn ignore_attr_or_type_error(py: Python<'_>, result: PyResult<()>) -> PyResult<()> {
    match result {
        Ok(()) => Ok(()),
        Err(err)
            if err.is_instance_of::<PyAttributeError>(py)
                || err.is_instance_of::<PyTypeError>(py) =>
        {
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn ignore_attr_or_value_error<T>(py: Python<'_>, result: PyResult<T>) -> PyResult<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err)
            if err.is_instance_of::<PyAttributeError>(py)
                || err.is_instance_of::<PyValueError>(py) =>
        {
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

fn update_function_metadata(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    qualname: &str,
    name: &str,
    doc: Option<&str>,
    annotate_fn: &Bound<'_, PyAny>,
) -> PyResult<()> {
    ignore_attr_or_type_error(py, func.setattr("__qualname__", qualname))?;
    ignore_attr_or_type_error(py, func.setattr("__name__", name))?;
    if func.cast::<PyFunction>().is_ok() {
        let kwargs = PyDict::new(py);
        kwargs.set_item("co_name", name)?;
        kwargs.set_item("co_qualname", qualname)?;
        if let Some(replaced) = ignore_attr_or_value_error(
            py,
            func.getattr("__code__")?
                .call_method("replace", (), Some(&kwargs)),
        )? {
            ignore_attr_or_type_error(py, func.setattr("__code__", replaced))?;
        }
    }
    if let Some(doc) = doc {
        ignore_attr_or_type_error(py, func.setattr("__doc__", doc))?;
    }
    if !annotate_fn.is_none() {
        ignore_attr_or_type_error(py, func.setattr("__annotate__", annotate_fn))?;
    }
    Ok(())
}

fn set_plan_metadata(
    func: &Bound<'_, PyAny>,
    module_name: &str,
    function_id: usize,
    plan_name: &str,
    module_globals: &Bound<'_, PyAny>,
    entry_ref: Option<&str>,
) -> PyResult<()> {
    func.setattr("__dp_plan_module", module_name)?;
    func.setattr("__dp_function_id", function_id)?;
    func.setattr("__dp_plan_name", plan_name)?;
    if let Some(entry_ref) = entry_ref {
        func.setattr("__dp_entry_ref", entry_ref)?;
    }
    if module_globals.cast::<PyDict>().is_ok() {
        func.setattr("__dp_plan_globals", module_globals)?;
    }
    Ok(())
}

fn resolve_module_name(module_globals: &Bound<'_, PyAny>, operation: &str) -> PyResult<String> {
    let globals = module_globals
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("module_globals must be a dict"))?;
    let Some(module_name_obj) = globals.get_item("__name__")? else {
        return Err(PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} requires module_globals['__name__']"
        )));
    };
    module_name_obj.extract::<String>().map_err(|_| {
        PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} requires module_globals['__name__'] to be a str"
        ))
    })
}

fn lookup_bb_function(
    module_name: &str,
    function_id: usize,
    operation: &str,
) -> PyResult<BlockPyFunction<PreparedBbBlockPyPass>> {
    soac_eval::jit::lookup_blockpy_function(module_name, function_id).ok_or_else(|| {
        PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} failed to resolve static function metadata for {module_name}.fn#{function_id}"
        ))
    })
}

fn entry_state_order(function: &BlockPyFunction<PreparedBbBlockPyPass>) -> Vec<String> {
    function.entry_block().param_name_vec()
}

fn py_param_specs(
    py: Python<'_>,
    function: &BlockPyFunction<PreparedBbBlockPyPass>,
) -> PyResult<Py<PyTuple>> {
    let params = function
        .params
        .params
        .iter()
        .map(|param| {
            (
                param.name.clone(),
                format!("{:?}", param.kind),
                param.has_default,
            )
        })
        .collect::<Vec<_>>();
    Ok(PyTuple::new(py, params)?.unbind())
}

fn ensure_bb_plan(
    module_name: &str,
    function: &BlockPyFunction<PreparedBbBlockPyPass>,
    operation: &str,
) -> PyResult<String> {
    let plan_name = function
        .function_id
        .plan_qualname(function.names.qualname.as_str());
    if soac_eval::jit::lookup_clif_plan(module_name, function.function_id.0).is_none() {
        return Err(PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} requires a registered plan, but none is available for {module_name}.{plan_name}"
        )));
    }
    Ok(plan_name)
}

fn build_closure_map<'py>(
    py: Python<'py>,
    closure_names: &Bound<'py, PyAny>,
    closure_values: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let closure_names = closure_names.cast::<PyTuple>().map_err(|_| {
        PyTypeError::new_err(format!(
            "generator resume closure_names must be a tuple, got {:?}",
            closure_names.get_type()
        ))
    })?;
    let closure_values = closure_values.cast::<PyTuple>().map_err(|_| {
        PyTypeError::new_err(format!(
            "generator resume closure_values must be a tuple, got {:?}",
            closure_values.get_type()
        ))
    })?;
    if closure_names.len() != closure_values.len() {
        return Err(PyRuntimeError::new_err(format!(
            "generator resume closure metadata length mismatch: {} names vs {} values",
            closure_names.len(),
            closure_values.len()
        )));
    }

    let closure_map = PyDict::new(py);
    for (name_obj, value_obj) in closure_names.iter().zip(closure_values.iter()) {
        let name = name_obj.extract::<String>().map_err(|_| {
            PyTypeError::new_err("generator resume closure_names entries must be strings")
        })?;
        closure_map.set_item(name, value_obj)?;
    }
    Ok(closure_map)
}

#[pyfunction]
fn make_bb_function(
    py: Python<'_>,
    function_id: usize,
    captures: Py<PyAny>,
    param_defaults: Py<PyAny>,
    module_globals: Py<PyAny>,
    annotate_fn: Py<PyAny>,
) -> PyResult<Py<PyAny>> {
    let dp = import_dp_module(py)?;
    let module_globals = module_globals.bind(py);
    let module_name = resolve_module_name(&module_globals, "function instantiation")?;
    let function = lookup_bb_function(&module_name, function_id, "function instantiation")?;
    let plan_name = ensure_bb_plan(&module_name, &function, "function instantiation")?;
    let params = py_param_specs(py, &function)?;
    let state_order = PyTuple::new(py, entry_state_order(&function))?.unbind();
    let signature_info = dp
        .getattr("_build_bb_signature")?
        .call1((params.bind(py), param_defaults.bind(py)))?;
    let signature = signature_info.cast::<PyTuple>()?.get_item(0)?.unbind();
    let closure_values = dp
        .getattr("_bb_capture_values")?
        .call1((captures.bind(py),))?
        .unbind();
    let raw_entry = make_lazy_clif_entry(
        py,
        &dp,
        false,
        function.names.display_name.as_str(),
        &module_globals,
    )?;
    let deleted_value = dp.getattr("DELETED")?;
    register_lazy_clif_vectorcall(
        py,
        &raw_entry,
        module_name.as_str(),
        function_id,
        plan_name.as_str(),
        state_order.bind(py).as_any(),
        Some(params.bind(py).as_any()),
        Some(param_defaults.bind(py)),
        Some(closure_values.bind(py)),
        None,
        &deleted_value,
        0,
        None,
    )?;
    let entry = dp
        .getattr("_bb_wrap_with_closure")?
        .call1((raw_entry, closure_values.bind(py)))?;
    let entry = dp
        .getattr("_bb_rebind_function_globals")?
        .call1((entry, &module_globals))?;
    entry.setattr("__signature__", signature.bind(py))?;
    update_function_metadata(
        py,
        &entry,
        function.names.qualname.as_str(),
        function.names.display_name.as_str(),
        function.doc.as_deref(),
        annotate_fn.bind(py),
    )?;
    entry.setattr("__module__", module_name.as_str())?;
    Ok(entry.unbind())
}

#[pyfunction]
#[pyo3(signature = (function_id, closure_names, closure_values, module_globals, async_gen=false))]
fn make_bb_hidden_resume(
    py: Python<'_>,
    function_id: usize,
    closure_names: Py<PyAny>,
    closure_values: Py<PyAny>,
    module_globals: Py<PyAny>,
    async_gen: bool,
) -> PyResult<Py<PyAny>> {
    let dp = import_dp_module(py)?;
    let module_globals = module_globals.bind(py);
    let operation = if async_gen {
        "async generator resume"
    } else {
        "generator resume"
    };
    let module_name = resolve_module_name(&module_globals, operation)?;
    let function = lookup_bb_function(&module_name, function_id, operation)?;
    let plan_name = ensure_bb_plan(&module_name, &function, operation)?;
    let state_order = PyTuple::new(py, entry_state_order(&function))?.unbind();
    let closure_map = build_closure_map(py, &closure_names.bind(py), &closure_values.bind(py))?;
    let hidden_name = format!("_dp_resume_{}", function.names.fn_name);
    let raw_entry = make_lazy_clif_entry(py, &dp, false, hidden_name.as_str(), &module_globals)?;
    let deleted_value = dp.getattr("DELETED")?;
    register_lazy_clif_vectorcall(
        py,
        &raw_entry,
        module_name.as_str(),
        function_id,
        plan_name.as_str(),
        state_order.bind(py).as_any(),
        None,
        None,
        Some(closure_map.as_any()),
        None,
        &deleted_value,
        if async_gen { 2 } else { 1 },
        None,
    )?;
    let entry = dp
        .getattr("_bb_wrap_with_closure")?
        .call1((raw_entry, &closure_map))?;
    let entry = dp
        .getattr("_bb_rebind_function_globals")?
        .call1((entry, &module_globals))?;
    entry.setattr("__module__", module_name.as_str())?;
    set_plan_metadata(
        &entry,
        module_name.as_str(),
        function_id,
        plan_name.as_str(),
        &module_globals,
        Some(function.entry_block().label_str()),
    )?;
    Ok(entry.unbind())
}

#[pyfunction]
#[pyo3(signature = (function_id, resume, module_globals, async_gen=false))]
fn make_bb_generator(
    py: Python<'_>,
    function_id: usize,
    resume: Py<PyAny>,
    module_globals: Py<PyAny>,
    async_gen: bool,
) -> PyResult<Py<PyAny>> {
    let dp = import_dp_module(py)?;
    let module_globals = module_globals.bind(py);
    let operation = if async_gen {
        "async generator construction"
    } else {
        "generator construction"
    };
    let module_name = resolve_module_name(&module_globals, operation)?;
    let function = lookup_bb_function(&module_name, function_id, operation)?;
    let name = function.names.display_name.clone();
    let qualname = function.names.qualname.clone();
    let code = if async_gen {
        dp.getattr("_dp_make_async_gen_code")?
            .call1((name.as_str(), qualname.as_str()))?
    } else {
        dp.getattr("_dp_make_gen_code")?
            .call1((name.as_str(), qualname.as_str()))?
    };
    let kwargs = PyDict::new(py);
    kwargs.set_item("resume", resume.bind(py))?;
    kwargs.set_item("name", name.as_str())?;
    kwargs.set_item("qualname", qualname.as_str())?;
    kwargs.set_item("code", code)?;
    let cls_name = if async_gen {
        "_DpClosureAsyncGenerator"
    } else {
        "_DpClosureGenerator"
    };
    let generator = dp.getattr(cls_name)?.call((), Some(&kwargs))?;
    Ok(generator.unbind())
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
    let param_defaults_obj = metadata.get_item(2)?.unbind();
    let closure_values_obj = metadata.get_item(3)?.unbind();
    let closure_layout_obj = metadata.get_item(4)?.unbind();
    let deleted_obj = metadata.get_item(5)?.unbind();
    let bind_kind = metadata.get_item(6)?.extract::<i32>()?;
    let materialize_entry_obj = metadata.get_item(7)?.unbind();
    let state_order_bound = state_order_obj.bind(py);
    let params_bound = params_obj.bind(py);
    let param_defaults_bound = param_defaults_obj.bind(py);
    let closure_values_bound = closure_values_obj.bind(py);
    let closure_layout_bound = closure_layout_obj.bind(py);
    let deleted_bound = deleted_obj.bind(py);
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
            if param_defaults_bound.is_none() {
                std::ptr::null_mut()
            } else {
                param_defaults_bound.as_ptr()
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
    module.add_function(wrap_pyfunction!(debug_pass_shape, module)?)?;
    module.add_function(wrap_pyfunction!(inspect_pipeline, module)?)?;
    module.add_function(wrap_pyfunction!(make_bb_function, module)?)?;
    module.add_function(wrap_pyfunction!(make_bb_hidden_resume, module)?)?;
    module.add_function(wrap_pyfunction!(make_bb_generator, module)?)?;
    module.add_function(wrap_pyfunction!(jit_has_bb_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_block_param_names, module)?)?;
    module.add_function(wrap_pyfunction!(jit_debug_plan, module)?)?;
    module.add_function(wrap_pyfunction!(jit_render_bb_with_cfg_plan, module)?)?;
    module.add_function(wrap_pyfunction!(register_clif_vectorcall, module)?)?;
    module.add_function(wrap_pyfunction!(jit_compile_clif_wrapper, module)?)?;
    Ok(())
}

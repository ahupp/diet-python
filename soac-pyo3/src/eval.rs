use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyModule, PyTuple};
use std::ffi::c_void;

struct ResolvedSpecializedJitBlocks {
    function: dp_transform::block_py::BlockPyFunction<dp_transform::passes::CodegenBlockPyPass>,
    block_ptrs: Vec<*mut c_void>,
    true_obj: *mut c_void,
    false_obj: *mut c_void,
}

pub(crate) fn jit_has_bb_plan_impl(module_name: &str, function_id: usize) -> bool {
    soac_eval::jit::lookup_blockpy_function(module_name, function_id).is_some()
}

pub(crate) fn jit_block_param_names_impl(
    module_name: &str,
    function_id: usize,
    entry_label: &str,
) -> PyResult<Vec<String>> {
    let Some(function) = soac_eval::jit::lookup_blockpy_function(module_name, function_id) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.fn#{function_id}"
        )));
    };
    let Some(index) = function
        .blocks
        .iter()
        .position(|block| block.label.to_string() == entry_label)
    else {
        return Err(PyRuntimeError::new_err(format!(
            "entry label {:?} not found in plan {module_name}.fn#{}",
            entry_label, function_id
        )));
    };
    Ok(function.blocks[index].param_name_vec())
}

pub(crate) fn jit_debug_plan_impl(module_name: &str, function_id: usize) -> PyResult<String> {
    let Some(function) = soac_eval::jit::lookup_blockpy_function(module_name, function_id) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.fn#{function_id}"
        )));
    };
    let block_info = function
        .blocks
        .iter()
        .map(|block| soac_eval::jit::jit_block_info(&function, block))
        .collect::<Vec<_>>();
    Ok(format!(
        "function:\n{function:#?}\n\njit_blocks:\n{block_info:#?}"
    ))
}

fn resolve_specialized_jit_blocks_by_key(
    py: Python<'_>,
    module_name: &str,
    function_id: usize,
) -> PyResult<ResolvedSpecializedJitBlocks> {
    let Some(function) = soac_eval::jit::lookup_blockpy_function(module_name, function_id) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.fn#{function_id}"
        )));
    };
    let block_ptrs = vec![std::ptr::null_mut::<c_void>(); function.blocks.len()];
    if block_ptrs.is_empty() {
        return Err(PyRuntimeError::new_err(format!(
            "invalid JIT plan with no blocks for {module_name}.fn#{function_id}"
        )));
    }

    let true_obj = PyBool::new(py, true).as_ptr() as *mut c_void;
    let false_obj = PyBool::new(py, false).as_ptr() as *mut c_void;

    Ok(ResolvedSpecializedJitBlocks {
        function,
        block_ptrs,
        true_obj,
        false_obj,
    })
}

pub(crate) fn jit_render_bb_with_cfg_plan_impl(
    py: Python<'_>,
    module_name: &str,
    function_id: usize,
) -> PyResult<(String, String, String)> {
    let resolved = resolve_specialized_jit_blocks_by_key(py, module_name, function_id)?;
    let empty_tuple_obj = PyTuple::empty(py);
    PyModule::import(py, "__dp__")?;
    let builtins = PyModule::import(py, "builtins")?;
    let deleted_obj = builtins.getattr("__dp_DELETED")?;
    unsafe {
        soac_eval::jit::render_cranelift_run_bb_specialized_with_cfg(
            resolved.block_ptrs.as_slice(),
            &resolved.function,
            resolved.true_obj,
            resolved.false_obj,
            deleted_obj.as_ptr() as *mut c_void,
            empty_tuple_obj.as_ptr() as *mut c_void,
        )
        .map(|rendered| (rendered.clif, rendered.cfg_dot, rendered.vcode_disasm))
        .map_err(PyRuntimeError::new_err)
    }
}

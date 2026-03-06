use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyTuple};
use std::ffi::c_void;

struct ResolvedSpecializedJitBlocks {
    plan: soac_eval::jit::ClifPlan,
    block_ptrs: Vec<*mut c_void>,
    true_obj: *mut c_void,
    false_obj: *mut c_void,
}

pub(crate) fn jit_has_bb_plan_impl(module_name: &str, qualname: &str) -> bool {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, qualname) else {
        return false;
    };
    let has_none = plan
        .block_fast_paths
        .iter()
        .any(|path| matches!(path, soac_eval::jit::BlockFastPath::None));
    if has_none && std::env::var("DIET_PYTHON_DEBUG_JIT_HAS").as_deref() == Ok("1") {
        eprintln!("jit_has_bb_plan=false for {module_name}.{qualname}");
        for (idx, (label, path)) in plan
            .block_labels
            .iter()
            .zip(plan.block_fast_paths.iter())
            .enumerate()
        {
            eprintln!(
                "  [{idx}] {label}: {path:?}, exc_target={:?}",
                plan.block_exc_targets.get(idx).copied().flatten()
            );
        }
    }
    !has_none
}

pub(crate) fn jit_block_param_names_impl(
    module_name: &str,
    qualname: &str,
    entry_label: &str,
) -> PyResult<Vec<String>> {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, qualname) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.{qualname}"
        )));
    };
    let Some(index) = plan
        .block_labels
        .iter()
        .position(|label| label == entry_label)
    else {
        return Err(PyRuntimeError::new_err(format!(
            "entry label {:?} not found in plan {module_name}.{qualname}",
            entry_label
        )));
    };
    Ok(plan.block_param_names[index].clone())
}

pub(crate) fn jit_debug_plan_impl(module_name: &str, qualname: &str) -> PyResult<String> {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, qualname) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.{qualname}"
        )));
    };
    Ok(format!("{plan:#?}"))
}

fn resolve_specialized_jit_blocks_by_key(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
) -> PyResult<ResolvedSpecializedJitBlocks> {
    let plan = soac_eval::jit::lookup_clif_plan(module_name, qualname);
    let Some(plan) = plan else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.{qualname}"
        )));
    };
    if plan
        .block_fast_paths
        .iter()
        .any(|path| matches!(path, soac_eval::jit::BlockFastPath::None))
    {
        return Err(PyRuntimeError::new_err(format!(
            "specialized JIT requires fully lowered fastpath blocks: {module_name}.{qualname}"
        )));
    }
    let block_ptrs = vec![std::ptr::null_mut::<c_void>(); plan.block_labels.len()];
    if plan.entry_index >= block_ptrs.len() {
        return Err(PyRuntimeError::new_err(format!(
            "invalid JIT entry index {} for {} blocks",
            plan.entry_index,
            block_ptrs.len()
        )));
    }

    let true_obj = PyBool::new(py, true).as_ptr() as *mut c_void;
    let false_obj = PyBool::new(py, false).as_ptr() as *mut c_void;

    Ok(ResolvedSpecializedJitBlocks {
        plan,
        block_ptrs,
        true_obj,
        false_obj,
    })
}

pub(crate) fn jit_render_bb_with_cfg_plan_impl(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
) -> PyResult<(String, String)> {
    let resolved = resolve_specialized_jit_blocks_by_key(py, module_name, qualname)?;
    let empty_tuple_obj = PyTuple::empty(py);
    unsafe {
        soac_eval::jit::render_cranelift_run_bb_specialized_with_cfg(
            resolved.block_ptrs.as_slice(),
            &resolved.plan,
            resolved.true_obj,
            resolved.false_obj,
            empty_tuple_obj.as_ptr() as *mut c_void,
        )
        .map(|rendered| (rendered.clif, rendered.cfg_dot))
        .map_err(PyRuntimeError::new_err)
    }
}

#[cfg(test)]
mod tests {
    use dp_transform::basic_block::bb_ir;
    use std::any::Any;

    fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
        if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        }
    }

    fn parse_and_lower(source: &str) -> Result<dp_transform::LoweringResult, String> {
        let options = dp_transform::Options {
            inject_import: true,
            eval_mode: true,
            lower_attributes: true,
            truthy: false,
            force_import_rewrite: true,
            ..dp_transform::Options::default()
        };

        match std::panic::catch_unwind(|| {
            dp_transform::transform_str_to_ruff_with_options(source, options)
        }) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(err.to_string()),
            Err(payload) => Err(panic_payload_to_string(payload)),
        }
    }

    fn validate_bb_module_for_jit(bb_module: Option<&bb_ir::BbModule>) -> Result<(), String> {
        let bb_module = bb_module.ok_or_else(|| {
            "JIT mode requires emitted basic-block IR, but none was produced".to_string()
        })?;
        for function in &bb_module.functions {
            match &function.kind {
                bb_ir::BbFunctionKind::Function
                | bb_ir::BbFunctionKind::Generator { .. }
                | bb_ir::BbFunctionKind::AsyncGenerator { .. } => {}
            }
        }
        Ok(())
    }

    fn run_cranelift_jit_preflight(bb_module: Option<&bb_ir::BbModule>) -> Result<(), String> {
        let bb_module = bb_module.ok_or_else(|| {
            "JIT mode requires emitted basic-block IR, but none was produced".to_string()
        })?;
        let normalized = dp_transform::basic_block::normalize_bb_module_for_codegen(bb_module);
        soac_eval::jit::run_cranelift_smoke(&normalized)
    }

    #[test]
    fn jit_validator_accepts_class_defs_without_def_fn_ops() {
        let source = r#"
class C:
    x = 1
    def m(self):
        return self.x
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref())
            .expect("validator should accept lowered class defs");
    }

    #[test]
    fn jit_validator_accepts_coroutines() {
        let source = r#"
async def run():
    return 1
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref())
            .expect("validator should accept coroutine lowering");
    }

    #[test]
    fn jit_validator_accepts_async_generators() {
        let source = r#"
async def run():
    yield 1
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref())
            .expect("validator should accept async generator lowering");
    }

    #[test]
    fn jit_validator_allows_try_jump_terminators() {
        let source = r#"
def f():
    return 1
"#;
        let mut bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module
            .expect("bb module should be present");
        let function = bb_module
            .functions
            .first_mut()
            .expect("must contain at least one function");
        let block = function
            .blocks
            .first_mut()
            .expect("function must contain at least one block");
        block.term = bb_ir::BbTerm::TryJump {
            body_label: "body".to_string(),
            except_label: "except".to_string(),
            except_exc_name: None,
            body_region_labels: vec![],
            except_region_labels: vec![],
            finally_label: None,
            finally_exc_name: None,
            finally_region_labels: vec![],
            finally_fallthrough_label: None,
        };
        validate_bb_module_for_jit(Some(&bb_module)).expect("validator should allow try_jump");
    }

    #[test]
    fn jit_preflight_runs_cranelift_for_supported_module() {
        let source = r#"
def f(x):
    return x
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref()).expect("validator should allow module");
        run_cranelift_jit_preflight(bb_module.as_ref()).expect("cranelift preflight should run");
    }
}

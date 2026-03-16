use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyModule, PyTuple};
use std::ffi::c_void;

struct ResolvedSpecializedJitBlocks {
    plan: soac_eval::jit::ClifPlan,
    block_ptrs: Vec<*mut c_void>,
    true_obj: *mut c_void,
    false_obj: *mut c_void,
}

pub(crate) fn jit_has_bb_plan_impl(module_name: &str, function_id: usize) -> bool {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, function_id) else {
        return false;
    };
    let has_none = plan
        .block_fast_paths
        .iter()
        .any(|path| matches!(path, soac_eval::jit::BlockFastPath::None));
    if has_none && std::env::var("DIET_PYTHON_DEBUG_JIT_HAS").as_deref() == Ok("1") {
        eprintln!("jit_has_bb_plan=false for {module_name}.fn#{function_id}");
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
    function_id: usize,
    entry_label: &str,
) -> PyResult<Vec<String>> {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, function_id) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.fn#{function_id}"
        )));
    };
    let Some(index) = plan
        .block_labels
        .iter()
        .position(|label| label == entry_label)
    else {
        return Err(PyRuntimeError::new_err(format!(
            "entry label {:?} not found in plan {module_name}.fn#{}",
            entry_label, function_id
        )));
    };
    Ok(plan.block_param_names[index].clone())
}

pub(crate) fn jit_debug_plan_impl(module_name: &str, function_id: usize) -> PyResult<String> {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, function_id) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.fn#{function_id}"
        )));
    };
    Ok(format!("{plan:#?}"))
}

fn resolve_specialized_jit_blocks_by_key(
    py: Python<'_>,
    module_name: &str,
    function_id: usize,
) -> PyResult<ResolvedSpecializedJitBlocks> {
    let plan = soac_eval::jit::lookup_clif_plan(module_name, function_id);
    let Some(plan) = plan else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.fn#{function_id}"
        )));
    };
    if plan
        .block_fast_paths
        .iter()
        .any(|path| matches!(path, soac_eval::jit::BlockFastPath::None))
    {
        return Err(PyRuntimeError::new_err(format!(
            "specialized JIT requires fully lowered fastpath blocks: {module_name}.fn#{function_id}"
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
            &resolved.plan,
            resolved.true_obj,
            resolved.false_obj,
            deleted_obj.as_ptr() as *mut c_void,
            empty_tuple_obj.as_ptr() as *mut c_void,
        )
        .map(|rendered| (rendered.clif, rendered.cfg_dot, rendered.vcode_disasm))
        .map_err(PyRuntimeError::new_err)
    }
}

#[cfg(test)]
mod tests {
    use dp_transform::basic_block::bb_ir;
    use soac_eval::jit::{self, BlockExcArgSource};
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
            eval_mode: true,
            lower_attributes: true,
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

    fn parse_and_lower_runtime_style(source: &str) -> Result<dp_transform::LoweringResult, String> {
        match std::panic::catch_unwind(|| {
            dp_transform::transform_str_to_ruff_with_options(
                source,
                dp_transform::Options {
                    lower_attributes: false,
                    ..dp_transform::Options::default()
                },
            )
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
        for function in &bb_module.callable_defs {
            match &function.kind {
                dp_transform::basic_block::lowered_ir::LoweredFunctionKind::Function
                | dp_transform::basic_block::lowered_ir::LoweredFunctionKind::Generator {
                    ..
                }
                | dp_transform::basic_block::lowered_ir::LoweredFunctionKind::AsyncGenerator {
                    ..
                } => {}
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
    fn jit_validator_accepts_lowered_try_blocks() {
        let source = r#"
def f():
    try:
        return 1
    except Exception:
        return 2
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref())
            .expect("validator should accept lowered try blocks");
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

    #[test]
    fn generator_throw_handler_plan_keeps_try_exception_param() {
        let source = r#"
def exercise():
    outer_capture = 2
    def gen():
        total = 1
        try:
            total += outer_capture
            yield total
        except ValueError as exc:
            total += len(str(exc))
        yield total
    return gen
"#;
        let bb_module = parse_and_lower_runtime_style(source)
            .expect("lowering should succeed")
            .bb_module
            .expect("bb module should exist");
        let normalized = dp_transform::basic_block::normalize_bb_module_for_codegen(&bb_module);
        let module_name = "jit_plan_generator_throw_handler_param_test";
        jit::register_clif_module_plans(module_name, &normalized)
            .expect("plan registration should succeed");
        let gen_function = normalized
            .callable_defs
            .iter()
            .find(|function| function.qualname == "exercise.<locals>.gen")
            .expect("missing lowered generator function");
        let plan = jit::lookup_clif_plan(module_name, gen_function.function_id.0)
            .expect("registered plan should exist");

        let handler_entry_targets = plan
            .block_param_names
            .iter()
            .enumerate()
            .filter(|(_, params)| params.iter().any(|name| name.starts_with("_dp_try_exc_")))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        assert!(
            !handler_entry_targets.is_empty(),
            "expected at least one except handler block with an explicit try-exception carrier: {:?}",
            plan.block_param_names
        );
        assert!(
            plan.block_exc_dispatches.iter().flatten().any(|dispatch| {
                handler_entry_targets.contains(&dispatch.target_index)
                    && dispatch
                        .arg_sources
                        .iter()
                        .any(|source| matches!(source, BlockExcArgSource::Exception))
            }),
            "expected a dispatch into an except handler target to pass the active exception: {:?}",
            plan.block_exc_dispatches
                .iter()
                .enumerate()
                .filter_map(|(index, dispatch)| {
                    dispatch.as_ref().map(|dispatch| {
                        (
                            &plan.block_labels[index],
                            &plan.block_labels[dispatch.target_index],
                            &dispatch.arg_sources,
                        )
                    })
                })
                .collect::<Vec<_>>()
        );
        assert!(
            plan.block_param_names
                .iter()
                .enumerate()
                .any(|(_, params)| {
                    params.iter().any(|name| name.starts_with("_dp_try_exc_"))
                        && params.iter().any(|name| name == "exc")
                }),
            "expected some lowered handler block to preserve the user-visible exception binding: {:?}",
            plan.block_param_names
                .iter()
                .enumerate()
                .filter(|(_, params)| params.iter().any(|name| name.starts_with("_dp_try_exc_")))
                .map(|(index, params)| (&plan.block_labels[index], params))
                .collect::<Vec<_>>()
        );
    }
}

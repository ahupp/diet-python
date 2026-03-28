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
        .blocks
        .iter()
        .any(|block| matches!(block.fast_path, soac_eval::jit::BlockFastPath::None));
    if has_none && std::env::var("DIET_PYTHON_DEBUG_JIT_HAS").as_deref() == Ok("1") {
        eprintln!("jit_has_bb_plan=false for {module_name}.fn#{function_id}");
        for (idx, block) in plan.blocks.iter().enumerate() {
            eprintln!(
                "  [{idx}] {label}: {path:?}, exc_target={:?}",
                block.exc_target,
                label = block.label.as_str(),
                path = &block.fast_path
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
        .blocks
        .iter()
        .position(|block| block.label == entry_label)
    else {
        return Err(PyRuntimeError::new_err(format!(
            "entry label {:?} not found in plan {module_name}.fn#{}",
            entry_label, function_id
        )));
    };
    Ok(plan.blocks[index].param_names.clone())
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
        .blocks
        .iter()
        .any(|block| matches!(block.fast_path, soac_eval::jit::BlockFastPath::None))
    {
        return Err(PyRuntimeError::new_err(format!(
            "specialized JIT requires fully lowered fastpath blocks: {module_name}.fn#{function_id}"
        )));
    }
    let block_ptrs = vec![std::ptr::null_mut::<c_void>(); plan.blocks.len()];
    if block_ptrs.is_empty() {
        return Err(PyRuntimeError::new_err(format!(
            "invalid JIT plan with no blocks for {module_name}.fn#{function_id}"
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
    use dp_transform::block_py::BlockPyFunctionKind;
    use soac_eval::jit::{self, BlockExcArgSource};
    use std::any::Any;
    use std::collections::HashSet;

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
        match std::panic::catch_unwind(|| dp_transform::transform_str_to_ruff(source)) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(err.to_string()),
            Err(payload) => Err(panic_payload_to_string(payload)),
        }
    }

    fn parse_and_lower_runtime_style(source: &str) -> Result<dp_transform::LoweringResult, String> {
        match std::panic::catch_unwind(|| dp_transform::transform_str_to_ruff(source)) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(err.to_string()),
            Err(payload) => Err(panic_payload_to_string(payload)),
        }
    }

    fn validate_bb_module_for_jit(
        bb_module: Option<
            &dp_transform::block_py::BlockPyModule<
                dp_transform::passes::ResolvedStorageBlockPyPass,
            >,
        >,
    ) -> Result<(), String> {
        let bb_module = bb_module.ok_or_else(|| {
            "JIT mode requires emitted basic-block IR, but none was produced".to_string()
        })?;
        for function in &bb_module.callable_defs {
            match function.lowered_kind() {
                BlockPyFunctionKind::Function
                | BlockPyFunctionKind::Coroutine
                | BlockPyFunctionKind::Generator
                | BlockPyFunctionKind::AsyncGenerator => {}
            }
        }
        Ok(())
    }

    fn run_cranelift_jit_preflight(result: &dp_transform::LoweringResult) -> Result<(), String> {
        let normalized = result
            .codegen_module
            .as_ref()
            .ok_or_else(|| "JIT mode requires tracked bb_codegen output".to_string())?;
        soac_eval::jit::run_cranelift_smoke(normalized)
    }

    #[test]
    fn function_plan_reports_slot_inventory_for_locals_capture_and_except_state() {
        let source = r#"
def outer(scale):
    factor = scale
    def inner(x):
        total = x
        try:
            total += factor
        except Exception as exc:
            return total + len(str(exc))
        return total
    return inner
"#;
        let result = parse_and_lower(source).expect("lowering should succeed");
        let normalized = result
            .codegen_module
            .as_ref()
            .expect("bb_codegen pass should be tracked")
            .clone();
        let module_name = "jit_plan_slot_inventory_test";
        jit::register_clif_module_plans(module_name, &normalized)
            .expect("plan registration should succeed");
        let inner_function = normalized
            .callable_defs
            .iter()
            .find(|function| function.names.bind_name == "inner")
            .expect("missing lowered inner function");
        let plan = jit::lookup_clif_plan(module_name, inner_function.function_id.0)
            .expect("registered plan should exist");

        assert_eq!(
            plan.ambient_param_names.len(),
            1,
            "expected one closure capture in ambient state: {:?}",
            plan.ambient_param_names
        );
        let capture_name = &plan.ambient_param_names[0];
        assert!(
            capture_name.contains("factor"),
            "expected capture name to track factor: {capture_name:?}"
        );
        assert_eq!(
            plan.slot_names.first(),
            Some(capture_name),
            "slot inventory should seed ambient captures first: {:?}",
            plan.slot_names
        );
        assert!(
            plan.slot_names.iter().any(|name| name == "x"),
            "expected parameter x in slot inventory: {:?}",
            plan.slot_names
        );
        assert!(
            plan.slot_names.iter().any(|name| name == "total"),
            "expected local total in slot inventory: {:?}",
            plan.slot_names
        );
        assert!(
            plan.slot_names
                .iter()
                .any(|name| name.starts_with("_dp_try_exc_")),
            "expected synthetic try-exception state in slot inventory: {:?}",
            plan.slot_names
        );

        let unique_names = plan.slot_names.iter().collect::<HashSet<_>>();
        assert_eq!(
            unique_names.len(),
            plan.slot_names.len(),
            "slot inventory should not duplicate names: {:?}",
            plan.slot_names
        );
    }

    #[test]
    fn jit_validator_accepts_class_defs_without_def_fn_ops() {
        let source = r#"
class C:
    x = 1
    def m(self):
        return self.x
"#;
        let result = parse_and_lower(source).expect("lowering should succeed");
        let bb_module =
            result
                .pass_tracker
                .get::<dp_transform::block_py::BlockPyModule<
                    dp_transform::passes::ResolvedStorageBlockPyPass,
                >>("name_binding");
        validate_bb_module_for_jit(bb_module).expect("validator should accept lowered class defs");
    }

    #[test]
    fn jit_validator_accepts_coroutines() {
        let source = r#"
async def run():
    return 1
"#;
        let result = parse_and_lower(source).expect("lowering should succeed");
        let bb_module =
            result
                .pass_tracker
                .get::<dp_transform::block_py::BlockPyModule<
                    dp_transform::passes::ResolvedStorageBlockPyPass,
                >>("name_binding");
        validate_bb_module_for_jit(bb_module).expect("validator should accept coroutine lowering");
    }

    #[test]
    fn jit_validator_accepts_async_generators() {
        let source = r#"
async def run():
    yield 1
"#;
        let result = parse_and_lower(source).expect("lowering should succeed");
        let bb_module =
            result
                .pass_tracker
                .get::<dp_transform::block_py::BlockPyModule<
                    dp_transform::passes::ResolvedStorageBlockPyPass,
                >>("name_binding");
        validate_bb_module_for_jit(bb_module)
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
        let result = parse_and_lower(source).expect("lowering should succeed");
        let bb_module =
            result
                .pass_tracker
                .get::<dp_transform::block_py::BlockPyModule<
                    dp_transform::passes::ResolvedStorageBlockPyPass,
                >>("name_binding");
        validate_bb_module_for_jit(bb_module).expect("validator should accept lowered try blocks");
    }

    #[test]
    fn jit_preflight_runs_cranelift_for_supported_module() {
        let source = r#"
def f(x):
    return x
"#;
        let result = parse_and_lower(source).expect("lowering should succeed");
        let bb_module =
            result
                .pass_tracker
                .get::<dp_transform::block_py::BlockPyModule<
                    dp_transform::passes::ResolvedStorageBlockPyPass,
                >>("name_binding");
        validate_bb_module_for_jit(bb_module).expect("validator should allow module");
        run_cranelift_jit_preflight(&result).expect("cranelift preflight should run");
    }

    #[test]
    fn generator_throw_handler_plan_keeps_try_exception_state_and_closure_exc_binding() {
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
        let result = parse_and_lower_runtime_style(source).expect("lowering should succeed");
        let normalized = result
            .codegen_module
            .as_ref()
            .expect("bb_codegen pass should be tracked")
            .clone();
        let module_name = "jit_plan_generator_throw_handler_param_test";
        jit::register_clif_module_plans(module_name, &normalized)
            .expect("plan registration should succeed");
        let gen_function = normalized
            .callable_defs
            .iter()
            .find(|function| function.names.bind_name == "gen_resume")
            .expect("missing lowered generator resume function");
        let plan = jit::lookup_clif_plan(module_name, gen_function.function_id.0)
            .expect("registered plan should exist");

        let handler_entry_targets = plan
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| {
                block
                    .param_names
                    .iter()
                    .any(|name| name.starts_with("_dp_try_exc_"))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        assert!(
            !handler_entry_targets.is_empty(),
            "expected at least one except handler block with an explicit try-exception carrier: {:?}",
            plan.blocks
        );
        assert!(
            plan.blocks
                .iter()
                .filter_map(|block| block.exc_dispatch.as_ref())
                .any(|dispatch| {
                    handler_entry_targets.contains(&dispatch.target_index)
                        && (plan.blocks[dispatch.target_index]
                            .runtime_param_names
                            .iter()
                            .any(|name| name.starts_with("_dp_try_exc_"))
                            || dispatch
                                .slot_writes
                                .iter()
                                .any(|(_, source)| matches!(source, BlockExcArgSource::Exception)))
                }),
            "expected a dispatch into an except handler target to pass the active exception: {:?}",
            plan.blocks
                .iter()
                .enumerate()
                .filter_map(|(index, block)| {
                    block.exc_dispatch.as_ref().map(|dispatch| {
                        (
                            &plan.blocks[index].label,
                            &plan.blocks[dispatch.target_index].label,
                            &dispatch.slot_writes,
                        )
                    })
                })
                .collect::<Vec<_>>()
        );

        let closure_layout = gen_function
            .closure_layout()
            .as_ref()
            .expect("hidden resume should preserve closure layout");
        assert!(
            closure_layout
                .freevars
                .iter()
                .any(|slot| slot.logical_name == "exc"),
            "expected hidden resume closure layout to preserve the user-visible exception binding as a freevar cell: {:?}",
            closure_layout
        );
        assert!(
            closure_layout
                .freevars
                .iter()
                .any(|slot| slot.logical_name == "exc" && slot.storage_name.contains("exc")),
            "expected hidden resume closure slot for exc to keep a stable cell storage name: {:?}",
            closure_layout
        );
    }
}

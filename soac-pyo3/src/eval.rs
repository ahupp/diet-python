use dp_transform::{
    Options,
    basic_block::{bb_ir, normalize_bb_module_for_codegen},
    min_ast,
    transform_str_to_ruff_with_options,
};
use pyo3::exceptions::{PyRuntimeError, PySyntaxError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use soac_eval::tree_walk::{self as interpreter, RuntimeFns};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::CString;
use std::ffi::c_void;

#[derive(Debug)]
pub(crate) enum TransformToMinAstError {
    Parse(String),
    Lowering(String),
    MinAstConversion(String),
}

pub(crate) struct EvalLoweringResult {
    pub(crate) min_ast_module: min_ast::Module,
    pub(crate) bb_module: Option<bb_ir::BbModule>,
    pub(crate) transformed_source: String,
}

#[derive(Clone, Copy)]
struct JitRunBbHooks {
    run_bb_step: *mut ffi::PyObject,
    term_kind: *mut ffi::PyObject,
    term_jump_target: *mut ffi::PyObject,
    term_jump_args: *mut ffi::PyObject,
    term_ret_value: *mut ffi::PyObject,
    term_raise: *mut ffi::PyObject,
    term_invalid: *mut ffi::PyObject,
}

thread_local! {
    static JIT_RUN_BB_HOOK_STACK: RefCell<Vec<JitRunBbHooks>> = const { RefCell::new(Vec::new()) };
}

fn push_jit_run_bb_hooks(hooks: JitRunBbHooks) {
    unsafe {
        ffi::Py_INCREF(hooks.run_bb_step);
        ffi::Py_INCREF(hooks.term_kind);
        ffi::Py_INCREF(hooks.term_jump_target);
        ffi::Py_INCREF(hooks.term_jump_args);
        ffi::Py_INCREF(hooks.term_ret_value);
        ffi::Py_INCREF(hooks.term_raise);
        ffi::Py_INCREF(hooks.term_invalid);
    }
    JIT_RUN_BB_HOOK_STACK.with(|stack| stack.borrow_mut().push(hooks));
}

fn pop_jit_run_bb_hooks() {
    let popped = JIT_RUN_BB_HOOK_STACK.with(|stack| stack.borrow_mut().pop());
    if let Some(hooks) = popped {
        unsafe {
            ffi::Py_DECREF(hooks.run_bb_step);
            ffi::Py_DECREF(hooks.term_kind);
            ffi::Py_DECREF(hooks.term_jump_target);
            ffi::Py_DECREF(hooks.term_jump_args);
            ffi::Py_DECREF(hooks.term_ret_value);
            ffi::Py_DECREF(hooks.term_raise);
            ffi::Py_DECREF(hooks.term_invalid);
        }
    }
}

fn current_jit_run_bb_hooks() -> Option<JitRunBbHooks> {
    JIT_RUN_BB_HOOK_STACK.with(|stack| stack.borrow().last().copied())
}

struct JitRunBbHooksGuard;

impl Drop for JitRunBbHooksGuard {
    fn drop(&mut self) {
        pop_jit_run_bb_hooks();
    }
}

impl TransformToMinAstError {
    pub(crate) fn to_py_err(self) -> PyErr {
        match self {
            TransformToMinAstError::Parse(msg) => PySyntaxError::new_err(msg),
            TransformToMinAstError::Lowering(msg) => {
                PyRuntimeError::new_err(format!("AST lowering failed: {msg}"))
            }
            TransformToMinAstError::MinAstConversion(msg) => {
                PyRuntimeError::new_err(format!("min_ast conversion failed: {msg}"))
            }
        }
    }
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn parse_and_lower(source: &str) -> Result<dp_transform::LoweringResult, TransformToMinAstError> {
    let options = Options {
        inject_import: true,
        eval_mode: true,
        lower_attributes: true,
        truthy: false,
        force_import_rewrite: true,
        ..Options::default()
    };

    match std::panic::catch_unwind(|| transform_str_to_ruff_with_options(source, options)) {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(TransformToMinAstError::Parse(err.to_string())),
        Err(payload) => Err(TransformToMinAstError::Lowering(panic_payload_to_string(
            payload,
        ))),
    }
}

pub(crate) fn transform_to_min_ast(
    source: &str,
) -> Result<EvalLoweringResult, TransformToMinAstError> {
    let lowered = parse_and_lower(source)?;
    let bb_module = lowered.bb_module.clone();
    let transformed_source = lowered.to_string();
    match std::panic::catch_unwind(|| lowered.into_min_ast()) {
        Ok(module) => Ok(EvalLoweringResult {
            min_ast_module: module,
            bb_module,
            transformed_source,
        }),
        Err(payload) => Err(TransformToMinAstError::MinAstConversion(
            panic_payload_to_string(payload),
        )),
    }
}

fn build_module_spec(
    py: Python<'_>,
    name: &str,
    path: &str,
    is_package: bool,
) -> PyResult<Py<PyAny>> {
    let importlib_util = py.import("importlib.util")?;
    let spec_from = importlib_util.getattr("spec_from_file_location")?;
    let spec = if is_package {
        let kwargs = PyDict::new(py);
        let dir = std::path::Path::new(path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        kwargs.set_item("submodule_search_locations", vec![dir])?;
        spec_from.call((name, path), Some(&kwargs))?
    } else {
        spec_from.call1((name, path))?
    };
    Ok(spec.unbind())
}

fn set_spec_initializing(spec: &Bound<'_, PyAny>, value: bool) {
    if spec.setattr("_initializing", value).is_err() {
        unsafe {
            ffi::PyErr_Clear();
        }
    }
}

fn collect_function_codes_from_code(
    code_obj: &Bound<'_, PyAny>,
    code_map: &Bound<'_, PyDict>,
    expected_names: &HashSet<String>,
) -> PyResult<()> {
    unsafe {
        if ffi::PyObject_TypeCheck(code_obj.as_ptr(), std::ptr::addr_of_mut!(ffi::PyCode_Type)) == 0
        {
            return Ok(());
        }
    }

    let name_obj = code_obj.getattr("co_name")?;
    if let Ok(name) = name_obj.extract::<String>() {
        if expected_names.contains(name.as_str()) {
            code_map.set_item(name_obj, code_obj)?;
        }
    }

    let consts = code_obj.getattr("co_consts")?;
    for item in consts.try_iter()? {
        collect_function_codes_from_code(&item?, code_map, expected_names)?;
    }
    Ok(())
}

fn collect_min_ast_function_names(stmts: &[min_ast::StmtNode], out: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            min_ast::StmtNode::FunctionDef(def) => {
                out.insert(def.name.clone());
                collect_min_ast_function_names(&def.body, out);
            }
            min_ast::StmtNode::While { body, orelse, .. }
            | min_ast::StmtNode::If { body, orelse, .. } => {
                collect_min_ast_function_names(body, out);
                collect_min_ast_function_names(orelse, out);
            }
            min_ast::StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
                ..
            } => {
                collect_min_ast_function_names(body, out);
                if let Some(handler) = handler {
                    collect_min_ast_function_names(handler, out);
                }
                collect_min_ast_function_names(orelse, out);
                collect_min_ast_function_names(finalbody, out);
            }
            _ => {}
        }
    }
}

fn collect_bb_function_names(bb_module: &bb_ir::BbModule, out: &mut HashSet<String>) {
    for function in &bb_module.functions {
        out.insert(function.bind_name.clone());
        out.insert(function.entry.clone());
        for block in &function.blocks {
            out.insert(block.label.clone());
        }
    }
    if let Some(module_init) = bb_module.module_init.as_ref() {
        out.insert(module_init.clone());
    }
}

fn expected_function_code_names(
    module_ast: &min_ast::Module,
    bb_module: Option<&bb_ir::BbModule>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_min_ast_function_names(&module_ast.body, &mut names);
    if let Some(bb_module) = bb_module {
        collect_bb_function_names(bb_module, &mut names);
    }
    names
}

fn compile_transformed_function_code_map(
    py: Python<'_>,
    path: &str,
    transformed_source: &str,
    expected_names: &HashSet<String>,
) -> PyResult<Py<PyDict>> {
    let builtins = py.import("builtins")?;
    let module_code = builtins
        .getattr("compile")?
        .call1((transformed_source, path, "exec"))?;
    let code_map = PyDict::new(py);
    collect_function_codes_from_code(&module_code, &code_map, expected_names)?;
    Ok(code_map.unbind())
}

pub(crate) fn eval_source_impl(py: Python<'_>, path: &str, source: &str) -> PyResult<Py<PyAny>> {
    eval_source_impl_with_name(py, path, source, "eval_source", None)
}

fn jit_mode_enabled() -> bool {
    std::env::var_os("DIET_PYTHON_JIT").as_deref() == Some("1".as_ref())
}

fn validate_bb_module_for_jit(bb_module: Option<&bb_ir::BbModule>) -> Result<(), String> {
    let bb_module = bb_module.ok_or_else(|| {
        "JIT mode requires emitted basic-block IR, but none was produced".to_string()
    })?;
    for function in &bb_module.functions {
        match &function.kind {
            bb_ir::BbFunctionKind::Function | bb_ir::BbFunctionKind::Generator { .. } => {}
            bb_ir::BbFunctionKind::Coroutine => {
                return Err(format!(
                    "JIT mode does not support coroutine functions yet: {}",
                    function.qualname
                ));
            }
            bb_ir::BbFunctionKind::AsyncGenerator { .. } => {
                return Err(format!(
                    "JIT mode does not support async generator functions yet: {}",
                    function.qualname
                ));
            }
        }
        for block in &function.blocks {
            if matches!(block.term, bb_ir::BbTerm::TryJump { .. }) {
                return Err(format!(
                    "JIT mode does not support try_jump terminators yet: {}:{}",
                    function.qualname, block.label
                ));
            }
        }
    }
    Ok(())
}

fn run_cranelift_jit_preflight(bb_module: Option<&bb_ir::BbModule>) -> Result<(), String> {
    let bb_module = bb_module.ok_or_else(|| {
        "JIT mode requires emitted basic-block IR, but none was produced".to_string()
    })?;
    let normalized = normalize_bb_module_for_codegen(bb_module);
    soac_eval::jit::run_cranelift_smoke(&normalized)
}

fn run_cranelift_python_call_preflight(py: Python<'_>) -> Result<(), String> {
    unsafe extern "C" fn preflight_incref(obj: *mut c_void) {
        if !obj.is_null() {
            ffi::Py_INCREF(obj as *mut ffi::PyObject);
        }
    }

    unsafe extern "C" fn preflight_decref(obj: *mut c_void) {
        if !obj.is_null() {
            ffi::Py_DECREF(obj as *mut ffi::PyObject);
        }
    }

    unsafe extern "C" fn preflight_call_one_arg(
        callable: *mut c_void,
        arg: *mut c_void,
    ) -> *mut c_void {
        ffi::PyObject_CallFunctionObjArgs(
            callable as *mut ffi::PyObject,
            arg as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        ) as *mut c_void
    }

    unsafe extern "C" fn preflight_compare_eq(lhs: *mut c_void, rhs: *mut c_void) -> i32 {
        ffi::PyObject_RichCompareBool(
            lhs as *mut ffi::PyObject,
            rhs as *mut ffi::PyObject,
            ffi::Py_EQ,
        )
    }

    // Execute one real Python call through JITed machine code, including
    // imported INCREF/DECREF and Python-call helper symbols.
    let builtins = py
        .import("builtins")
        .map_err(|err| format!("failed to import builtins for JIT preflight: {err}"))?;
    let len_fn = builtins
        .getattr("len")
        .map_err(|err| format!("failed to resolve builtins.len for JIT preflight: {err}"))?;
    let arg = PyList::new(py, [1_i64, 2, 3])
        .map_err(|err| format!("failed to build list arg for JIT preflight: {err}"))?;
    let expected = 3_i64.into_pyobject(py).map_err(|err| {
        format!("failed to build expected result object for JIT preflight: {err}")
    })?;
    unsafe {
        soac_eval::jit::run_cranelift_python_call_smoke(
            len_fn.as_ptr() as *mut c_void,
            arg.as_ptr() as *mut c_void,
            expected.as_ptr() as *mut c_void,
            preflight_incref,
            preflight_decref,
            preflight_call_one_arg,
            preflight_compare_eq,
        )?;
    }
    Ok(())
}

pub(crate) fn jit_run_bb_impl(
    py: Python<'_>,
    entry: &Bound<'_, PyAny>,
    args: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    unsafe extern "C" fn preflight_incref(obj: *mut c_void) {
        if !obj.is_null() {
            ffi::Py_INCREF(obj as *mut ffi::PyObject);
        }
    }

    unsafe extern "C" fn preflight_decref(obj: *mut c_void) {
        if !obj.is_null() {
            ffi::Py_DECREF(obj as *mut ffi::PyObject);
        }
    }

    unsafe extern "C" fn preflight_call_two_args(
        callable: *mut c_void,
        arg1: *mut c_void,
        arg2: *mut c_void,
    ) -> *mut c_void {
        ffi::PyObject_CallFunctionObjArgs(
            callable as *mut ffi::PyObject,
            arg1 as *mut ffi::PyObject,
            arg2 as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        ) as *mut c_void
    }

    unsafe extern "C" fn run_bb_step_hook(block: *mut c_void, args: *mut c_void) -> *mut c_void {
        let Some(hooks) = current_jit_run_bb_hooks() else {
            return std::ptr::null_mut();
        };
        ffi::PyObject_CallFunctionObjArgs(
            hooks.run_bb_step,
            block as *mut ffi::PyObject,
            args as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        ) as *mut c_void
    }

    unsafe extern "C" fn term_kind_hook(term: *mut c_void) -> i64 {
        let Some(hooks) = current_jit_run_bb_hooks() else {
            return -1;
        };
        let result = ffi::PyObject_CallFunctionObjArgs(
            hooks.term_kind,
            term as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        );
        if result.is_null() {
            return -1;
        }
        let value = ffi::PyLong_AsLongLong(result);
        ffi::Py_DECREF(result);
        value as i64
    }

    unsafe extern "C" fn term_jump_target_hook(term: *mut c_void) -> *mut c_void {
        let Some(hooks) = current_jit_run_bb_hooks() else {
            return std::ptr::null_mut();
        };
        ffi::PyObject_CallFunctionObjArgs(
            hooks.term_jump_target,
            term as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        ) as *mut c_void
    }

    unsafe extern "C" fn term_jump_args_hook(term: *mut c_void) -> *mut c_void {
        let Some(hooks) = current_jit_run_bb_hooks() else {
            return std::ptr::null_mut();
        };
        ffi::PyObject_CallFunctionObjArgs(
            hooks.term_jump_args,
            term as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        ) as *mut c_void
    }

    unsafe extern "C" fn term_ret_value_hook(term: *mut c_void) -> *mut c_void {
        let Some(hooks) = current_jit_run_bb_hooks() else {
            return std::ptr::null_mut();
        };
        ffi::PyObject_CallFunctionObjArgs(
            hooks.term_ret_value,
            term as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        ) as *mut c_void
    }

    unsafe extern "C" fn term_raise_hook(term: *mut c_void) -> i32 {
        let Some(hooks) = current_jit_run_bb_hooks() else {
            return -1;
        };
        let result = ffi::PyObject_CallFunctionObjArgs(
            hooks.term_raise,
            term as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        );
        if result.is_null() {
            return -1;
        }
        ffi::Py_DECREF(result);
        0
    }

    unsafe extern "C" fn term_invalid_hook(term: *mut c_void) -> i32 {
        let Some(hooks) = current_jit_run_bb_hooks() else {
            return -1;
        };
        let result = ffi::PyObject_CallFunctionObjArgs(
            hooks.term_invalid,
            term as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        );
        if result.is_null() {
            return -1;
        }
        ffi::Py_DECREF(result);
        0
    }

    let dp_module = py.import("__dp__")?;
    let run_bb_step = dp_module.getattr("_run_bb_step")?;
    let term_kind = dp_module.getattr("_bb_term_kind")?;
    let term_jump_target = dp_module.getattr("_bb_term_jump_target")?;
    let term_jump_args = dp_module.getattr("_bb_term_jump_args")?;
    let term_ret_value = dp_module.getattr("_bb_term_ret_value")?;
    let term_raise = dp_module.getattr("_bb_term_raise")?;
    let term_invalid = dp_module.getattr("_bb_term_invalid")?;
    let resolve_blocks = dp_module.getattr("_bb_resolve_blocks")?;

    push_jit_run_bb_hooks(JitRunBbHooks {
        run_bb_step: run_bb_step.as_ptr(),
        term_kind: term_kind.as_ptr(),
        term_jump_target: term_jump_target.as_ptr(),
        term_jump_args: term_jump_args.as_ptr(),
        term_ret_value: term_ret_value.as_ptr(),
        term_raise: term_raise.as_ptr(),
        term_invalid: term_invalid.as_ptr(),
    });
    let _hooks_guard = JitRunBbHooksGuard;

    let result_ptr = if let Some((entry_index, block_ptrs)) =
        resolve_specialized_jit_blocks(py, entry, &resolve_blocks, true)?
    {
        unsafe {
            soac_eval::jit::run_cranelift_run_bb_specialized(
                block_ptrs.as_slice(),
                entry_index,
                args.as_ptr() as *mut c_void,
                preflight_incref,
                preflight_decref,
                run_bb_step_hook,
                term_kind_hook,
                term_jump_target_hook,
                term_jump_args_hook,
                term_ret_value_hook,
                term_raise_hook,
                term_invalid_hook,
            )
            .map_err(PyRuntimeError::new_err)?
        }
    } else {
        unsafe {
            soac_eval::jit::run_cranelift_run_bb(
                entry.as_ptr() as *mut c_void,
                args.as_ptr() as *mut c_void,
                preflight_incref,
                preflight_decref,
                run_bb_step_hook,
                term_kind_hook,
                term_jump_target_hook,
                term_jump_args_hook,
                term_ret_value_hook,
                term_raise_hook,
            )
            .map_err(PyRuntimeError::new_err)?
        }
    };

    if result_ptr.is_null() {
        if unsafe { ffi::PyErr_Occurred() }.is_null() {
            return Err(PyRuntimeError::new_err("Cranelift JIT run_bb returned null result without exception"));
        }
        return Err(PyErr::fetch(py));
    }
    let result = unsafe { Bound::<PyAny>::from_owned_ptr(py, result_ptr as *mut ffi::PyObject) };
    Ok(result.unbind())
}

fn resolve_specialized_jit_blocks(
    py: Python<'_>,
    entry: &Bound<'_, PyAny>,
    resolve_blocks: &Bound<'_, PyAny>,
    allow_missing: bool,
) -> PyResult<Option<(usize, Vec<*mut c_void>)>> {
    let module_name = entry
        .getattr("__module__")
        .ok()
        .and_then(|obj| obj.extract::<String>().ok())
        .unwrap_or_default();
    let entry_label = entry
        .getattr("__name__")
        .ok()
        .and_then(|obj| obj.extract::<String>().ok())
        .unwrap_or_default();

    let mut plan = soac_eval::jit::lookup_bb_entry_plan(module_name.as_str(), entry_label.as_str());
    if plan.is_none() && !allow_missing {
        if let Ok(kwdefaults_obj) = entry.getattr("__kwdefaults__") {
            if let Ok(kwdefaults) = kwdefaults_obj.cast::<PyDict>() {
                if let Ok(Some(entry_bb_obj)) = kwdefaults.get_item("__dp_entry_bb") {
                    if let Ok(entry_bb_name) = entry_bb_obj.getattr("__name__").and_then(|o| o.extract::<String>()) {
                        plan = soac_eval::jit::lookup_bb_entry_plan(module_name.as_str(), entry_bb_name.as_str());
                    }
                }
            }
        }
    }
    let Some(plan) = plan else {
        return Ok(None);
    };
    let block_labels = PyTuple::new(py, plan.block_labels.iter().map(String::as_str))?;
    let resolved_blocks_obj = match resolve_blocks.call1((entry, block_labels)) {
        Ok(value) => value,
        Err(err) if allow_missing => {
            drop(err);
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    let resolved_blocks = resolved_blocks_obj
        .cast::<PyTuple>()
        .map_err(|_| PyRuntimeError::new_err("expected _bb_resolve_blocks() to return a tuple"))?;
    let mut block_ptrs = Vec::with_capacity(resolved_blocks.len());
    for item in resolved_blocks.iter() {
        block_ptrs.push(item.as_ptr() as *mut c_void);
    }
    if plan.entry_index >= block_ptrs.len() {
        if allow_missing {
            return Ok(None);
        }
        return Err(PyRuntimeError::new_err(format!(
            "invalid JIT entry index {} for {} blocks",
            plan.entry_index,
            block_ptrs.len()
        )));
    }
    Ok(Some((plan.entry_index, block_ptrs)))
}

pub(crate) fn jit_render_bb_impl(py: Python<'_>, entry: &Bound<'_, PyAny>) -> PyResult<String> {
    let dp_module = py.import("__dp__")?;
    let resolve_blocks = dp_module.getattr("_bb_resolve_blocks")?;
    let Some((entry_index, block_ptrs)) =
        resolve_specialized_jit_blocks(py, entry, &resolve_blocks, false)?
    else {
        let module_name = entry
            .getattr("__module__")
            .ok()
            .and_then(|obj| obj.extract::<String>().ok())
            .unwrap_or_default();
        let entry_label = entry
            .getattr("__name__")
            .ok()
            .and_then(|obj| obj.extract::<String>().ok())
            .unwrap_or_default();
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.{entry_label}"
        )));
    };
    unsafe {
        soac_eval::jit::render_cranelift_run_bb_specialized(block_ptrs.as_slice(), entry_index)
            .map_err(PyRuntimeError::new_err)
    }
}

fn eval_source_impl_with_name_and_spec(
    py: Python<'_>,
    path: &str,
    source: &str,
    name: &str,
    package: Option<&str>,
    spec_opt: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let lowering = transform_to_min_ast(source).map_err(TransformToMinAstError::to_py_err)?;
    let module_ast = lowering.min_ast_module;
    let bb_module = lowering.bb_module;
    let transformed_source = lowering.transformed_source;
    if jit_mode_enabled() {
        validate_bb_module_for_jit(bb_module.as_ref()).map_err(PyRuntimeError::new_err)?;
        if let Some(bb_module) = bb_module.as_ref() {
            let normalized = normalize_bb_module_for_codegen(bb_module);
            soac_eval::jit::register_bb_module_plans(name, &normalized).map_err(|err| {
                PyRuntimeError::new_err(format!("Cranelift JIT plan registration failed: {err}"))
            })?;
        }
        run_cranelift_jit_preflight(bb_module.as_ref()).map_err(|err| {
            PyRuntimeError::new_err(format!("Cranelift JIT preflight failed: {err}"))
        })?;
        run_cranelift_python_call_preflight(py).map_err(|err| {
            PyRuntimeError::new_err(format!("Cranelift JIT Python-call preflight failed: {err}"))
        })?;
    }

    unsafe {
        let module = PyModule::new(py, name)?;

        let layout = Box::new(interpreter::ScopeLayout::new(HashSet::new()));
        let layout_ptr = Box::into_raw(layout);
        let scope = Box::new(interpreter::ScopeInstance::new(layout_ptr));
        let scope_ptr = Box::into_raw(scope);

        module.setattr("__name__", name)?;

        if let Some(package) = package {
            module.setattr("__package__", package)?;
        }

        module.setattr("__file__", path)?;

        if let Some(min_ast::StmtNode::Expr { value, .. }) = module_ast.body.first() {
            if let min_ast::ExprNode::String { value, .. } = value {
                module.setattr("__doc__", value)?;
            } else {
                module.setattr("__doc__", py.None())?;
            }
        } else {
            module.setattr("__doc__", py.None())?;
        }

        let spec = if let Some(spec) = spec_opt {
            spec
        } else {
            let is_package = path.ends_with("__init__.py");
            build_module_spec(py, name, path, is_package)?
        };

        set_spec_initializing(spec.bind(py), true);
        let eval_result = (|| -> PyResult<()> {
            module.setattr("__spec__", spec.bind(py))?;
            match spec.bind(py).getattr("submodule_search_locations") {
                Ok(submodules) => {
                    if !submodules.is_none() {
                        module.setattr("__path__", submodules)?;
                    }
                }
                Err(_) => {
                    ffi::PyErr_Clear();
                }
            }

            let builtins = ffi::PyEval_GetBuiltins();
            if builtins.is_null() {
                return Err(PyErr::fetch(py));
            }
            let builtins_dict =
                Bound::<PyAny>::from_borrowed_ptr(py, builtins).cast_into::<PyDict>()?;
            module.setattr("__builtins__", &builtins_dict)?;

            let dp_module = py.import("__dp__")?;
            if jit_mode_enabled() {
                let runtime_module = py.import("diet_python")?;
                let jit_run_bb = runtime_module.getattr("jit_run_bb")?;
                let jit_render_bb = runtime_module.getattr("jit_render_bb")?;
                dp_module.setattr("_jit_run_bb", jit_run_bb)?;
                dp_module.setattr("_jit_render_bb", jit_render_bb)?;
            } else {
                dp_module.setattr("_jit_run_bb", py.None())?;
                dp_module.setattr("_jit_render_bb", py.None())?;
            }
            let runtime_fns = RuntimeFns::new(&builtins_dict, &dp_module.as_any())?;
            let expected_names = expected_function_code_names(&module_ast, bb_module.as_ref());
            let function_code_map = compile_transformed_function_code_map(
                py,
                path,
                transformed_source.as_str(),
                &expected_names,
            )?;

            let module_dict = module.dict();
            let name_cstr =
                CString::new(name).map_err(|_| PyRuntimeError::new_err("invalid __name__"))?;
            let modules = ffi::PyImport_GetModuleDict();
            if modules.is_null()
                || ffi::PyDict_SetItemString(modules, name_cstr.as_ptr(), module.as_any().as_ptr())
                    != 0
            {
                return Err(PyErr::fetch(py));
            }

            if interpreter::eval_module(
                &module_ast,
                scope_ptr,
                module_dict.as_ptr(),
                builtins,
                function_code_map.bind(py).as_ptr(),
                &runtime_fns,
            )
            .is_err()
            {
                ffi::PyDict_DelItemString(modules, name_cstr.as_ptr());
                return Err(PyErr::fetch(py));
            }

            if let Err(err) = module
                .getattr("_dp_module_init")
                .and_then(|init_fn| init_fn.call0())
            {
                ffi::PyDict_DelItemString(modules, name_cstr.as_ptr());
                return Err(err);
            }

            Ok(())
        })();

        set_spec_initializing(spec.bind(py), false);
        eval_result?;
        Ok(module.unbind().into_any())
    }
}

pub(crate) fn eval_source_impl_with_name(
    py: Python<'_>,
    path: &str,
    source: &str,
    name: &str,
    package: Option<&str>,
) -> PyResult<Py<PyAny>> {
    eval_source_impl_with_name_and_spec(py, path, source, name, package, None)
}

pub(crate) fn eval_source_impl_with_spec(
    py: Python<'_>,
    path: &str,
    source: &str,
    name: &str,
    package: Option<&str>,
    spec: Py<PyAny>,
) -> PyResult<Py<PyAny>> {
    eval_source_impl_with_name_and_spec(py, path, source, name, package, Some(spec))
}

#[cfg(test)]
mod tests {
    use super::{run_cranelift_jit_preflight, transform_to_min_ast, validate_bb_module_for_jit};
    use dp_transform::basic_block::bb_ir;

    #[test]
    fn jit_validator_accepts_nested_defs_and_generators() {
        let source = r#"
def outer(x):
    def inner(y):
        return y + 1
    def gen():
        yield inner(x)
    return list(gen())
"#;
        let bb_module = transform_to_min_ast(source)
            .expect("lowering should succeed")
            .bb_module;
        assert!(validate_bb_module_for_jit(bb_module.as_ref()).is_ok());
    }

    #[test]
    fn jit_validator_rejects_coroutines() {
        let source = r#"
async def run():
    return 1
"#;
        let bb_module = transform_to_min_ast(source)
            .expect("lowering should succeed")
            .bb_module;
        let err = validate_bb_module_for_jit(bb_module.as_ref()).expect_err("must reject");
        assert!(err.contains("coroutine"), "{err}");
    }

    #[test]
    fn jit_validator_rejects_async_generators() {
        let source = r#"
async def run():
    yield 1
"#;
        let bb_module = transform_to_min_ast(source)
            .expect("lowering should succeed")
            .bb_module;
        let err = validate_bb_module_for_jit(bb_module.as_ref()).expect_err("must reject");
        assert!(err.contains("async generator"), "{err}");
    }

    #[test]
    fn jit_validator_rejects_try_jump_terminators() {
        let source = r#"
def f():
    return 1
"#;
        let mut bb_module = transform_to_min_ast(source)
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
        let err = validate_bb_module_for_jit(Some(&bb_module)).expect_err("must reject");
        assert!(err.contains("try_jump"), "{err}");
    }

    #[test]
    fn jit_preflight_runs_cranelift_for_supported_module() {
        let source = r#"
def f(x):
    return x
"#;
        let bb_module = transform_to_min_ast(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref()).expect("validator should allow module");
        run_cranelift_jit_preflight(bb_module.as_ref()).expect("cranelift preflight should run");
    }
}

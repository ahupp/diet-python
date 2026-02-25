use cranelift_codegen::ir;
use cranelift_codegen::ir::InstBuilder;
use cranelift_codegen::settings;
use cranelift_codegen::settings::Configurable;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use dp_transform::basic_block::bb_ir::{BbModule, BbTerm};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::sync::{Mutex, OnceLock};

type ObjPtr = *mut c_void;
type IncrefFn = unsafe extern "C" fn(ObjPtr);
type DecrefFn = unsafe extern "C" fn(ObjPtr);
type CallOneArgFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
type CallTwoArgsFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
type CompareEqFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> i32;
type RunBbStepFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
type TermKindFn = unsafe extern "C" fn(ObjPtr) -> i64;
type TermJumpTargetFn = unsafe extern "C" fn(ObjPtr) -> ObjPtr;
type TermJumpArgsFn = unsafe extern "C" fn(ObjPtr) -> ObjPtr;
type TermRetValueFn = unsafe extern "C" fn(ObjPtr) -> ObjPtr;
type TermRaiseFn = unsafe extern "C" fn(ObjPtr) -> i32;
type TermInvalidFn = unsafe extern "C" fn(ObjPtr) -> i32;

static mut DP_JIT_INCREF_FN: Option<IncrefFn> = None;
static mut DP_JIT_DECREF_FN: Option<DecrefFn> = None;
static mut DP_JIT_CALL_ONE_ARG_FN: Option<CallOneArgFn> = None;
static mut DP_JIT_CALL_TWO_ARGS_FN: Option<CallTwoArgsFn> = None;
static mut DP_JIT_RUN_BB_STEP_FN: Option<RunBbStepFn> = None;
static mut DP_JIT_TERM_KIND_FN: Option<TermKindFn> = None;
static mut DP_JIT_TERM_JUMP_TARGET_FN: Option<TermJumpTargetFn> = None;
static mut DP_JIT_TERM_JUMP_ARGS_FN: Option<TermJumpArgsFn> = None;
static mut DP_JIT_TERM_RET_VALUE_FN: Option<TermRetValueFn> = None;
static mut DP_JIT_TERM_RAISE_FN: Option<TermRaiseFn> = None;
static mut DP_JIT_TERM_INVALID_FN: Option<TermInvalidFn> = None;

#[derive(Clone, Debug)]
pub struct EntryBlockPlan {
    pub entry_index: usize,
    pub block_labels: Vec<String>,
}

type ModulePlans = HashMap<String, EntryBlockPlan>;
type PlanRegistry = HashMap<String, ModulePlans>;

static BB_PLAN_REGISTRY: OnceLock<Mutex<PlanRegistry>> = OnceLock::new();

fn bb_plan_registry() -> &'static Mutex<PlanRegistry> {
    BB_PLAN_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_entry_plan(function: &dp_transform::basic_block::bb_ir::BbFunction) -> Result<EntryBlockPlan, String> {
    let mut label_to_index = HashMap::new();
    for (index, block) in function.blocks.iter().enumerate() {
        label_to_index.insert(block.label.clone(), index);
    }
    let Some(entry_index) = label_to_index.get(function.entry.as_str()).copied() else {
        return Err(format!(
            "missing entry label {} in function {}",
            function.entry, function.qualname
        ));
    };
    for block in &function.blocks {
        match &block.term {
            BbTerm::Jump(target) => {
                if !label_to_index.contains_key(target.as_str()) {
                    return Err(format!(
                        "unknown jump target {target} in {}:{}",
                        function.qualname, block.label
                    ));
                }
            }
            BbTerm::BrIf {
                then_label,
                else_label,
                ..
            } => {
                if !label_to_index.contains_key(then_label.as_str()) {
                    return Err(format!(
                        "unknown then target {then_label} in {}:{}",
                        function.qualname, block.label
                    ));
                }
                if !label_to_index.contains_key(else_label.as_str()) {
                    return Err(format!(
                        "unknown else target {else_label} in {}:{}",
                        function.qualname, block.label
                    ));
                }
            }
            BbTerm::BrTable {
                targets,
                default_label,
                ..
            } => {
                if !label_to_index.contains_key(default_label.as_str()) {
                    return Err(format!(
                        "unknown br_table default target {default_label} in {}:{}",
                        function.qualname, block.label
                    ));
                }
                for target in targets {
                    if !label_to_index.contains_key(target.as_str()) {
                        return Err(format!(
                            "unknown br_table target {target} in {}:{}",
                            function.qualname, block.label
                        ));
                    }
                }
            }
            BbTerm::Raise { .. } | BbTerm::Ret(_) => {}
            BbTerm::TryJump { .. } => {
                return Err(format!(
                    "unsupported try_jump in JIT entry plan for {}:{}",
                    function.qualname, block.label
                ));
            }
        }
    }
    Ok(EntryBlockPlan {
        entry_index,
        block_labels: function.blocks.iter().map(|block| block.label.clone()).collect(),
    })
}

pub fn register_bb_module_plans(module_name: &str, module: &BbModule) -> Result<(), String> {
    let mut plans = HashMap::new();
    for function in &module.functions {
        let plan = build_entry_plan(function)?;
        plans.insert(function.entry.clone(), plan);
    }
    let mut registry = bb_plan_registry()
        .lock()
        .map_err(|_| "failed to lock bb plan registry".to_string())?;
    registry.insert(module_name.to_string(), plans);
    Ok(())
}

pub fn lookup_bb_entry_plan(module_name: &str, entry_label: &str) -> Option<EntryBlockPlan> {
    let registry = bb_plan_registry().lock().ok()?;
    let module_plans = registry.get(module_name)?;
    module_plans.get(entry_label).cloned()
}

unsafe extern "C" fn dp_jit_incref(obj: ObjPtr) {
    if let Some(func) = DP_JIT_INCREF_FN {
        func(obj);
    }
}

unsafe extern "C" fn dp_jit_decref(obj: ObjPtr) {
    if let Some(func) = DP_JIT_DECREF_FN {
        func(obj);
    }
}

unsafe extern "C" fn dp_jit_call_one_arg(callable: ObjPtr, arg: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_ONE_ARG_FN {
        return func(callable, arg);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_call_two_args(callable: ObjPtr, arg1: ObjPtr, arg2: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_TWO_ARGS_FN {
        return func(callable, arg1, arg2);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_run_bb_step(block: ObjPtr, args: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_RUN_BB_STEP_FN {
        return func(block, args);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_term_kind(term: ObjPtr) -> i64 {
    if let Some(func) = DP_JIT_TERM_KIND_FN {
        return func(term);
    }
    -1
}

unsafe extern "C" fn dp_jit_term_jump_target(term: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_TERM_JUMP_TARGET_FN {
        return func(term);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_term_jump_args(term: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_TERM_JUMP_ARGS_FN {
        return func(term);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_term_ret_value(term: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_TERM_RET_VALUE_FN {
        return func(term);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_term_raise(term: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_TERM_RAISE_FN {
        return func(term);
    }
    -1
}

unsafe extern "C" fn dp_jit_term_invalid(term: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_TERM_INVALID_FN {
        return func(term);
    }
    -1
}

fn new_jit_builder() -> Result<JITBuilder, String> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("is_pic", "false")
        .map_err(|err| format!("failed to configure Cranelift flags: {err}"))?;
    let isa_builder =
        cranelift_codegen::isa::lookup_by_name("x86_64").map_err(|err| format!("{err}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|err| format!("failed to finish ISA: {err}"))?;
    Ok(JITBuilder::with_isa(
        isa,
        cranelift_module::default_libcall_names(),
    ))
}

fn new_jit_module() -> Result<JITModule, String> {
    Ok(JITModule::new(new_jit_builder()?))
}

pub fn run_cranelift_smoke(module: &BbModule) -> Result<(), String> {
    let function_count = module.functions.len() as i64;
    let block_count = module.functions.iter().map(|f| f.blocks.len() as i64).sum::<i64>();
    let sentinel = (function_count << 32) ^ block_count;

    let mut jit_module = new_jit_module()?;
    let mut ctx = jit_module.make_context();
    ctx.func.signature.returns.push(ir::AbiParam::new(ir::types::I64));
    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let value = builder.ins().iconst(ir::types::I64, sentinel);
        builder.ins().return_(&[value]);
        builder.finalize();
    }

    let function_id = jit_module
        .declare_function("dp_jit_smoke", Linkage::Local, &ctx.func.signature)
        .map_err(|err| format!("failed to declare Cranelift function: {err}"))?;
    jit_module
        .define_function(function_id, &mut ctx)
        .map_err(|err| format!("failed to define Cranelift function: {err}"))?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize Cranelift definitions: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(function_id);
    let compiled: extern "C" fn() -> i64 = unsafe { std::mem::transmute(code_ptr) };
    let got = compiled();
    if got != sentinel {
        return Err(format!(
            "Cranelift JIT smoke mismatch: expected {sentinel}, got {got}"
        ));
    }
    Ok(())
}

pub unsafe fn run_cranelift_python_call_smoke(
    callable: ObjPtr,
    arg: ObjPtr,
    expected: ObjPtr,
    incref_fn: IncrefFn,
    decref_fn: DecrefFn,
    call_one_arg_fn: CallOneArgFn,
    compare_eq_fn: CompareEqFn,
) -> Result<(), String> {
    if callable.is_null() || arg.is_null() || expected.is_null() {
        return Err("invalid null Python object pointer passed to JIT smoke call".to_string());
    }

    DP_JIT_INCREF_FN = Some(incref_fn);
    DP_JIT_DECREF_FN = Some(decref_fn);
    DP_JIT_CALL_ONE_ARG_FN = Some(call_one_arg_fn);

    let mut builder = new_jit_builder()?;
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol("dp_jit_call_one_arg", dp_jit_call_one_arg as *const u8);
    let mut jit_module = JITModule::new(builder);
    let ptr_ty = jit_module.target_config().pointer_type();

    let mut incref_sig = jit_module.make_signature();
    incref_sig.params.push(ir::AbiParam::new(ptr_ty));
    let mut decref_sig = jit_module.make_signature();
    decref_sig.params.push(ir::AbiParam::new(ptr_ty));
    let mut call_sig = jit_module.make_signature();
    call_sig.params.push(ir::AbiParam::new(ptr_ty));
    call_sig.params.push(ir::AbiParam::new(ptr_ty));
    call_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut main_sig = jit_module.make_signature();
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let incref_id = jit_module
        .declare_function("dp_jit_incref", Linkage::Import, &incref_sig)
        .map_err(|err| format!("failed to declare imported incref symbol: {err}"))?;
    let decref_id = jit_module
        .declare_function("dp_jit_decref", Linkage::Import, &decref_sig)
        .map_err(|err| format!("failed to declare imported decref symbol: {err}"))?;
    let call_id = jit_module
        .declare_function("dp_jit_call_one_arg", Linkage::Import, &call_sig)
        .map_err(|err| format!("failed to declare imported call symbol: {err}"))?;
    let main_id = jit_module
        .declare_function("dp_jit_call_smoke", Linkage::Local, &main_sig)
        .map_err(|err| format!("failed to declare jit call smoke function: {err}"))?;

    let mut ctx = jit_module.make_context();
    ctx.func.signature = main_sig.clone();
    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry = fb.create_block();
        fb.append_block_params_for_function_params(entry);
        fb.switch_to_block(entry);
        fb.seal_block(entry);

        let callable_val = fb.block_params(entry)[0];
        let arg_val = fb.block_params(entry)[1];

        let incref_ref = jit_module.declare_func_in_func(incref_id, &mut fb.func);
        let decref_ref = jit_module.declare_func_in_func(decref_id, &mut fb.func);
        let call_ref = jit_module.declare_func_in_func(call_id, &mut fb.func);

        fb.ins().call(incref_ref, &[callable_val]);
        fb.ins().call(incref_ref, &[arg_val]);
        let call_inst = fb.ins().call(call_ref, &[callable_val, arg_val]);
        let result_val = fb.inst_results(call_inst)[0];
        fb.ins().call(decref_ref, &[arg_val]);
        fb.ins().call(decref_ref, &[callable_val]);
        fb.ins().return_(&[result_val]);
        fb.finalize();
    }

    jit_module
        .define_function(main_id, &mut ctx)
        .map_err(|err| format!("failed to define jit call smoke function: {err}"))?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize jit call smoke function: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(main_id);
    let compiled: extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr =
        std::mem::transmute(code_ptr);
    let result = compiled(callable, arg);
    if result.is_null() {
        return Err("Cranelift Python-call smoke returned null result".to_string());
    }
    let matches = compare_eq_fn(result, expected);
    decref_fn(result);
    if matches < 0 {
        return Err("Cranelift Python-call smoke comparison raised Python exception".to_string());
    }
    if matches == 0 {
        return Err("Cranelift Python-call smoke returned unexpected value".to_string());
    }
    Ok(())
}

pub unsafe fn run_cranelift_python_call_two_args(
    callable: ObjPtr,
    arg1: ObjPtr,
    arg2: ObjPtr,
    incref_fn: IncrefFn,
    decref_fn: DecrefFn,
    call_two_args_fn: CallTwoArgsFn,
) -> Result<ObjPtr, String> {
    if callable.is_null() || arg1.is_null() || arg2.is_null() {
        return Err("invalid null Python object pointer passed to JIT two-arg call".to_string());
    }

    DP_JIT_INCREF_FN = Some(incref_fn);
    DP_JIT_DECREF_FN = Some(decref_fn);
    DP_JIT_CALL_TWO_ARGS_FN = Some(call_two_args_fn);

    let mut builder = new_jit_builder()?;
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol("dp_jit_call_two_args", dp_jit_call_two_args as *const u8);
    let mut jit_module = JITModule::new(builder);
    let ptr_ty = jit_module.target_config().pointer_type();

    let mut incref_sig = jit_module.make_signature();
    incref_sig.params.push(ir::AbiParam::new(ptr_ty));
    let mut decref_sig = jit_module.make_signature();
    decref_sig.params.push(ir::AbiParam::new(ptr_ty));
    let mut call_sig = jit_module.make_signature();
    call_sig.params.push(ir::AbiParam::new(ptr_ty));
    call_sig.params.push(ir::AbiParam::new(ptr_ty));
    call_sig.params.push(ir::AbiParam::new(ptr_ty));
    call_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut main_sig = jit_module.make_signature();
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let incref_id = jit_module
        .declare_function("dp_jit_incref", Linkage::Import, &incref_sig)
        .map_err(|err| format!("failed to declare imported incref symbol: {err}"))?;
    let decref_id = jit_module
        .declare_function("dp_jit_decref", Linkage::Import, &decref_sig)
        .map_err(|err| format!("failed to declare imported decref symbol: {err}"))?;
    let call_id = jit_module
        .declare_function("dp_jit_call_two_args", Linkage::Import, &call_sig)
        .map_err(|err| format!("failed to declare imported two-arg call symbol: {err}"))?;
    let main_id = jit_module
        .declare_function("dp_jit_call2", Linkage::Local, &main_sig)
        .map_err(|err| format!("failed to declare jit two-arg call function: {err}"))?;

    let mut ctx = jit_module.make_context();
    ctx.func.signature = main_sig;
    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry = fb.create_block();
        fb.append_block_params_for_function_params(entry);
        fb.switch_to_block(entry);
        fb.seal_block(entry);

        let callable_val = fb.block_params(entry)[0];
        let arg1_val = fb.block_params(entry)[1];
        let arg2_val = fb.block_params(entry)[2];

        let incref_ref = jit_module.declare_func_in_func(incref_id, &mut fb.func);
        let decref_ref = jit_module.declare_func_in_func(decref_id, &mut fb.func);
        let call_ref = jit_module.declare_func_in_func(call_id, &mut fb.func);

        fb.ins().call(incref_ref, &[callable_val]);
        fb.ins().call(incref_ref, &[arg1_val]);
        fb.ins().call(incref_ref, &[arg2_val]);
        let call_inst = fb.ins().call(call_ref, &[callable_val, arg1_val, arg2_val]);
        let result_val = fb.inst_results(call_inst)[0];
        fb.ins().call(decref_ref, &[arg2_val]);
        fb.ins().call(decref_ref, &[arg1_val]);
        fb.ins().call(decref_ref, &[callable_val]);
        fb.ins().return_(&[result_val]);
        fb.finalize();
    }

    jit_module
        .define_function(main_id, &mut ctx)
        .map_err(|err| format!("failed to define jit two-arg call function: {err}"))?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize jit two-arg call function: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(main_id);
    let compiled: extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr = std::mem::transmute(code_ptr);
    let result = compiled(callable, arg1, arg2);
    Ok(result)
}

pub unsafe fn run_cranelift_run_bb(
    entry: ObjPtr,
    args: ObjPtr,
    incref_fn: IncrefFn,
    decref_fn: DecrefFn,
    run_bb_step_fn: RunBbStepFn,
    term_kind_fn: TermKindFn,
    term_jump_target_fn: TermJumpTargetFn,
    term_jump_args_fn: TermJumpArgsFn,
    term_ret_value_fn: TermRetValueFn,
    term_raise_fn: TermRaiseFn,
) -> Result<ObjPtr, String> {
    if entry.is_null() || args.is_null() {
        return Err("invalid null block/args passed to JIT run_bb".to_string());
    }

    DP_JIT_INCREF_FN = Some(incref_fn);
    DP_JIT_DECREF_FN = Some(decref_fn);
    DP_JIT_RUN_BB_STEP_FN = Some(run_bb_step_fn);
    DP_JIT_TERM_KIND_FN = Some(term_kind_fn);
    DP_JIT_TERM_JUMP_TARGET_FN = Some(term_jump_target_fn);
    DP_JIT_TERM_JUMP_ARGS_FN = Some(term_jump_args_fn);
    DP_JIT_TERM_RET_VALUE_FN = Some(term_ret_value_fn);
    DP_JIT_TERM_RAISE_FN = Some(term_raise_fn);

    let mut builder = new_jit_builder()?;
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol("dp_jit_run_bb_step", dp_jit_run_bb_step as *const u8);
    builder.symbol("dp_jit_term_kind", dp_jit_term_kind as *const u8);
    builder.symbol("dp_jit_term_jump_target", dp_jit_term_jump_target as *const u8);
    builder.symbol("dp_jit_term_jump_args", dp_jit_term_jump_args as *const u8);
    builder.symbol("dp_jit_term_ret_value", dp_jit_term_ret_value as *const u8);
    builder.symbol("dp_jit_term_raise", dp_jit_term_raise as *const u8);
    let mut jit_module = JITModule::new(builder);
    let ptr_ty = jit_module.target_config().pointer_type();
    let i64_ty = ir::types::I64;
    let i32_ty = ir::types::I32;

    let mut incref_sig = jit_module.make_signature();
    incref_sig.params.push(ir::AbiParam::new(ptr_ty));
    let mut decref_sig = jit_module.make_signature();
    decref_sig.params.push(ir::AbiParam::new(ptr_ty));

    let mut step_sig = jit_module.make_signature();
    step_sig.params.push(ir::AbiParam::new(ptr_ty));
    step_sig.params.push(ir::AbiParam::new(ptr_ty));
    step_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut term_kind_sig = jit_module.make_signature();
    term_kind_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_kind_sig.returns.push(ir::AbiParam::new(i64_ty));

    let mut term_jump_target_sig = jit_module.make_signature();
    term_jump_target_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_jump_target_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut term_jump_args_sig = jit_module.make_signature();
    term_jump_args_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_jump_args_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut term_ret_value_sig = jit_module.make_signature();
    term_ret_value_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_ret_value_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut term_raise_sig = jit_module.make_signature();
    term_raise_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_raise_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut main_sig = jit_module.make_signature();
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let incref_id = jit_module
        .declare_function("dp_jit_incref", Linkage::Import, &incref_sig)
        .map_err(|err| format!("failed to declare imported incref symbol: {err}"))?;
    let decref_id = jit_module
        .declare_function("dp_jit_decref", Linkage::Import, &decref_sig)
        .map_err(|err| format!("failed to declare imported decref symbol: {err}"))?;
    let step_id = jit_module
        .declare_function("dp_jit_run_bb_step", Linkage::Import, &step_sig)
        .map_err(|err| format!("failed to declare imported run_bb_step symbol: {err}"))?;
    let term_kind_id = jit_module
        .declare_function("dp_jit_term_kind", Linkage::Import, &term_kind_sig)
        .map_err(|err| format!("failed to declare imported term_kind symbol: {err}"))?;
    let term_jump_target_id = jit_module
        .declare_function("dp_jit_term_jump_target", Linkage::Import, &term_jump_target_sig)
        .map_err(|err| format!("failed to declare imported term_jump_target symbol: {err}"))?;
    let term_jump_args_id = jit_module
        .declare_function("dp_jit_term_jump_args", Linkage::Import, &term_jump_args_sig)
        .map_err(|err| format!("failed to declare imported term_jump_args symbol: {err}"))?;
    let term_ret_value_id = jit_module
        .declare_function("dp_jit_term_ret_value", Linkage::Import, &term_ret_value_sig)
        .map_err(|err| format!("failed to declare imported term_ret_value symbol: {err}"))?;
    let term_raise_id = jit_module
        .declare_function("dp_jit_term_raise", Linkage::Import, &term_raise_sig)
        .map_err(|err| format!("failed to declare imported term_raise symbol: {err}"))?;
    let main_id = jit_module
        .declare_function("dp_jit_run_bb", Linkage::Local, &main_sig)
        .map_err(|err| format!("failed to declare jit run_bb function: {err}"))?;

    let mut ctx = jit_module.make_context();
    ctx.func.signature = main_sig;
    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry_block = fb.create_block();
        let loop_block = fb.create_block();
        let step_null_block = fb.create_block();
        let dispatch_block = fb.create_block();
        let jump_block = fb.create_block();
        let jump_term_cleanup_block = fb.create_block();
        let jump_cleanup_block = fb.create_block();
        let ret_block = fb.create_block();
        let raise_block = fb.create_block();
        let invalid_block = fb.create_block();

        fb.append_block_params_for_function_params(entry_block);
        fb.append_block_param(loop_block, ptr_ty); // block
        fb.append_block_param(loop_block, ptr_ty); // args
        fb.append_block_param(dispatch_block, ptr_ty); // block
        fb.append_block_param(dispatch_block, ptr_ty); // args
        fb.append_block_param(dispatch_block, ptr_ty); // term
        fb.append_block_param(jump_block, ptr_ty); // block
        fb.append_block_param(jump_block, ptr_ty); // args
        fb.append_block_param(jump_block, ptr_ty); // term
        fb.append_block_param(jump_term_cleanup_block, ptr_ty); // block
        fb.append_block_param(jump_term_cleanup_block, ptr_ty); // args
        fb.append_block_param(jump_term_cleanup_block, ptr_ty); // term
        fb.append_block_param(jump_cleanup_block, ptr_ty); // block
        fb.append_block_param(jump_cleanup_block, ptr_ty); // args
        fb.append_block_param(jump_cleanup_block, ptr_ty); // term
        fb.append_block_param(jump_cleanup_block, ptr_ty); // next_block
        fb.append_block_param(jump_cleanup_block, ptr_ty); // next_args
        fb.append_block_param(ret_block, ptr_ty); // block
        fb.append_block_param(ret_block, ptr_ty); // args
        fb.append_block_param(ret_block, ptr_ty); // term
        fb.append_block_param(raise_block, ptr_ty); // block
        fb.append_block_param(raise_block, ptr_ty); // args
        fb.append_block_param(raise_block, ptr_ty); // term
        fb.append_block_param(invalid_block, ptr_ty); // block
        fb.append_block_param(invalid_block, ptr_ty); // args
        fb.append_block_param(invalid_block, ptr_ty); // term
        fb.append_block_param(step_null_block, ptr_ty); // block
        fb.append_block_param(step_null_block, ptr_ty); // args

        fb.switch_to_block(entry_block);
        let entry_val = fb.block_params(entry_block)[0];
        let args_val = fb.block_params(entry_block)[1];

        let incref_ref = jit_module.declare_func_in_func(incref_id, &mut fb.func);
        let decref_ref = jit_module.declare_func_in_func(decref_id, &mut fb.func);
        let step_ref = jit_module.declare_func_in_func(step_id, &mut fb.func);
        let term_kind_ref = jit_module.declare_func_in_func(term_kind_id, &mut fb.func);
        let term_jump_target_ref =
            jit_module.declare_func_in_func(term_jump_target_id, &mut fb.func);
        let term_jump_args_ref = jit_module.declare_func_in_func(term_jump_args_id, &mut fb.func);
        let term_ret_value_ref = jit_module.declare_func_in_func(term_ret_value_id, &mut fb.func);
        let term_raise_ref = jit_module.declare_func_in_func(term_raise_id, &mut fb.func);

        fb.ins().call(incref_ref, &[entry_val]);
        fb.ins().call(incref_ref, &[args_val]);
        let entry_jump_args = [ir::BlockArg::Value(entry_val), ir::BlockArg::Value(args_val)];
        fb.ins().jump(loop_block, &entry_jump_args);

        fb.switch_to_block(loop_block);
        let block_val = fb.block_params(loop_block)[0];
        let block_args_val = fb.block_params(loop_block)[1];
        let step_inst = fb.ins().call(step_ref, &[block_val, block_args_val]);
        let term_val = fb.inst_results(step_inst)[0];
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        let is_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, term_val, null_ptr);
        let step_null_args = [ir::BlockArg::Value(block_val), ir::BlockArg::Value(block_args_val)];
        let dispatch_args = [
            ir::BlockArg::Value(block_val),
            ir::BlockArg::Value(block_args_val),
            ir::BlockArg::Value(term_val),
        ];
        fb.ins()
            .brif(is_null, step_null_block, &step_null_args, dispatch_block, &dispatch_args);

        fb.switch_to_block(step_null_block);
        let step_block_val = fb.block_params(step_null_block)[0];
        let step_args_val = fb.block_params(step_null_block)[1];
        fb.ins().call(decref_ref, &[step_args_val]);
        fb.ins().call(decref_ref, &[step_block_val]);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(dispatch_block);
        let dispatch_block_val = fb.block_params(dispatch_block)[0];
        let dispatch_args_val = fb.block_params(dispatch_block)[1];
        let dispatch_term_val = fb.block_params(dispatch_block)[2];
        let kind_inst = fb.ins().call(term_kind_ref, &[dispatch_term_val]);
        let kind_val = fb.inst_results(kind_inst)[0];
        let jump_kind = fb.ins().iconst(i64_ty, 0);
        let ret_kind = fb.ins().iconst(i64_ty, 1);
        let raise_kind = fb.ins().iconst(i64_ty, 2);
        let is_jump = fb.ins().icmp(ir::condcodes::IntCC::Equal, kind_val, jump_kind);
        let after_jump_check = fb.create_block();
        fb.append_block_param(after_jump_check, ptr_ty);
        fb.append_block_param(after_jump_check, ptr_ty);
        fb.append_block_param(after_jump_check, ptr_ty);
        let dispatch_triplet = [
            ir::BlockArg::Value(dispatch_block_val),
            ir::BlockArg::Value(dispatch_args_val),
            ir::BlockArg::Value(dispatch_term_val),
        ];
        fb.ins().brif(
            is_jump,
            jump_block,
            &dispatch_triplet,
            after_jump_check,
            &dispatch_triplet,
        );
        fb.switch_to_block(after_jump_check);
        let aj_block = fb.block_params(after_jump_check)[0];
        let aj_args = fb.block_params(after_jump_check)[1];
        let aj_term = fb.block_params(after_jump_check)[2];
        let is_ret = fb.ins().icmp(ir::condcodes::IntCC::Equal, kind_val, ret_kind);
        let after_ret_check = fb.create_block();
        fb.append_block_param(after_ret_check, ptr_ty);
        fb.append_block_param(after_ret_check, ptr_ty);
        fb.append_block_param(after_ret_check, ptr_ty);
        let aj_triplet = [
            ir::BlockArg::Value(aj_block),
            ir::BlockArg::Value(aj_args),
            ir::BlockArg::Value(aj_term),
        ];
        fb.ins().brif(
            is_ret,
            ret_block,
            &aj_triplet,
            after_ret_check,
            &aj_triplet,
        );
        fb.switch_to_block(after_ret_check);
        let ar_block = fb.block_params(after_ret_check)[0];
        let ar_args = fb.block_params(after_ret_check)[1];
        let ar_term = fb.block_params(after_ret_check)[2];
        let is_raise = fb.ins().icmp(ir::condcodes::IntCC::Equal, kind_val, raise_kind);
        let ar_triplet = [
            ir::BlockArg::Value(ar_block),
            ir::BlockArg::Value(ar_args),
            ir::BlockArg::Value(ar_term),
        ];
        fb.ins().brif(
            is_raise,
            raise_block,
            &ar_triplet,
            invalid_block,
            &ar_triplet,
        );

        fb.switch_to_block(jump_block);
        let jump_block_val = fb.block_params(jump_block)[0];
        let jump_args_val = fb.block_params(jump_block)[1];
        let jump_term_val = fb.block_params(jump_block)[2];
        let next_block_inst = fb.ins().call(term_jump_target_ref, &[jump_term_val]);
        let next_block_val = fb.inst_results(next_block_inst)[0];
        let next_args_inst = fb.ins().call(term_jump_args_ref, &[jump_term_val]);
        let next_args_val = fb.inst_results(next_args_inst)[0];
        let next_block_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, next_block_val, null_ptr);
        let jump_cleanup_short = [
            ir::BlockArg::Value(jump_block_val),
            ir::BlockArg::Value(jump_args_val),
            ir::BlockArg::Value(jump_term_val),
        ];
        let jump_cleanup_long = [
            ir::BlockArg::Value(jump_block_val),
            ir::BlockArg::Value(jump_args_val),
            ir::BlockArg::Value(jump_term_val),
            ir::BlockArg::Value(next_block_val),
            ir::BlockArg::Value(next_args_val),
        ];
        fb.ins().brif(
            next_block_null,
            jump_term_cleanup_block,
            &jump_cleanup_short,
            jump_cleanup_block,
            &jump_cleanup_long,
        );

        fb.switch_to_block(jump_term_cleanup_block);
        let jtc_block = fb.block_params(jump_term_cleanup_block)[0];
        let jtc_args = fb.block_params(jump_term_cleanup_block)[1];
        let jtc_term = fb.block_params(jump_term_cleanup_block)[2];
        fb.ins().call(decref_ref, &[jtc_term]);
        fb.ins().call(decref_ref, &[jtc_args]);
        fb.ins().call(decref_ref, &[jtc_block]);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(jump_cleanup_block);
        let jc_block = fb.block_params(jump_cleanup_block)[0];
        let jc_args = fb.block_params(jump_cleanup_block)[1];
        let jc_term = fb.block_params(jump_cleanup_block)[2];
        let jc_next_block = fb.block_params(jump_cleanup_block)[3];
        let jc_next_args = fb.block_params(jump_cleanup_block)[4];
        let next_args_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, jc_next_args, null_ptr);
        let jump_ok_block = fb.create_block();
        fb.append_block_param(jump_ok_block, ptr_ty);
        fb.append_block_param(jump_ok_block, ptr_ty);
        fb.append_block_param(jump_ok_block, ptr_ty);
        fb.append_block_param(jump_ok_block, ptr_ty);
        fb.append_block_param(jump_ok_block, ptr_ty);
        let jc_short = [
            ir::BlockArg::Value(jc_block),
            ir::BlockArg::Value(jc_args),
            ir::BlockArg::Value(jc_term),
        ];
        let jc_long = [
            ir::BlockArg::Value(jc_block),
            ir::BlockArg::Value(jc_args),
            ir::BlockArg::Value(jc_term),
            ir::BlockArg::Value(jc_next_block),
            ir::BlockArg::Value(jc_next_args),
        ];
        fb.ins().brif(
            next_args_null,
            jump_term_cleanup_block,
            &jc_short,
            jump_ok_block,
            &jc_long,
        );
        fb.switch_to_block(jump_ok_block);
        let jo_block = fb.block_params(jump_ok_block)[0];
        let jo_args = fb.block_params(jump_ok_block)[1];
        let jo_term = fb.block_params(jump_ok_block)[2];
        let jo_next_block = fb.block_params(jump_ok_block)[3];
        let jo_next_args = fb.block_params(jump_ok_block)[4];
        fb.ins().call(decref_ref, &[jo_term]);
        fb.ins().call(decref_ref, &[jo_args]);
        fb.ins().call(decref_ref, &[jo_block]);
        let loop_back_args = [ir::BlockArg::Value(jo_next_block), ir::BlockArg::Value(jo_next_args)];
        fb.ins().jump(loop_block, &loop_back_args);

        fb.switch_to_block(ret_block);
        let ret_block_val = fb.block_params(ret_block)[0];
        let ret_args_val = fb.block_params(ret_block)[1];
        let ret_term_val = fb.block_params(ret_block)[2];
        let ret_val_inst = fb.ins().call(term_ret_value_ref, &[ret_term_val]);
        let ret_value = fb.inst_results(ret_val_inst)[0];
        fb.ins().call(decref_ref, &[ret_term_val]);
        fb.ins().call(decref_ref, &[ret_args_val]);
        fb.ins().call(decref_ref, &[ret_block_val]);
        fb.ins().return_(&[ret_value]);

        fb.switch_to_block(raise_block);
        let raise_block_val = fb.block_params(raise_block)[0];
        let raise_args_val = fb.block_params(raise_block)[1];
        let raise_term_val = fb.block_params(raise_block)[2];
        let _ = fb.ins().call(term_raise_ref, &[raise_term_val]);
        fb.ins().call(decref_ref, &[raise_term_val]);
        fb.ins().call(decref_ref, &[raise_args_val]);
        fb.ins().call(decref_ref, &[raise_block_val]);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(invalid_block);
        let invalid_block_val = fb.block_params(invalid_block)[0];
        let invalid_args_val = fb.block_params(invalid_block)[1];
        let invalid_term_val = fb.block_params(invalid_block)[2];
        let _ = fb.ins().call(term_raise_ref, &[invalid_term_val]);
        fb.ins().call(decref_ref, &[invalid_term_val]);
        fb.ins().call(decref_ref, &[invalid_args_val]);
        fb.ins().call(decref_ref, &[invalid_block_val]);
        fb.ins().return_(&[null_ptr]);
        fb.seal_all_blocks();

        fb.finalize();
    }

    jit_module
        .define_function(main_id, &mut ctx)
        .map_err(|err| format!("failed to define jit run_bb function: {err}"))?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize jit run_bb function: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(main_id);
    let compiled: extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr = std::mem::transmute(code_ptr);
    Ok(compiled(entry, args))
}

fn build_cranelift_run_bb_specialized_function(
    jit_module: &mut JITModule,
    blocks: &[ObjPtr],
    entry_index: usize,
) -> Result<(cranelift_codegen::Context, cranelift_module::FuncId), String> {
    let ptr_ty = jit_module.target_config().pointer_type();
    let i64_ty = ir::types::I64;
    let i32_ty = ir::types::I32;

    let mut incref_sig = jit_module.make_signature();
    incref_sig.params.push(ir::AbiParam::new(ptr_ty));
    let mut decref_sig = jit_module.make_signature();
    decref_sig.params.push(ir::AbiParam::new(ptr_ty));

    let mut step_sig = jit_module.make_signature();
    step_sig.params.push(ir::AbiParam::new(ptr_ty));
    step_sig.params.push(ir::AbiParam::new(ptr_ty));
    step_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut term_kind_sig = jit_module.make_signature();
    term_kind_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_kind_sig.returns.push(ir::AbiParam::new(i64_ty));

    let mut term_jump_target_sig = jit_module.make_signature();
    term_jump_target_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_jump_target_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut term_jump_args_sig = jit_module.make_signature();
    term_jump_args_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_jump_args_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut term_ret_value_sig = jit_module.make_signature();
    term_ret_value_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_ret_value_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut term_raise_sig = jit_module.make_signature();
    term_raise_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_raise_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut term_invalid_sig = jit_module.make_signature();
    term_invalid_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_invalid_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut main_sig = jit_module.make_signature();
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let incref_id = jit_module
        .declare_function("dp_jit_incref", Linkage::Import, &incref_sig)
        .map_err(|err| format!("failed to declare imported incref symbol: {err}"))?;
    let decref_id = jit_module
        .declare_function("dp_jit_decref", Linkage::Import, &decref_sig)
        .map_err(|err| format!("failed to declare imported decref symbol: {err}"))?;
    let step_id = jit_module
        .declare_function("dp_jit_run_bb_step", Linkage::Import, &step_sig)
        .map_err(|err| format!("failed to declare imported run_bb_step symbol: {err}"))?;
    let term_kind_id = jit_module
        .declare_function("dp_jit_term_kind", Linkage::Import, &term_kind_sig)
        .map_err(|err| format!("failed to declare imported term_kind symbol: {err}"))?;
    let term_jump_target_id = jit_module
        .declare_function("dp_jit_term_jump_target", Linkage::Import, &term_jump_target_sig)
        .map_err(|err| format!("failed to declare imported term_jump_target symbol: {err}"))?;
    let term_jump_args_id = jit_module
        .declare_function("dp_jit_term_jump_args", Linkage::Import, &term_jump_args_sig)
        .map_err(|err| format!("failed to declare imported term_jump_args symbol: {err}"))?;
    let term_ret_value_id = jit_module
        .declare_function("dp_jit_term_ret_value", Linkage::Import, &term_ret_value_sig)
        .map_err(|err| format!("failed to declare imported term_ret_value symbol: {err}"))?;
    let term_raise_id = jit_module
        .declare_function("dp_jit_term_raise", Linkage::Import, &term_raise_sig)
        .map_err(|err| format!("failed to declare imported term_raise symbol: {err}"))?;
    let term_invalid_id = jit_module
        .declare_function("dp_jit_term_invalid", Linkage::Import, &term_invalid_sig)
        .map_err(|err| format!("failed to declare imported term_invalid symbol: {err}"))?;
    let main_id = jit_module
        .declare_function("dp_jit_run_bb_specialized", Linkage::Local, &main_sig)
        .map_err(|err| format!("failed to declare specialized jit run_bb function: {err}"))?;

    let mut ctx = jit_module.make_context();
    ctx.func.signature = main_sig;
    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry_block = fb.create_block();
        let dispatch_block = fb.create_block();
        let mut dispatch_match_blocks = Vec::with_capacity(blocks.len());
        let invalid_pc_block = fb.create_block();
        let step_null_block = fb.create_block();
        let term_dispatch_block = fb.create_block();
        let jump_block = fb.create_block();
        let ret_block = fb.create_block();
        let raise_block = fb.create_block();
        let invalid_term_block = fb.create_block();
        let jump_invalid_target_block = fb.create_block();

        let mut exec_blocks = Vec::with_capacity(blocks.len());
        let mut jump_match_blocks = Vec::with_capacity(blocks.len());
        for _ in 0..blocks.len() {
            dispatch_match_blocks.push(fb.create_block());
            exec_blocks.push(fb.create_block());
            jump_match_blocks.push(fb.create_block());
        }

        fb.append_block_params_for_function_params(entry_block);
        fb.append_block_param(dispatch_block, i64_ty); // block index
        fb.append_block_param(dispatch_block, ptr_ty); // args
        for block in &dispatch_match_blocks {
            fb.append_block_param(*block, i64_ty); // block index
            fb.append_block_param(*block, ptr_ty); // args
        }
        fb.append_block_param(invalid_pc_block, ptr_ty); // args
        fb.append_block_param(step_null_block, ptr_ty); // args
        fb.append_block_param(term_dispatch_block, ptr_ty); // args
        fb.append_block_param(term_dispatch_block, ptr_ty); // term
        fb.append_block_param(jump_block, ptr_ty); // args
        fb.append_block_param(jump_block, ptr_ty); // term
        fb.append_block_param(ret_block, ptr_ty); // args
        fb.append_block_param(ret_block, ptr_ty); // term
        fb.append_block_param(raise_block, ptr_ty); // args
        fb.append_block_param(raise_block, ptr_ty); // term
        fb.append_block_param(invalid_term_block, ptr_ty); // args
        fb.append_block_param(invalid_term_block, ptr_ty); // term
        fb.append_block_param(jump_invalid_target_block, ptr_ty); // args
        fb.append_block_param(jump_invalid_target_block, ptr_ty); // term
        fb.append_block_param(jump_invalid_target_block, ptr_ty); // next_args

        for block in &exec_blocks {
            fb.append_block_param(*block, ptr_ty); // args
        }
        for block in &jump_match_blocks {
            fb.append_block_param(*block, ptr_ty); // args
            fb.append_block_param(*block, ptr_ty); // term
            fb.append_block_param(*block, ptr_ty); // next_args
            fb.append_block_param(*block, ptr_ty); // target
        }

        fb.switch_to_block(entry_block);
        let entry_args = fb.block_params(entry_block)[0];
        let incref_ref = jit_module.declare_func_in_func(incref_id, &mut fb.func);
        let decref_ref = jit_module.declare_func_in_func(decref_id, &mut fb.func);
        let step_ref = jit_module.declare_func_in_func(step_id, &mut fb.func);
        let term_kind_ref = jit_module.declare_func_in_func(term_kind_id, &mut fb.func);
        let term_jump_target_ref =
            jit_module.declare_func_in_func(term_jump_target_id, &mut fb.func);
        let term_jump_args_ref = jit_module.declare_func_in_func(term_jump_args_id, &mut fb.func);
        let term_ret_value_ref = jit_module.declare_func_in_func(term_ret_value_id, &mut fb.func);
        let term_raise_ref = jit_module.declare_func_in_func(term_raise_id, &mut fb.func);
        let term_invalid_ref = jit_module.declare_func_in_func(term_invalid_id, &mut fb.func);

        fb.ins().call(incref_ref, &[entry_args]);
        let entry_idx = fb.ins().iconst(i64_ty, entry_index as i64);
        let entry_jump_args = [
            ir::BlockArg::Value(entry_idx),
            ir::BlockArg::Value(entry_args),
        ];
        fb.ins().jump(dispatch_block, &entry_jump_args);

        fb.switch_to_block(dispatch_block);
        let dispatch_idx = fb.block_params(dispatch_block)[0];
        let dispatch_args = fb.block_params(dispatch_block)[1];
        let dispatch_start_args = [
            ir::BlockArg::Value(dispatch_idx),
            ir::BlockArg::Value(dispatch_args),
        ];
        fb.ins()
            .jump(dispatch_match_blocks[0], &dispatch_start_args);

        for (index, dispatch_match) in dispatch_match_blocks.iter().enumerate() {
            fb.switch_to_block(*dispatch_match);
            let match_idx = fb.block_params(*dispatch_match)[0];
            let match_args = fb.block_params(*dispatch_match)[1];
            let expected_idx = fb.ins().iconst(i64_ty, index as i64);
            let is_match = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, match_idx, expected_idx);
            let exec_jump_args = [ir::BlockArg::Value(match_args)];
            if index + 1 < dispatch_match_blocks.len() {
                let next = dispatch_match_blocks[index + 1];
                let next_args = [ir::BlockArg::Value(match_idx), ir::BlockArg::Value(match_args)];
                fb.ins()
                    .brif(is_match, exec_blocks[index], &exec_jump_args, next, &next_args);
            } else {
                let invalid_args = [ir::BlockArg::Value(match_args)];
                fb.ins().brif(
                    is_match,
                    exec_blocks[index],
                    &exec_jump_args,
                    invalid_pc_block,
                    &invalid_args,
                );
            }
        }

        fb.switch_to_block(invalid_pc_block);
        let invalid_pc_args = fb.block_params(invalid_pc_block)[0];
        let _ = fb.ins().call(term_invalid_ref, &[invalid_pc_args]);
        fb.ins().call(decref_ref, &[invalid_pc_args]);
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        fb.ins().return_(&[null_ptr]);

        for (index, block) in exec_blocks.iter().enumerate() {
            fb.switch_to_block(*block);
            let exec_args = fb.block_params(*block)[0];
            let block_const = fb.ins().iconst(ptr_ty, blocks[index] as i64);
            let step_inst = fb.ins().call(step_ref, &[block_const, exec_args]);
            let term = fb.inst_results(step_inst)[0];
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let is_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, term, null_ptr);
            let step_null_args = [ir::BlockArg::Value(exec_args)];
            let term_dispatch_args = [ir::BlockArg::Value(exec_args), ir::BlockArg::Value(term)];
            fb.ins().brif(
                is_null,
                step_null_block,
                &step_null_args,
                term_dispatch_block,
                &term_dispatch_args,
            );
        }

        fb.switch_to_block(step_null_block);
        let step_null_args = fb.block_params(step_null_block)[0];
        fb.ins().call(decref_ref, &[step_null_args]);
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(term_dispatch_block);
        let term_dispatch_args = fb.block_params(term_dispatch_block)[0];
        let term_dispatch_term = fb.block_params(term_dispatch_block)[1];
        let kind_inst = fb.ins().call(term_kind_ref, &[term_dispatch_term]);
        let kind_val = fb.inst_results(kind_inst)[0];
        let jump_kind = fb.ins().iconst(i64_ty, 0);
        let ret_kind = fb.ins().iconst(i64_ty, 1);
        let raise_kind = fb.ins().iconst(i64_ty, 2);

        let after_jump_check = fb.create_block();
        fb.append_block_param(after_jump_check, ptr_ty);
        fb.append_block_param(after_jump_check, ptr_ty);

        let td_pair = [
            ir::BlockArg::Value(term_dispatch_args),
            ir::BlockArg::Value(term_dispatch_term),
        ];
        let is_jump = fb.ins().icmp(ir::condcodes::IntCC::Equal, kind_val, jump_kind);
        fb.ins()
            .brif(is_jump, jump_block, &td_pair, after_jump_check, &td_pair);

        fb.switch_to_block(after_jump_check);
        let aj_args = fb.block_params(after_jump_check)[0];
        let aj_term = fb.block_params(after_jump_check)[1];

        let after_ret_check = fb.create_block();
        fb.append_block_param(after_ret_check, ptr_ty);
        fb.append_block_param(after_ret_check, ptr_ty);

        let aj_pair = [ir::BlockArg::Value(aj_args), ir::BlockArg::Value(aj_term)];
        let is_ret = fb.ins().icmp(ir::condcodes::IntCC::Equal, kind_val, ret_kind);
        fb.ins().brif(is_ret, ret_block, &aj_pair, after_ret_check, &aj_pair);

        fb.switch_to_block(after_ret_check);
        let ar_args = fb.block_params(after_ret_check)[0];
        let ar_term = fb.block_params(after_ret_check)[1];
        let ar_pair = [ir::BlockArg::Value(ar_args), ir::BlockArg::Value(ar_term)];
        let is_raise = fb.ins().icmp(ir::condcodes::IntCC::Equal, kind_val, raise_kind);
        fb.ins()
            .brif(is_raise, raise_block, &ar_pair, invalid_term_block, &ar_pair);

        fb.switch_to_block(jump_block);
        let jump_args = fb.block_params(jump_block)[0];
        let jump_term = fb.block_params(jump_block)[1];
        let target_inst = fb.ins().call(term_jump_target_ref, &[jump_term]);
        let target_val = fb.inst_results(target_inst)[0];
        let next_args_inst = fb.ins().call(term_jump_args_ref, &[jump_term]);
        let next_args_val = fb.inst_results(next_args_inst)[0];
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        let next_args_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, next_args_val, null_ptr);
        let jump_invalid_args = [
            ir::BlockArg::Value(jump_args),
            ir::BlockArg::Value(jump_term),
            ir::BlockArg::Value(next_args_val),
        ];
        let first_match_args = [
            ir::BlockArg::Value(jump_args),
            ir::BlockArg::Value(jump_term),
            ir::BlockArg::Value(next_args_val),
            ir::BlockArg::Value(target_val),
        ];
        fb.ins().brif(
            next_args_null,
            jump_invalid_target_block,
            &jump_invalid_args,
            jump_match_blocks[0],
            &first_match_args,
        );

        for (index, match_block) in jump_match_blocks.iter().enumerate() {
            fb.switch_to_block(*match_block);
            let match_args = fb.block_params(*match_block)[0];
            let match_term = fb.block_params(*match_block)[1];
            let match_next_args = fb.block_params(*match_block)[2];
            let match_target = fb.block_params(*match_block)[3];
            let block_const = fb.ins().iconst(ptr_ty, blocks[index] as i64);
            let is_match = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, match_target, block_const);
            let matched_dispatch = fb.create_block();
            fb.append_block_param(matched_dispatch, ptr_ty); // args
            fb.append_block_param(matched_dispatch, ptr_ty); // term
            fb.append_block_param(matched_dispatch, ptr_ty); // next_args
            let matched_args = [
                ir::BlockArg::Value(match_args),
                ir::BlockArg::Value(match_term),
                ir::BlockArg::Value(match_next_args),
            ];
            if index + 1 < jump_match_blocks.len() {
                let next = jump_match_blocks[index + 1];
                let fallback_args = [
                    ir::BlockArg::Value(match_args),
                    ir::BlockArg::Value(match_term),
                    ir::BlockArg::Value(match_next_args),
                    ir::BlockArg::Value(match_target),
                ];
                fb.ins()
                    .brif(is_match, matched_dispatch, &matched_args, next, &fallback_args);
            } else {
                fb.ins().brif(
                    is_match,
                    matched_dispatch,
                    &matched_args,
                    jump_invalid_target_block,
                    &matched_args,
                );
            }

            fb.switch_to_block(matched_dispatch);
            let md_args = fb.block_params(matched_dispatch)[0];
            let md_term = fb.block_params(matched_dispatch)[1];
            let md_next_args = fb.block_params(matched_dispatch)[2];
            fb.ins().call(decref_ref, &[md_term]);
            fb.ins().call(decref_ref, &[md_args]);
            let next_idx = fb.ins().iconst(i64_ty, index as i64);
            let dispatch_jump_args = [
                ir::BlockArg::Value(next_idx),
                ir::BlockArg::Value(md_next_args),
            ];
            fb.ins().jump(dispatch_block, &dispatch_jump_args);
        }

        fb.switch_to_block(jump_invalid_target_block);
        let jit_args = fb.block_params(jump_invalid_target_block)[0];
        let jit_term = fb.block_params(jump_invalid_target_block)[1];
        let jit_next_args = fb.block_params(jump_invalid_target_block)[2];
        let _ = fb.ins().call(term_invalid_ref, &[jit_term]);
        fb.ins().call(decref_ref, &[jit_next_args]);
        fb.ins().call(decref_ref, &[jit_term]);
        fb.ins().call(decref_ref, &[jit_args]);
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(ret_block);
        let ret_args = fb.block_params(ret_block)[0];
        let ret_term = fb.block_params(ret_block)[1];
        let ret_val_inst = fb.ins().call(term_ret_value_ref, &[ret_term]);
        let ret_value = fb.inst_results(ret_val_inst)[0];
        fb.ins().call(decref_ref, &[ret_term]);
        fb.ins().call(decref_ref, &[ret_args]);
        fb.ins().return_(&[ret_value]);

        fb.switch_to_block(raise_block);
        let raise_args = fb.block_params(raise_block)[0];
        let raise_term = fb.block_params(raise_block)[1];
        let _ = fb.ins().call(term_raise_ref, &[raise_term]);
        fb.ins().call(decref_ref, &[raise_term]);
        fb.ins().call(decref_ref, &[raise_args]);
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(invalid_term_block);
        let invalid_term_args = fb.block_params(invalid_term_block)[0];
        let invalid_term = fb.block_params(invalid_term_block)[1];
        let _ = fb.ins().call(term_invalid_ref, &[invalid_term]);
        fb.ins().call(decref_ref, &[invalid_term]);
        fb.ins().call(decref_ref, &[invalid_term_args]);
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        fb.ins().return_(&[null_ptr]);

        fb.seal_all_blocks();
        fb.finalize();
    }

    Ok((ctx, main_id))
}

pub unsafe fn render_cranelift_run_bb_specialized(
    blocks: &[ObjPtr],
    entry_index: usize,
) -> Result<String, String> {
    if blocks.is_empty() {
        return Err("specialized JIT run_bb requires at least one block".to_string());
    }
    if entry_index >= blocks.len() {
        return Err(format!(
            "specialized JIT run_bb entry index out of range: {entry_index} >= {}",
            blocks.len()
        ));
    }

    let mut builder = new_jit_builder()?;
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol("dp_jit_run_bb_step", dp_jit_run_bb_step as *const u8);
    builder.symbol("dp_jit_term_kind", dp_jit_term_kind as *const u8);
    builder.symbol("dp_jit_term_jump_target", dp_jit_term_jump_target as *const u8);
    builder.symbol("dp_jit_term_jump_args", dp_jit_term_jump_args as *const u8);
    builder.symbol("dp_jit_term_ret_value", dp_jit_term_ret_value as *const u8);
    builder.symbol("dp_jit_term_raise", dp_jit_term_raise as *const u8);
    builder.symbol("dp_jit_term_invalid", dp_jit_term_invalid as *const u8);
    let mut jit_module = JITModule::new(builder);
    let (ctx, _) = build_cranelift_run_bb_specialized_function(&mut jit_module, blocks, entry_index)?;
    Ok(ctx.func.display().to_string())
}

pub unsafe fn run_cranelift_run_bb_specialized(
    blocks: &[ObjPtr],
    entry_index: usize,
    args: ObjPtr,
    incref_fn: IncrefFn,
    decref_fn: DecrefFn,
    run_bb_step_fn: RunBbStepFn,
    term_kind_fn: TermKindFn,
    term_jump_target_fn: TermJumpTargetFn,
    term_jump_args_fn: TermJumpArgsFn,
    term_ret_value_fn: TermRetValueFn,
    term_raise_fn: TermRaiseFn,
    term_invalid_fn: TermInvalidFn,
) -> Result<ObjPtr, String> {
    if args.is_null() {
        return Err("invalid null args passed to specialized JIT run_bb".to_string());
    }
    if blocks.is_empty() {
        return Err("specialized JIT run_bb requires at least one block".to_string());
    }
    if entry_index >= blocks.len() {
        return Err(format!(
            "specialized JIT run_bb entry index out of range: {entry_index} >= {}",
            blocks.len()
        ));
    }

    DP_JIT_INCREF_FN = Some(incref_fn);
    DP_JIT_DECREF_FN = Some(decref_fn);
    DP_JIT_RUN_BB_STEP_FN = Some(run_bb_step_fn);
    DP_JIT_TERM_KIND_FN = Some(term_kind_fn);
    DP_JIT_TERM_JUMP_TARGET_FN = Some(term_jump_target_fn);
    DP_JIT_TERM_JUMP_ARGS_FN = Some(term_jump_args_fn);
    DP_JIT_TERM_RET_VALUE_FN = Some(term_ret_value_fn);
    DP_JIT_TERM_RAISE_FN = Some(term_raise_fn);
    DP_JIT_TERM_INVALID_FN = Some(term_invalid_fn);

    let mut builder = new_jit_builder()?;
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol("dp_jit_run_bb_step", dp_jit_run_bb_step as *const u8);
    builder.symbol("dp_jit_term_kind", dp_jit_term_kind as *const u8);
    builder.symbol("dp_jit_term_jump_target", dp_jit_term_jump_target as *const u8);
    builder.symbol("dp_jit_term_jump_args", dp_jit_term_jump_args as *const u8);
    builder.symbol("dp_jit_term_ret_value", dp_jit_term_ret_value as *const u8);
    builder.symbol("dp_jit_term_raise", dp_jit_term_raise as *const u8);
    builder.symbol("dp_jit_term_invalid", dp_jit_term_invalid as *const u8);
    let mut jit_module = JITModule::new(builder);
    let (mut ctx, main_id) =
        build_cranelift_run_bb_specialized_function(&mut jit_module, blocks, entry_index)?;

    jit_module
        .define_function(main_id, &mut ctx)
        .map_err(|err| format!("failed to define specialized jit run_bb function: {err}"))?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize specialized jit run_bb function: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(main_id);
    let compiled: extern "C" fn(ObjPtr) -> ObjPtr = std::mem::transmute(code_ptr);
    Ok(compiled(args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_specialized_jit_clif_smoke() {
        let blocks = [1usize as ObjPtr, 2usize as ObjPtr, 3usize as ObjPtr];
        let clif = unsafe { render_cranelift_run_bb_specialized(&blocks, 1) }
            .expect("specialized JIT CLIF render should succeed");
        assert!(
            clif.contains("function"),
            "missing function body in rendered CLIF:\n{clif}"
        );
        assert!(
            clif.contains("call fn2"),
            "missing run_bb_step helper call in rendered CLIF:\n{clif}"
        );
    }
}

use cranelift_codegen::cfg_printer::CFGPrinter;
use cranelift_codegen::incremental_cache::CacheKvStore;
use cranelift_codegen::ir;
use cranelift_codegen::ir::InstBuilder;
use cranelift_codegen::settings;
use cranelift_codegen::settings::Configurable;
use cranelift_control::ControlPlane;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Switch};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, ModuleReloc};
use dp_transform::block_py::{BlockPyModule, intrinsics};
use dp_transform::passes::PreparedBbBlockPyPass;
use pyo3::ffi;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

mod planning;
mod specialized_helpers;

pub use planning::{
    BlockExcArgSource, BlockExcDispatchPlan, BlockFastPath, ClifBlockPlan, ClifPlan,
    DirectSimpleAssignPlan, DirectSimpleBlockArgPlan, DirectSimpleBlockPlan, DirectSimpleBrIfPlan,
    DirectSimpleCallPart, DirectSimpleDeletePlan, DirectSimpleDeleteTargetPlan,
    DirectSimpleExprPlan, DirectSimpleOpPlan, DirectSimpleRetPlan, DirectSimpleTermPlan,
    lookup_blockpy_function, lookup_clif_plan, register_clif_module_plans,
};
pub use specialized_helpers::ObjPtr;
pub use specialized_helpers::SpecializedJitHooks;
pub use specialized_helpers::default_specialized_hooks;
use specialized_helpers::{
    dp_jit_decref, install_specialized_hooks, register_specialized_jit_symbols,
};

static INCREMENTAL_CLIF_CACHE: OnceLock<Mutex<HashMap<Vec<u8>, Vec<u8>>>> = OnceLock::new();
static NEXT_IMPORT_SPEC_ID: AtomicUsize = AtomicUsize::new(0);

fn incremental_clif_cache() -> &'static Mutex<HashMap<Vec<u8>, Vec<u8>>> {
    INCREMENTAL_CLIF_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

struct GlobalIncrementalCacheStore<'a> {
    map: &'a Mutex<HashMap<Vec<u8>, Vec<u8>>>,
}

#[derive(Clone, Copy, Debug)]
enum SigType {
    Pointer,
    I64,
    I32,
}

#[derive(Clone, Copy, Debug)]
struct StaticSignature {
    params: &'static [SigType],
    returns: &'static [SigType],
}

impl StaticSignature {
    const fn new(params: &'static [SigType], returns: &'static [SigType]) -> Self {
        Self { params, returns }
    }
}

#[derive(Debug)]
struct ImportSpec {
    symbol: &'static str,
    signature: StaticSignature,
    internal_id: OnceLock<usize>,
}

impl ImportSpec {
    const fn new(symbol: &'static str, signature: StaticSignature) -> Self {
        Self {
            symbol,
            signature,
            internal_id: OnceLock::new(),
        }
    }

    fn internal_id(&'static self) -> usize {
        *self
            .internal_id
            .get_or_init(|| NEXT_IMPORT_SPEC_ID.fetch_add(1, Ordering::Relaxed))
    }
}

struct ModuleFuncImports {
    func_ids_by_internal_id: Vec<Option<FuncId>>,
    import_id_to_symbol: HashMap<u32, &'static str>,
}

impl ModuleFuncImports {
    fn new() -> Self {
        Self {
            func_ids_by_internal_id: Vec::new(),
            import_id_to_symbol: HashMap::new(),
        }
    }

    fn debug_symbols(&self) -> &HashMap<u32, &'static str> {
        &self.import_id_to_symbol
    }

    fn ensure_declared(
        &mut self,
        jit_module: &mut JITModule,
        spec: &'static ImportSpec,
    ) -> Result<FuncId, String> {
        let internal_id = spec.internal_id();
        if internal_id >= self.func_ids_by_internal_id.len() {
            self.func_ids_by_internal_id.resize(internal_id + 1, None);
        }
        if let Some(func_id) = self.func_ids_by_internal_id[internal_id] {
            return Ok(func_id);
        }
        let sig = lower_static_signature(jit_module, spec.signature);
        let func_id = declare_import_fn(jit_module, spec.symbol, &sig)?;
        self.func_ids_by_internal_id[internal_id] = Some(func_id);
        self.import_id_to_symbol
            .insert(func_id.as_u32(), spec.symbol);
        Ok(func_id)
    }
}

struct FuncBuildImports<'a> {
    module_imports: &'a mut ModuleFuncImports,
    func_refs_by_internal_id: Vec<Option<ir::FuncRef>>,
}

impl<'a> FuncBuildImports<'a> {
    fn new(module_imports: &'a mut ModuleFuncImports) -> Self {
        Self {
            module_imports,
            func_refs_by_internal_id: Vec::new(),
        }
    }

    fn get(
        &mut self,
        jit_module: &mut JITModule,
        func: &mut ir::Function,
        spec: &'static ImportSpec,
    ) -> Result<ir::FuncRef, String> {
        let internal_id = spec.internal_id();
        if internal_id >= self.func_refs_by_internal_id.len() {
            self.func_refs_by_internal_id.resize(internal_id + 1, None);
        }
        if let Some(func_ref) = self.func_refs_by_internal_id[internal_id] {
            return Ok(func_ref);
        }
        let func_id = self.module_imports.ensure_declared(jit_module, spec)?;
        let func_ref = jit_module.declare_func_in_func(func_id, func);
        self.func_refs_by_internal_id[internal_id] = Some(func_ref);
        Ok(func_ref)
    }
}

impl CacheKvStore for GlobalIncrementalCacheStore<'_> {
    fn get(&self, key: &[u8]) -> Option<Cow<'_, [u8]>> {
        let map = self.map.lock().ok()?;
        map.get(key).map(|value| Cow::Owned(value.clone()))
    }

    fn insert(&mut self, key: &[u8], val: Vec<u8>) {
        if let Ok(mut map) = self.map.lock() {
            map.insert(key.to_vec(), val);
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderedSpecializedClif {
    pub clif: String,
    pub cfg_dot: String,
    pub vcode_disasm: String,
}

struct CompiledSpecializedRunner {
    _jit_module: JITModule,
    _literal_pool: Vec<Box<[u8]>>,
    entry: Option<extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr>,
}

pub type VectorcallEntryFn = unsafe extern "C" fn(ObjPtr, *const ObjPtr, usize, ObjPtr) -> ObjPtr;

struct CompiledVectorcallRunner {
    _jit_module: JITModule,
}

fn direct_simple_expr_is_borrowable(expr: &DirectSimpleExprPlan, local_names: &[String]) -> bool {
    match expr {
        DirectSimpleExprPlan::Name(name) => local_names.iter().any(|candidate| candidate == name),
        DirectSimpleExprPlan::Int(_)
        | DirectSimpleExprPlan::Float(_)
        | DirectSimpleExprPlan::Bytes(_)
        | DirectSimpleExprPlan::Intrinsic { .. }
        | DirectSimpleExprPlan::Call { .. } => false,
    }
}

enum DirectSimpleCallCallee<'a> {
    Name(&'a str),
    Intrinsic(&'static dyn intrinsics::Intrinsic),
}

impl DirectSimpleCallCallee<'_> {
    fn name(&self) -> &str {
        match self {
            DirectSimpleCallCallee::Name(name) => name,
            DirectSimpleCallCallee::Intrinsic(intrinsic) => intrinsic.name(),
        }
    }
}

fn direct_simple_call_positional_args<'a>(
    expr: &'a DirectSimpleExprPlan,
) -> Option<(DirectSimpleCallCallee<'a>, Vec<&'a DirectSimpleExprPlan>)> {
    let (callee, parts) = match expr {
        DirectSimpleExprPlan::Call { func, parts } => {
            let DirectSimpleExprPlan::Name(func_name) = func.as_ref() else {
                return None;
            };
            (
                DirectSimpleCallCallee::Name(func_name.as_str()),
                parts.as_slice(),
            )
        }
        DirectSimpleExprPlan::Intrinsic { intrinsic, parts } => (
            DirectSimpleCallCallee::Intrinsic(*intrinsic),
            parts.as_slice(),
        ),
        _ => return None,
    };
    let mut args = Vec::with_capacity(parts.len());
    for part in parts {
        let DirectSimpleCallPart::Pos(value) = part else {
            return None;
        };
        args.push(value);
    }
    Some((callee, args))
}

fn direct_simple_expr_const_string(expr: &DirectSimpleExprPlan) -> Option<String> {
    match expr {
        DirectSimpleExprPlan::Bytes(bytes) => String::from_utf8(bytes.clone()).ok(),
        DirectSimpleExprPlan::Intrinsic { .. } | DirectSimpleExprPlan::Call { .. } => {
            let (callee, args) = direct_simple_call_positional_args(expr)?;
            if args.len() != 1 {
                return None;
            }
            let func_name = callee.name();
            if func_name != "__dp_decode_literal_bytes" && func_name != "str" {
                return None;
            }
            let DirectSimpleExprPlan::Bytes(bytes) = args[0] else {
                return None;
            };
            String::from_utf8(bytes.clone()).ok()
        }
        _ => None,
    }
}

fn direct_simple_expr_is_frame_locals_fetch(expr: &DirectSimpleExprPlan) -> bool {
    let Some((callee, args)) = direct_simple_call_positional_args(expr) else {
        return false;
    };
    let func_name = callee.name();
    if func_name == "__dp_frame_locals" && args.len() == 1 {
        return true;
    }
    if (func_name == "PyObject_GetAttr" || func_name == "__dp_getattr") && args.len() == 2 {
        return direct_simple_expr_const_string(args[1]).as_deref() == Some("f_locals");
    }
    false
}

fn direct_simple_expr_as_frame_locals_setitem<'a>(
    expr: &'a DirectSimpleExprPlan,
    aliases: &HashSet<String>,
) -> Option<(
    &'a DirectSimpleExprPlan,
    &'a DirectSimpleExprPlan,
    &'a DirectSimpleExprPlan,
    String,
)> {
    let (callee, args) = direct_simple_call_positional_args(expr)?;
    let func_name = callee.name();
    if (func_name != "PyObject_SetItem" && func_name != "__dp_setitem") || args.len() != 3 {
        return None;
    }
    if let DirectSimpleExprPlan::Name(alias_name) = args[0] {
        if !aliases.contains(alias_name) && !direct_simple_expr_is_frame_locals_fetch(args[0]) {
            return None;
        }
    } else if !direct_simple_expr_is_frame_locals_fetch(args[0]) {
        return None;
    }
    let key_name = direct_simple_expr_const_string(args[1])?;
    Some((args[0], args[1], args[2], key_name))
}

fn intern_bytes_literal(literal_pool: &mut Vec<Box<[u8]>>, bytes: &[u8]) -> (*const u8, i64) {
    let boxed = bytes.to_vec().into_boxed_slice();
    let ptr = boxed.as_ptr();
    let len = boxed.len() as i64;
    literal_pool.push(boxed);
    (ptr, len)
}

struct DirectSimpleEmitConsts {
    step_null_block: ir::Block,
    step_null_args: Vec<ir::Value>,
    ptr_ty: ir::Type,
    i64_ty: ir::Type,
    none_const: ir::Value,
    true_const: ir::Value,
    false_const: ir::Value,
    deleted_const: ir::Value,
    empty_tuple_const: ir::Value,
    block_const: ir::Value,
}

struct DirectSimpleEmitCtx {
    incref_ref: ir::FuncRef,
    decref_ref: ir::FuncRef,
    py_call_ref: ir::FuncRef,
    make_int_ref: ir::FuncRef,
    consts: DirectSimpleEmitConsts,
    load_name_ref: ir::FuncRef,
    load_local_raw_by_name_ref: ir::FuncRef,
    pyobject_getattr_ref: ir::FuncRef,
    pyobject_setattr_ref: ir::FuncRef,
    pyobject_getitem_ref: ir::FuncRef,
    pyobject_setitem_ref: ir::FuncRef,
    decode_literal_bytes_ref: ir::FuncRef,
    load_deleted_name_ref: ir::FuncRef,
    make_cell_ref: ir::FuncRef,
    load_cell_ref: ir::FuncRef,
    store_cell_ref: ir::FuncRef,
    store_cell_if_not_deleted_ref: ir::FuncRef,
    make_bytes_ref: ir::FuncRef,
    make_float_ref: ir::FuncRef,
    py_call_object_ref: ir::FuncRef,
    py_call_with_kw_ref: ir::FuncRef,
    tuple_new_ref: ir::FuncRef,
    tuple_set_item_ref: ir::FuncRef,
    operator_refs: DirectSimpleOperatorRefs,
    ambient_names: Vec<String>,
    ambient_values: Vec<ir::Value>,
}

struct DirectSimpleIntrinsicEmitState<'a, 'b, 'c, 'd> {
    fb: &'a mut FunctionBuilder<'b>,
    local_names: &'c [String],
    local_values: &'c [ir::Value],
    ctx: &'c DirectSimpleEmitCtx,
    literal_pool: &'c mut Vec<Box<[u8]>>,
    jit_module: &'a mut JITModule,
    func_imports: &'a mut FuncBuildImports<'d>,
}

struct FunctionStateSlots {
    names: Vec<String>,
    slots: Vec<ir::StackSlot>,
}

impl FunctionStateSlots {
    fn new(fb: &mut FunctionBuilder<'_>, slot_names: &[String]) -> Self {
        let mut slots = Vec::with_capacity(slot_names.len());
        for _ in slot_names {
            slots.push(fb.create_sized_stack_slot(ir::StackSlotData::new(
                ir::StackSlotKind::ExplicitSlot,
                std::mem::size_of::<u64>() as u32,
                0,
            )));
        }
        Self {
            names: slot_names.to_vec(),
            slots,
        }
    }

    fn slot_for_name(&self, name: &str) -> Option<ir::StackSlot> {
        self.names
            .iter()
            .position(|candidate| candidate == name)
            .map(|index| self.slots[index])
    }

    fn initialize_all_to_value(
        &self,
        fb: &mut FunctionBuilder<'_>,
        value: ir::Value,
        incref_ref: ir::FuncRef,
    ) {
        for slot in &self.slots {
            fb.ins().call(incref_ref, &[value]);
            fb.ins().stack_store(value, *slot, 0);
        }
    }

    fn replace_cloned_value(
        &self,
        fb: &mut FunctionBuilder<'_>,
        name: &str,
        value: ir::Value,
        ptr_ty: ir::Type,
        incref_ref: ir::FuncRef,
        decref_ref: ir::FuncRef,
    ) -> Option<()> {
        let slot = self.slot_for_name(name)?;
        let previous = fb.ins().stack_load(ptr_ty, slot, 0);
        fb.ins().call(incref_ref, &[value]);
        fb.ins().stack_store(value, slot, 0);
        fb.ins().call(decref_ref, &[previous]);
        Some(())
    }

    fn decref_all(&self, fb: &mut FunctionBuilder<'_>, ptr_ty: ir::Type, decref_ref: ir::FuncRef) {
        for slot in &self.slots {
            let value = fb.ins().stack_load(ptr_ty, *slot, 0);
            fb.ins().call(decref_ref, &[value]);
        }
    }
}

fn bind_local_value(
    fb: &mut FunctionBuilder<'_>,
    local_names: &mut Vec<String>,
    local_values: &mut Vec<ir::Value>,
    name: &str,
    value: ir::Value,
    function_state_slots: &FunctionStateSlots,
    ptr_ty: ir::Type,
    incref_ref: ir::FuncRef,
    decref_ref: ir::FuncRef,
) {
    if let Some(existing_index) = local_names.iter().position(|candidate| candidate == name) {
        let previous = local_values[existing_index];
        fb.ins().call(decref_ref, &[previous]);
        local_values[existing_index] = value;
    } else {
        local_names.push(name.to_string());
        local_values.push(value);
    }
    let _ =
        function_state_slots.replace_cloned_value(fb, name, value, ptr_ty, incref_ref, decref_ref);
}

fn delete_local_value(
    fb: &mut FunctionBuilder<'_>,
    local_names: &mut Vec<String>,
    local_values: &mut Vec<ir::Value>,
    name: &str,
    function_state_slots: &FunctionStateSlots,
    deleted_const: ir::Value,
    ptr_ty: ir::Type,
    incref_ref: ir::FuncRef,
    decref_ref: ir::FuncRef,
) -> Result<(), String> {
    let Some(index) = local_names.iter().position(|candidate| candidate == name) else {
        return Err(format!("missing local binding for delete target: {name}"));
    };
    let previous = local_values.remove(index);
    local_names.remove(index);
    fb.ins().call(decref_ref, &[previous]);
    let _ = function_state_slots.replace_cloned_value(
        fb,
        name,
        deleted_const,
        ptr_ty,
        incref_ref,
        decref_ref,
    );
    Ok(())
}

impl DirectSimpleIntrinsicEmitState<'_, '_, '_, '_> {
    fn positional_args_for_intrinsic<'a>(
        &self,
        intrinsic: &dyn intrinsics::Intrinsic,
        parts: &'a [DirectSimpleCallPart],
    ) -> Vec<&'a DirectSimpleExprPlan> {
        let mut args = Vec::with_capacity(parts.len());
        for part in parts {
            let DirectSimpleCallPart::Pos(value) = part else {
                panic!(
                    "intrinsic {} received non-positional args in JIT plan",
                    intrinsic.name()
                );
            };
            args.push(value);
        }
        assert_eq!(
            args.len(),
            intrinsic.arity(),
            "intrinsic {} received {} args in JIT plan, expected {}",
            intrinsic.name(),
            args.len(),
            intrinsic.arity()
        );
        args
    }

    fn emit_arg_values(&mut self, args: &[&DirectSimpleExprPlan]) -> Vec<(ir::Value, bool)> {
        let mut arg_values = Vec::with_capacity(args.len());
        for arg in args {
            let borrowed_arg = direct_simple_expr_is_borrowable(arg, self.local_names);
            let value = emit_direct_simple_expr(
                self.fb,
                arg,
                self.local_names,
                self.local_values,
                self.ctx,
                self.literal_pool,
                borrowed_arg,
                self.jit_module,
                self.func_imports,
            );
            arg_values.push((value, borrowed_arg));
        }
        arg_values
    }

    fn import_func(&mut self, spec: &'static ImportSpec) -> ir::FuncRef {
        self.func_imports
            .get(self.jit_module, &mut self.fb.func, spec)
            .unwrap_or_else(|err| {
                panic!(
                    "failed to bind import {} during direct-simple intrinsic emission: {}",
                    spec.symbol, err
                )
            })
    }

    fn release_arg_values(&mut self, arg_values: &[(ir::Value, bool)]) {
        for (value, borrowed_arg) in arg_values {
            if !borrowed_arg {
                self.fb.ins().call(self.ctx.decref_ref, &[*value]);
            }
        }
    }

    fn finish_owned_result(&mut self, value: ir::Value) -> ir::Value {
        let null_ptr = self.fb.ins().iconst(self.ctx.consts.ptr_ty, 0);
        let value_is_null = self
            .fb
            .ins()
            .icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
        let value_ok_block = self.fb.create_block();
        self.fb
            .append_block_param(value_ok_block, self.ctx.consts.ptr_ty);
        self.fb.ins().brif(
            value_is_null,
            self.ctx.consts.step_null_block,
            &step_null_block_args(self.ctx),
            value_ok_block,
            &[ir::BlockArg::Value(value)],
        );
        self.fb.switch_to_block(value_ok_block);
        self.fb.block_params(value_ok_block)[0]
    }

    fn emit_owned_func_call(
        &mut self,
        func_ref: ir::FuncRef,
        args: &[&DirectSimpleExprPlan],
    ) -> ir::Value {
        let arg_values = self.emit_arg_values(args);
        let values = arg_values
            .iter()
            .map(|(value, _)| *value)
            .collect::<Vec<_>>();
        let call_inst = self.fb.ins().call(func_ref, &values);
        self.release_arg_values(&arg_values);
        self.finish_owned_result(self.fb.inst_results(call_inst)[0])
    }

    fn emit_bool_func_call(
        &mut self,
        func_ref: ir::FuncRef,
        args: &[&DirectSimpleExprPlan],
    ) -> ir::Value {
        let arg_values = self.emit_arg_values(args);
        let values = arg_values
            .iter()
            .map(|(value, _)| *value)
            .collect::<Vec<_>>();
        let call_inst = self.fb.ins().call(func_ref, &values);
        self.release_arg_values(&arg_values);
        emit_owned_bool_from_i32_result(self.fb, self.fb.inst_results(call_inst)[0], self.ctx)
    }

    fn emit_decode_literal_bytes(&mut self, bytes: &[u8]) -> ir::Value {
        let data = intern_bytes_literal(self.literal_pool, bytes);
        let data_ptr_val = self.fb.ins().iconst(self.ctx.consts.ptr_ty, data.0 as i64);
        let data_len_val = self.fb.ins().iconst(self.ctx.consts.i64_ty, data.1);
        let value_inst = self.fb.ins().call(
            self.ctx.decode_literal_bytes_ref,
            &[data_ptr_val, data_len_val],
        );
        self.finish_owned_result(self.fb.inst_results(value_inst)[0])
    }

    fn emit_pack_tuple(&mut self, args: &[&DirectSimpleExprPlan]) -> ir::Value {
        let arg_values = self.emit_arg_values(args);
        let tuple_value = emit_pack_current_values_tuple(
            self.fb,
            &arg_values
                .iter()
                .map(|(value, _)| *value)
                .collect::<Vec<_>>(),
            self.ctx,
        );
        self.release_arg_values(&arg_values);
        tuple_value
    }
}

trait JitIntrinsic: intrinsics::Intrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> Option<ir::Value>;
}

static BINARY_OBJECT_SIGNATURE: StaticSignature =
    StaticSignature::new(&[SigType::Pointer, SigType::Pointer], &[SigType::Pointer]);

static PYNUMBER_ADD_IMPORT: ImportSpec = ImportSpec::new("PyNumber_Add", BINARY_OBJECT_SIGNATURE);

impl JitIntrinsic for intrinsics::AddIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> Option<ir::Value> {
        let args = state.positional_args_for_intrinsic(self, parts);
        let add_ref = state.import_func(&PYNUMBER_ADD_IMPORT);
        Some(state.emit_owned_func_call(add_ref, &args))
    }
}

fn jit_intrinsic_by_intrinsic(
    intrinsic: &'static dyn intrinsics::Intrinsic,
) -> Option<&'static dyn JitIntrinsic> {
    intrinsic
        .as_any()
        .downcast_ref::<intrinsics::AddIntrinsic>()
        .map(|value| value as &dyn JitIntrinsic)
}

fn lookup_ambient_value(ctx: &DirectSimpleEmitCtx, name: &str) -> Option<ir::Value> {
    ctx.ambient_names
        .iter()
        .position(|candidate| candidate == name)
        .map(|index| ctx.ambient_values[index])
}

fn emit_decref_ambient_values(fb: &mut FunctionBuilder<'_>, ctx: &DirectSimpleEmitCtx) {
    for value in &ctx.ambient_values {
        fb.ins().call(ctx.decref_ref, &[*value]);
    }
}

fn resolve_named_value(
    name: &str,
    runtime_names: &[String],
    runtime_values: &[ir::Value],
    ambient_names: &[String],
    ambient_values: &[ir::Value],
) -> Option<ir::Value> {
    runtime_names
        .iter()
        .position(|candidate| candidate == name)
        .map(|index| runtime_values[index])
        .or_else(|| {
            ambient_names
                .iter()
                .position(|candidate| candidate == name)
                .map(|index| ambient_values[index])
        })
}

fn block_arg_values(values: &[ir::Value]) -> Vec<ir::BlockArg> {
    values.iter().copied().map(ir::BlockArg::Value).collect()
}

fn step_null_block_args(ctx: &DirectSimpleEmitCtx) -> Vec<ir::BlockArg> {
    block_arg_values(&ctx.consts.step_null_args)
}

fn emit_pack_current_values_tuple(
    fb: &mut FunctionBuilder<'_>,
    values: &[ir::Value],
    ctx: &DirectSimpleEmitCtx,
) -> ir::Value {
    if values.is_empty() {
        fb.ins()
            .call(ctx.incref_ref, &[ctx.consts.empty_tuple_const]);
        return ctx.consts.empty_tuple_const;
    }

    let ptr_ty = ctx.consts.ptr_ty;
    let i64_ty = ctx.consts.i64_ty;
    let null_ptr = fb.ins().iconst(ptr_ty, 0);
    let tuple_len = fb.ins().iconst(i64_ty, values.len() as i64);
    let tuple_inst = fb.ins().call(ctx.tuple_new_ref, &[tuple_len]);
    let tuple_obj = fb.inst_results(tuple_inst)[0];
    let tuple_is_null = fb
        .ins()
        .icmp(ir::condcodes::IntCC::Equal, tuple_obj, null_ptr);
    let tuple_ok_block = fb.create_block();
    fb.append_block_param(tuple_ok_block, ptr_ty);
    fb.ins().brif(
        tuple_is_null,
        ctx.consts.step_null_block,
        &step_null_block_args(ctx),
        tuple_ok_block,
        &[ir::BlockArg::Value(tuple_obj)],
    );
    fb.switch_to_block(tuple_ok_block);
    let tuple_obj = fb.block_params(tuple_ok_block)[0];

    let slot_size = (values.len() * std::mem::size_of::<u64>()) as u32;
    let stack_slot = fb.create_sized_stack_slot(ir::StackSlotData::new(
        ir::StackSlotKind::ExplicitSlot,
        slot_size,
        0,
    ));
    for (index, value) in values.iter().copied().enumerate() {
        fb.ins().stack_store(
            value,
            stack_slot,
            (index * std::mem::size_of::<u64>()) as i32,
        );
    }
    let values_base = fb.ins().stack_addr(ptr_ty, stack_slot, 0);

    let loop_block = fb.create_block();
    fb.append_block_param(loop_block, i64_ty);
    fb.append_block_param(loop_block, ptr_ty);
    let set_fail_block = fb.create_block();
    fb.append_block_param(set_fail_block, ptr_ty);
    let done_block = fb.create_block();
    fb.append_block_param(done_block, ptr_ty);
    let body_block = fb.create_block();
    fb.append_block_param(body_block, i64_ty);
    fb.append_block_param(body_block, ptr_ty);

    let zero_i64 = fb.ins().iconst(i64_ty, 0);
    fb.ins().jump(
        loop_block,
        &[
            ir::BlockArg::Value(zero_i64),
            ir::BlockArg::Value(tuple_obj),
        ],
    );

    fb.switch_to_block(loop_block);
    let loop_index = fb.block_params(loop_block)[0];
    let loop_tuple = fb.block_params(loop_block)[1];
    let at_end = fb
        .ins()
        .icmp(ir::condcodes::IntCC::Equal, loop_index, tuple_len);
    fb.ins().brif(
        at_end,
        done_block,
        &[ir::BlockArg::Value(loop_tuple)],
        body_block,
        &[
            ir::BlockArg::Value(loop_index),
            ir::BlockArg::Value(loop_tuple),
        ],
    );

    fb.switch_to_block(body_block);
    let body_index = fb.block_params(body_block)[0];
    let body_tuple = fb.block_params(body_block)[1];
    let value_offset = fb.ins().ishl_imm(body_index, 3);
    let value_addr = fb.ins().iadd(values_base, value_offset);
    let value = fb.ins().load(ptr_ty, ir::MemFlags::new(), value_addr, 0);
    fb.ins().call(ctx.incref_ref, &[value]);
    let set_inst = fb
        .ins()
        .call(ctx.tuple_set_item_ref, &[body_tuple, body_index, value]);
    let set_result = fb.inst_results(set_inst)[0];
    let set_failed = fb
        .ins()
        .icmp_imm(ir::condcodes::IntCC::NotEqual, set_result, 0);
    let next_index = fb.ins().iadd_imm(body_index, 1);
    fb.ins().brif(
        set_failed,
        set_fail_block,
        &[ir::BlockArg::Value(body_tuple)],
        loop_block,
        &[
            ir::BlockArg::Value(next_index),
            ir::BlockArg::Value(body_tuple),
        ],
    );

    fb.switch_to_block(set_fail_block);
    let failed_tuple = fb.block_params(set_fail_block)[0];
    fb.ins().call(ctx.decref_ref, &[failed_tuple]);
    fb.ins()
        .jump(ctx.consts.step_null_block, &step_null_block_args(ctx));

    fb.switch_to_block(done_block);
    fb.block_params(done_block)[0]
}

#[derive(Clone, Copy)]
struct DirectSimpleOperatorRefs {
    richcompare_ref: ir::FuncRef,
    sequence_contains_ref: ir::FuncRef,
    object_not_ref: ir::FuncRef,
    object_is_true_ref: ir::FuncRef,
    number_subtract_ref: ir::FuncRef,
    number_multiply_ref: ir::FuncRef,
    number_matrix_multiply_ref: ir::FuncRef,
    number_true_divide_ref: ir::FuncRef,
    number_floor_divide_ref: ir::FuncRef,
    number_remainder_ref: ir::FuncRef,
    number_power_ref: ir::FuncRef,
    number_lshift_ref: ir::FuncRef,
    number_rshift_ref: ir::FuncRef,
    number_or_ref: ir::FuncRef,
    number_xor_ref: ir::FuncRef,
    number_and_ref: ir::FuncRef,
    number_inplace_add_ref: ir::FuncRef,
    number_inplace_subtract_ref: ir::FuncRef,
    number_inplace_multiply_ref: ir::FuncRef,
    number_inplace_matrix_multiply_ref: ir::FuncRef,
    number_inplace_true_divide_ref: ir::FuncRef,
    number_inplace_floor_divide_ref: ir::FuncRef,
    number_inplace_remainder_ref: ir::FuncRef,
    number_inplace_power_ref: ir::FuncRef,
    number_inplace_lshift_ref: ir::FuncRef,
    number_inplace_rshift_ref: ir::FuncRef,
    number_inplace_or_ref: ir::FuncRef,
    number_inplace_xor_ref: ir::FuncRef,
    number_inplace_and_ref: ir::FuncRef,
    number_positive_ref: ir::FuncRef,
    number_negative_ref: ir::FuncRef,
    number_invert_ref: ir::FuncRef,
}

fn emit_owned_bool_from_cond(
    fb: &mut FunctionBuilder<'_>,
    cond: ir::Value,
    ctx: &DirectSimpleEmitCtx,
) -> ir::Value {
    let bool_value = fb
        .ins()
        .select(cond, ctx.consts.true_const, ctx.consts.false_const);
    fb.ins().call(ctx.incref_ref, &[bool_value]);
    bool_value
}

fn emit_owned_bool_from_i32_result(
    fb: &mut FunctionBuilder<'_>,
    result: ir::Value,
    ctx: &DirectSimpleEmitCtx,
) -> ir::Value {
    let is_error = fb.ins().icmp_imm(ir::condcodes::IntCC::Equal, result, -1);
    let ok_block = fb.create_block();
    fb.ins().brif(
        is_error,
        ctx.consts.step_null_block,
        &step_null_block_args(ctx),
        ok_block,
        &[],
    );
    fb.switch_to_block(ok_block);
    let is_true = fb.ins().icmp_imm(ir::condcodes::IntCC::NotEqual, result, 0);
    emit_owned_bool_from_cond(fb, is_true, ctx)
}

fn emit_direct_simple_expr(
    fb: &mut FunctionBuilder<'_>,
    expr: &DirectSimpleExprPlan,
    local_names: &[String],
    local_values: &[ir::Value],
    ctx: &DirectSimpleEmitCtx,
    literal_pool: &mut Vec<Box<[u8]>>,
    borrowed: bool,
    jit_module: &mut JITModule,
    func_imports: &mut FuncBuildImports<'_>,
) -> ir::Value {
    let incref_ref = ctx.incref_ref;
    let decref_ref = ctx.decref_ref;
    let py_call_ref = ctx.py_call_ref;
    let make_int_ref = ctx.make_int_ref;
    let step_null_block = ctx.consts.step_null_block;
    let ptr_ty = ctx.consts.ptr_ty;
    let i32_ty = ir::types::I32;
    let i64_ty = ctx.consts.i64_ty;
    let none_const = ctx.consts.none_const;
    let deleted_const = ctx.consts.deleted_const;
    let empty_tuple_const = ctx.consts.empty_tuple_const;
    let block_const = ctx.consts.block_const;
    let load_name_ref = ctx.load_name_ref;
    let pyobject_getattr_ref = ctx.pyobject_getattr_ref;
    let pyobject_setattr_ref = ctx.pyobject_setattr_ref;
    let pyobject_getitem_ref = ctx.pyobject_getitem_ref;
    let pyobject_setitem_ref = ctx.pyobject_setitem_ref;
    let decode_literal_bytes_ref = ctx.decode_literal_bytes_ref;
    let load_deleted_name_ref = ctx.load_deleted_name_ref;
    let make_cell_ref = ctx.make_cell_ref;
    let load_cell_ref = ctx.load_cell_ref;
    let store_cell_ref = ctx.store_cell_ref;
    let store_cell_if_not_deleted_ref = ctx.store_cell_if_not_deleted_ref;
    let make_bytes_ref = ctx.make_bytes_ref;
    let make_float_ref = ctx.make_float_ref;
    let py_call_object_ref = ctx.py_call_object_ref;
    let py_call_with_kw_ref = ctx.py_call_with_kw_ref;
    let tuple_new_ref = ctx.tuple_new_ref;
    let tuple_set_item_ref = ctx.tuple_set_item_ref;
    let operator_refs = ctx.operator_refs;

    match expr {
        DirectSimpleExprPlan::Name(name) => {
            if let Some(slot_index) = local_names.iter().position(|candidate| candidate == name) {
                let slot_value = local_values[slot_index];
                if !borrowed {
                    fb.ins().call(incref_ref, &[slot_value]);
                }
                return slot_value;
            }
            if let Some(slot_value) = lookup_ambient_value(ctx, name) {
                if !borrowed {
                    fb.ins().call(incref_ref, &[slot_value]);
                }
                return slot_value;
            }
            assert!(
                !borrowed,
                "global name lookup must produce owned references"
            );
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let (name_ptr, name_len) = intern_bytes_literal(literal_pool, name.as_bytes());
            let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
            let name_len_val = fb.ins().iconst(i64_ty, name_len);
            let value_inst = fb
                .ins()
                .call(load_name_ref, &[block_const, name_ptr_val, name_len_val]);
            let value = fb.inst_results(value_inst)[0];
            let value_is_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
            let value_ok_block = fb.create_block();
            fb.append_block_param(value_ok_block, ptr_ty);
            fb.ins().brif(
                value_is_null,
                step_null_block,
                &step_null_block_args(ctx),
                value_ok_block,
                &[ir::BlockArg::Value(value)],
            );
            fb.switch_to_block(value_ok_block);
            fb.block_params(value_ok_block)[0]
        }
        DirectSimpleExprPlan::Int(value) => {
            assert!(
                !borrowed,
                "direct simple plan must not use borrowed int expression"
            );
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let int_const = fb.ins().iconst(i64_ty, *value);
            let int_inst = fb.ins().call(make_int_ref, &[int_const]);
            let int_value = fb.inst_results(int_inst)[0];
            let int_is_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, int_value, null_ptr);
            let int_ok_block = fb.create_block();
            fb.append_block_param(int_ok_block, ptr_ty);
            fb.ins().brif(
                int_is_null,
                step_null_block,
                &step_null_block_args(ctx),
                int_ok_block,
                &[ir::BlockArg::Value(int_value)],
            );
            fb.switch_to_block(int_ok_block);
            fb.block_params(int_ok_block)[0]
        }
        DirectSimpleExprPlan::Float(value) => {
            assert!(
                !borrowed,
                "direct simple plan must not use borrowed float expression"
            );
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let float_const = fb.ins().f64const(*value);
            let float_inst = fb.ins().call(make_float_ref, &[float_const]);
            let float_value = fb.inst_results(float_inst)[0];
            let float_is_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, float_value, null_ptr);
            let float_ok_block = fb.create_block();
            fb.append_block_param(float_ok_block, ptr_ty);
            fb.ins().brif(
                float_is_null,
                step_null_block,
                &step_null_block_args(ctx),
                float_ok_block,
                &[ir::BlockArg::Value(float_value)],
            );
            fb.switch_to_block(float_ok_block);
            fb.block_params(float_ok_block)[0]
        }
        DirectSimpleExprPlan::Bytes(bytes) => {
            assert!(!borrowed, "bytes literal must produce owned references");
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let (data_ptr, data_len) = intern_bytes_literal(literal_pool, bytes.as_slice());
            let data_ptr_val = fb.ins().iconst(ptr_ty, data_ptr as i64);
            let data_len_val = fb.ins().iconst(i64_ty, data_len);
            let value_inst = fb.ins().call(make_bytes_ref, &[data_ptr_val, data_len_val]);
            let value = fb.inst_results(value_inst)[0];
            let value_is_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
            let value_ok_block = fb.create_block();
            fb.append_block_param(value_ok_block, ptr_ty);
            fb.ins().brif(
                value_is_null,
                step_null_block,
                &step_null_block_args(ctx),
                value_ok_block,
                &[ir::BlockArg::Value(value)],
            );
            fb.switch_to_block(value_ok_block);
            fb.block_params(value_ok_block)[0]
        }
        DirectSimpleExprPlan::Intrinsic { intrinsic, parts } => {
            assert!(
                !borrowed,
                "direct simple plan must not use borrowed intrinsic expression"
            );
            let mut intrinsic_state = DirectSimpleIntrinsicEmitState {
                fb,
                local_names,
                local_values,
                ctx,
                literal_pool,
                jit_module,
                func_imports,
            };
            if let Some(jit_intrinsic) = jit_intrinsic_by_intrinsic(*intrinsic) {
                if let Some(value) = jit_intrinsic.emit_direct_simple(&mut intrinsic_state, parts) {
                    return value;
                }
            }
            let fallback = DirectSimpleExprPlan::Call {
                func: Box::new(DirectSimpleExprPlan::Name(intrinsic.name().to_string())),
                parts: parts.clone(),
            };
            emit_direct_simple_expr(
                intrinsic_state.fb,
                &fallback,
                intrinsic_state.local_names,
                intrinsic_state.local_values,
                intrinsic_state.ctx,
                intrinsic_state.literal_pool,
                false,
                intrinsic_state.jit_module,
                intrinsic_state.func_imports,
            )
        }
        DirectSimpleExprPlan::Call { func, parts } => {
            assert!(
                !borrowed,
                "direct simple plan must not use borrowed call expression"
            );
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let mut simple_args: Vec<&DirectSimpleExprPlan> = Vec::new();
            let mut simple_keywords: Vec<(&str, &DirectSimpleExprPlan)> = Vec::new();
            let mut has_unpack = false;
            for part in parts {
                match part {
                    DirectSimpleCallPart::Pos(value) => simple_args.push(value),
                    DirectSimpleCallPart::Kw { name, value } => {
                        simple_keywords.push((name.as_str(), value))
                    }
                    DirectSimpleCallPart::Star(_) | DirectSimpleCallPart::KwStar(_) => {
                        has_unpack = true;
                    }
                }
            }
            let args: Vec<&DirectSimpleExprPlan> = simple_args.clone();
            let keywords: Vec<(&str, &DirectSimpleExprPlan)> = simple_keywords.clone();
            if let DirectSimpleExprPlan::Name(func_name) = func.as_ref() {
                if !has_unpack
                    && simple_keywords.is_empty()
                    && func_name == "__dp_decode_literal_bytes"
                    && simple_args.len() == 1
                {
                    if let DirectSimpleExprPlan::Bytes(bytes) = simple_args[0] {
                        let (data_ptr, data_len) =
                            intern_bytes_literal(literal_pool, bytes.as_slice());
                        let data_ptr_val = fb.ins().iconst(ptr_ty, data_ptr as i64);
                        let data_len_val = fb.ins().iconst(i64_ty, data_len);
                        let value_inst = fb
                            .ins()
                            .call(decode_literal_bytes_ref, &[data_ptr_val, data_len_val]);
                        let value = fb.inst_results(value_inst)[0];
                        let value_is_null =
                            fb.ins().icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
                        let value_ok_block = fb.create_block();
                        fb.append_block_param(value_ok_block, ptr_ty);
                        fb.ins().brif(
                            value_is_null,
                            step_null_block,
                            &step_null_block_args(ctx),
                            value_ok_block,
                            &[ir::BlockArg::Value(value)],
                        );
                        fb.switch_to_block(value_ok_block);
                        return fb.block_params(value_ok_block)[0];
                    }
                }
                if !has_unpack
                    && simple_keywords.is_empty()
                    && func_name == "str"
                    && simple_args.len() == 1
                {
                    if let DirectSimpleExprPlan::Bytes(bytes) = simple_args[0] {
                        let (data_ptr, data_len) =
                            intern_bytes_literal(literal_pool, bytes.as_slice());
                        let data_ptr_val = fb.ins().iconst(ptr_ty, data_ptr as i64);
                        let data_len_val = fb.ins().iconst(i64_ty, data_len);
                        let value_inst = fb
                            .ins()
                            .call(decode_literal_bytes_ref, &[data_ptr_val, data_len_val]);
                        let value = fb.inst_results(value_inst)[0];
                        let value_is_null =
                            fb.ins().icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
                        let value_ok_block = fb.create_block();
                        fb.append_block_param(value_ok_block, ptr_ty);
                        fb.ins().brif(
                            value_is_null,
                            step_null_block,
                            &step_null_block_args(ctx),
                            value_ok_block,
                            &[ir::BlockArg::Value(value)],
                        );
                        fb.switch_to_block(value_ok_block);
                        return fb.block_params(value_ok_block)[0];
                    }
                }
                if !has_unpack
                    && simple_keywords.is_empty()
                    && simple_args.is_empty()
                    && (func_name == "globals" || func_name == "__dp_globals")
                {
                    fb.ins().call(incref_ref, &[block_const]);
                    return block_const;
                }
            }
            if has_unpack {
                let callable_is_borrowed =
                    direct_simple_expr_is_borrowable(func.as_ref(), local_names);
                let callable = emit_direct_simple_expr(
                    fb,
                    func.as_ref(),
                    local_names,
                    local_values,
                    ctx,
                    literal_pool,
                    callable_is_borrowed,
                    jit_module,
                    func_imports,
                );

                let list_name_bytes = b"__dp_list";
                let list_name_ptr = fb.ins().iconst(ptr_ty, list_name_bytes.as_ptr() as i64);
                let list_name_len = fb.ins().iconst(i64_ty, list_name_bytes.len() as i64);
                let list_callable_inst = fb
                    .ins()
                    .call(load_name_ref, &[block_const, list_name_ptr, list_name_len]);
                let list_callable = fb.inst_results(list_callable_inst)[0];
                let list_callable_is_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, list_callable, null_ptr);
                let list_callable_ok = fb.create_block();
                fb.append_block_param(list_callable_ok, ptr_ty);
                fb.ins().brif(
                    list_callable_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    list_callable_ok,
                    &[ir::BlockArg::Value(list_callable)],
                );
                fb.switch_to_block(list_callable_ok);
                let list_callable = fb.block_params(list_callable_ok)[0];
                let args_list_inst = fb
                    .ins()
                    .call(py_call_object_ref, &[list_callable, empty_tuple_const]);
                fb.ins().call(decref_ref, &[list_callable]);
                let args_list = fb.inst_results(args_list_inst)[0];
                let args_list_is_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, args_list, null_ptr);
                let args_list_ok = fb.create_block();
                fb.append_block_param(args_list_ok, ptr_ty);
                fb.ins().brif(
                    args_list_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    args_list_ok,
                    &[ir::BlockArg::Value(args_list)],
                );
                fb.switch_to_block(args_list_ok);
                let args_list = fb.block_params(args_list_ok)[0];

                let needs_kwargs = parts.iter().any(|part| {
                    matches!(
                        part,
                        DirectSimpleCallPart::Kw { .. } | DirectSimpleCallPart::KwStar(_)
                    )
                });
                let kwargs_obj = if needs_kwargs {
                    let dict_name_bytes = b"__dp_dict";
                    let dict_name_ptr = fb.ins().iconst(ptr_ty, dict_name_bytes.as_ptr() as i64);
                    let dict_name_len = fb.ins().iconst(i64_ty, dict_name_bytes.len() as i64);
                    let dict_callable_inst = fb
                        .ins()
                        .call(load_name_ref, &[block_const, dict_name_ptr, dict_name_len]);
                    let dict_callable = fb.inst_results(dict_callable_inst)[0];
                    let dict_callable_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, dict_callable, null_ptr);
                    let dict_callable_ok = fb.create_block();
                    fb.append_block_param(dict_callable_ok, ptr_ty);
                    fb.ins().brif(
                        dict_callable_is_null,
                        step_null_block,
                        &step_null_block_args(ctx),
                        dict_callable_ok,
                        &[ir::BlockArg::Value(dict_callable)],
                    );
                    fb.switch_to_block(dict_callable_ok);
                    let dict_callable = fb.block_params(dict_callable_ok)[0];
                    let kwargs_inst = fb
                        .ins()
                        .call(py_call_object_ref, &[dict_callable, empty_tuple_const]);
                    fb.ins().call(decref_ref, &[dict_callable]);
                    let kwargs_obj = fb.inst_results(kwargs_inst)[0];
                    let kwargs_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, kwargs_obj, null_ptr);
                    let kwargs_ok = fb.create_block();
                    fb.append_block_param(kwargs_ok, ptr_ty);
                    fb.ins().brif(
                        kwargs_is_null,
                        step_null_block,
                        &step_null_block_args(ctx),
                        kwargs_ok,
                        &[ir::BlockArg::Value(kwargs_obj)],
                    );
                    fb.switch_to_block(kwargs_ok);
                    Some(fb.block_params(kwargs_ok)[0])
                } else {
                    None
                };

                for part in parts {
                    match part {
                        DirectSimpleCallPart::Pos(value_expr)
                        | DirectSimpleCallPart::Star(value_expr) => {
                            let method_name = match part {
                                DirectSimpleCallPart::Pos(_) => b"append".as_slice(),
                                _ => b"extend".as_slice(),
                            };
                            let (method_ptr, method_len) =
                                intern_bytes_literal(literal_pool, method_name);
                            let method_ptr_val = fb.ins().iconst(ptr_ty, method_ptr as i64);
                            let method_len_val = fb.ins().iconst(i64_ty, method_len);
                            let method_name_inst = fb
                                .ins()
                                .call(decode_literal_bytes_ref, &[method_ptr_val, method_len_val]);
                            let method_name_obj = fb.inst_results(method_name_inst)[0];
                            let method_name_is_null = fb.ins().icmp(
                                ir::condcodes::IntCC::Equal,
                                method_name_obj,
                                null_ptr,
                            );
                            let method_name_ok = fb.create_block();
                            fb.append_block_param(method_name_ok, ptr_ty);
                            fb.ins().brif(
                                method_name_is_null,
                                step_null_block,
                                &step_null_block_args(ctx),
                                method_name_ok,
                                &[ir::BlockArg::Value(method_name_obj)],
                            );
                            fb.switch_to_block(method_name_ok);
                            let method_name_obj = fb.block_params(method_name_ok)[0];
                            let method_inst = fb
                                .ins()
                                .call(pyobject_getattr_ref, &[args_list, method_name_obj]);
                            fb.ins().call(decref_ref, &[method_name_obj]);
                            let method_obj = fb.inst_results(method_inst)[0];
                            let method_is_null =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, method_obj, null_ptr);
                            let method_ok = fb.create_block();
                            fb.append_block_param(method_ok, ptr_ty);
                            fb.ins().brif(
                                method_is_null,
                                step_null_block,
                                &step_null_block_args(ctx),
                                method_ok,
                                &[ir::BlockArg::Value(method_obj)],
                            );
                            fb.switch_to_block(method_ok);
                            let method_obj = fb.block_params(method_ok)[0];
                            let value_borrowed =
                                direct_simple_expr_is_borrowable(value_expr, local_names);
                            let value_obj = emit_direct_simple_expr(
                                fb,
                                value_expr,
                                local_names,
                                local_values,
                                ctx,
                                literal_pool,
                                value_borrowed,
                                jit_module,
                                func_imports,
                            );
                            let call_inst = fb.ins().call(
                                py_call_ref,
                                &[method_obj, value_obj, null_ptr, null_ptr, null_ptr],
                            );
                            if !value_borrowed {
                                fb.ins().call(decref_ref, &[value_obj]);
                            }
                            fb.ins().call(decref_ref, &[method_obj]);
                            let call_value = fb.inst_results(call_inst)[0];
                            let call_is_null =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, call_value, null_ptr);
                            let call_ok = fb.create_block();
                            fb.append_block_param(call_ok, ptr_ty);
                            fb.ins().brif(
                                call_is_null,
                                step_null_block,
                                &step_null_block_args(ctx),
                                call_ok,
                                &[ir::BlockArg::Value(call_value)],
                            );
                            fb.switch_to_block(call_ok);
                            let call_value = fb.block_params(call_ok)[0];
                            fb.ins().call(decref_ref, &[call_value]);
                        }
                        DirectSimpleCallPart::Kw { name, value } => {
                            let kwargs_obj =
                                kwargs_obj.expect("kwargs object must exist for kw part");
                            let (key_ptr, key_len) =
                                intern_bytes_literal(literal_pool, name.as_bytes());
                            let key_ptr_val = fb.ins().iconst(ptr_ty, key_ptr as i64);
                            let key_len_val = fb.ins().iconst(i64_ty, key_len);
                            let key_inst = fb
                                .ins()
                                .call(decode_literal_bytes_ref, &[key_ptr_val, key_len_val]);
                            let key_obj = fb.inst_results(key_inst)[0];
                            let key_is_null =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, key_obj, null_ptr);
                            let key_ok = fb.create_block();
                            fb.append_block_param(key_ok, ptr_ty);
                            fb.ins().brif(
                                key_is_null,
                                step_null_block,
                                &step_null_block_args(ctx),
                                key_ok,
                                &[ir::BlockArg::Value(key_obj)],
                            );
                            fb.switch_to_block(key_ok);
                            let key_obj = fb.block_params(key_ok)[0];
                            let value_borrowed =
                                direct_simple_expr_is_borrowable(value, local_names);
                            let value_obj = emit_direct_simple_expr(
                                fb,
                                value,
                                local_names,
                                local_values,
                                ctx,
                                literal_pool,
                                value_borrowed,
                                jit_module,
                                func_imports,
                            );
                            let set_inst = fb
                                .ins()
                                .call(pyobject_setitem_ref, &[kwargs_obj, key_obj, value_obj]);
                            fb.ins().call(decref_ref, &[key_obj]);
                            if !value_borrowed {
                                fb.ins().call(decref_ref, &[value_obj]);
                            }
                            let set_value = fb.inst_results(set_inst)[0];
                            let set_failed =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, set_value, null_ptr);
                            let set_ok = fb.create_block();
                            let set_fail = fb.create_block();
                            fb.append_block_param(set_fail, ptr_ty);
                            fb.ins().brif(
                                set_failed,
                                set_fail,
                                &[ir::BlockArg::Value(kwargs_obj)],
                                set_ok,
                                &[],
                            );
                            fb.switch_to_block(set_fail);
                            let failed_kwargs = fb.block_params(set_fail)[0];
                            fb.ins().call(decref_ref, &[failed_kwargs]);
                            fb.ins().call(decref_ref, &[args_list]);
                            if !callable_is_borrowed {
                                fb.ins().call(decref_ref, &[callable]);
                            }
                            fb.ins().jump(step_null_block, &step_null_block_args(ctx));
                            fb.switch_to_block(set_ok);
                            fb.ins().call(decref_ref, &[set_value]);
                        }
                        DirectSimpleCallPart::KwStar(value_expr) => {
                            let kwargs_obj =
                                kwargs_obj.expect("kwargs object must exist for kwstar part");
                            let (update_ptr, update_len) =
                                intern_bytes_literal(literal_pool, b"update");
                            let update_ptr_val = fb.ins().iconst(ptr_ty, update_ptr as i64);
                            let update_len_val = fb.ins().iconst(i64_ty, update_len);
                            let update_name_inst = fb
                                .ins()
                                .call(decode_literal_bytes_ref, &[update_ptr_val, update_len_val]);
                            let update_name_obj = fb.inst_results(update_name_inst)[0];
                            let update_name_is_null = fb.ins().icmp(
                                ir::condcodes::IntCC::Equal,
                                update_name_obj,
                                null_ptr,
                            );
                            let update_name_ok = fb.create_block();
                            fb.append_block_param(update_name_ok, ptr_ty);
                            fb.ins().brif(
                                update_name_is_null,
                                step_null_block,
                                &step_null_block_args(ctx),
                                update_name_ok,
                                &[ir::BlockArg::Value(update_name_obj)],
                            );
                            fb.switch_to_block(update_name_ok);
                            let update_name_obj = fb.block_params(update_name_ok)[0];
                            let update_inst = fb
                                .ins()
                                .call(pyobject_getattr_ref, &[kwargs_obj, update_name_obj]);
                            fb.ins().call(decref_ref, &[update_name_obj]);
                            let update_obj = fb.inst_results(update_inst)[0];
                            let update_is_null =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, update_obj, null_ptr);
                            let update_ok = fb.create_block();
                            fb.append_block_param(update_ok, ptr_ty);
                            fb.ins().brif(
                                update_is_null,
                                step_null_block,
                                &step_null_block_args(ctx),
                                update_ok,
                                &[ir::BlockArg::Value(update_obj)],
                            );
                            fb.switch_to_block(update_ok);
                            let update_obj = fb.block_params(update_ok)[0];
                            let value_borrowed =
                                direct_simple_expr_is_borrowable(value_expr, local_names);
                            let value_obj = emit_direct_simple_expr(
                                fb,
                                value_expr,
                                local_names,
                                local_values,
                                ctx,
                                literal_pool,
                                value_borrowed,
                                jit_module,
                                func_imports,
                            );
                            let call_inst = fb.ins().call(
                                py_call_ref,
                                &[update_obj, value_obj, null_ptr, null_ptr, null_ptr],
                            );
                            if !value_borrowed {
                                fb.ins().call(decref_ref, &[value_obj]);
                            }
                            fb.ins().call(decref_ref, &[update_obj]);
                            let call_value = fb.inst_results(call_inst)[0];
                            let call_is_null =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, call_value, null_ptr);
                            let call_ok = fb.create_block();
                            fb.append_block_param(call_ok, ptr_ty);
                            fb.ins().brif(
                                call_is_null,
                                step_null_block,
                                &step_null_block_args(ctx),
                                call_ok,
                                &[ir::BlockArg::Value(call_value)],
                            );
                            fb.switch_to_block(call_ok);
                            let call_value = fb.block_params(call_ok)[0];
                            fb.ins().call(decref_ref, &[call_value]);
                        }
                    }
                }

                let tuple_name_bytes = b"__dp_tuple_from_iter";
                let tuple_name_ptr = fb.ins().iconst(ptr_ty, tuple_name_bytes.as_ptr() as i64);
                let tuple_name_len = fb.ins().iconst(i64_ty, tuple_name_bytes.len() as i64);
                let tuple_callable_inst = fb.ins().call(
                    load_name_ref,
                    &[block_const, tuple_name_ptr, tuple_name_len],
                );
                let tuple_callable = fb.inst_results(tuple_callable_inst)[0];
                let tuple_callable_is_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, tuple_callable, null_ptr);
                let tuple_callable_ok = fb.create_block();
                fb.append_block_param(tuple_callable_ok, ptr_ty);
                fb.ins().brif(
                    tuple_callable_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    tuple_callable_ok,
                    &[ir::BlockArg::Value(tuple_callable)],
                );
                fb.switch_to_block(tuple_callable_ok);
                let tuple_callable = fb.block_params(tuple_callable_ok)[0];
                let tuple_call_inst = fb.ins().call(
                    py_call_ref,
                    &[tuple_callable, args_list, null_ptr, null_ptr, null_ptr],
                );
                fb.ins().call(decref_ref, &[tuple_callable]);
                fb.ins().call(decref_ref, &[args_list]);
                let call_args_tuple = fb.inst_results(tuple_call_inst)[0];
                let call_args_tuple_is_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, call_args_tuple, null_ptr);
                let call_args_tuple_ok = fb.create_block();
                fb.append_block_param(call_args_tuple_ok, ptr_ty);
                fb.ins().brif(
                    call_args_tuple_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    call_args_tuple_ok,
                    &[ir::BlockArg::Value(call_args_tuple)],
                );
                fb.switch_to_block(call_args_tuple_ok);
                let call_args_tuple = fb.block_params(call_args_tuple_ok)[0];

                let call_inst = if let Some(kwargs_obj) = kwargs_obj {
                    let call_inst = fb.ins().call(
                        py_call_with_kw_ref,
                        &[callable, call_args_tuple, kwargs_obj],
                    );
                    fb.ins().call(decref_ref, &[kwargs_obj]);
                    call_inst
                } else {
                    fb.ins()
                        .call(py_call_object_ref, &[callable, call_args_tuple])
                };
                fb.ins().call(decref_ref, &[call_args_tuple]);
                if !callable_is_borrowed {
                    fb.ins().call(decref_ref, &[callable]);
                }
                let call_value = fb.inst_results(call_inst)[0];
                let call_is_null = fb
                    .ins()
                    .icmp(ir::condcodes::IntCC::Equal, call_value, null_ptr);
                let call_ok_block = fb.create_block();
                fb.append_block_param(call_ok_block, ptr_ty);
                fb.ins().brif(
                    call_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    call_ok_block,
                    &[ir::BlockArg::Value(call_value)],
                );
                fb.switch_to_block(call_ok_block);
                return fb.block_params(call_ok_block)[0];
            }
            if let DirectSimpleExprPlan::Name(func_name) = func.as_ref() {
                if keywords.is_empty()
                    && func_name == "__dp_decode_literal_bytes"
                    && args.len() == 1
                {
                    if let DirectSimpleExprPlan::Bytes(bytes) = &args[0] {
                        let (data_ptr, data_len) =
                            intern_bytes_literal(literal_pool, bytes.as_slice());
                        let data_ptr_val = fb.ins().iconst(ptr_ty, data_ptr as i64);
                        let data_len_val = fb.ins().iconst(i64_ty, data_len);
                        let value_inst = fb
                            .ins()
                            .call(decode_literal_bytes_ref, &[data_ptr_val, data_len_val]);
                        let value = fb.inst_results(value_inst)[0];
                        let value_is_null =
                            fb.ins().icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
                        let value_ok_block = fb.create_block();
                        fb.append_block_param(value_ok_block, ptr_ty);
                        fb.ins().brif(
                            value_is_null,
                            step_null_block,
                            &step_null_block_args(ctx),
                            value_ok_block,
                            &[ir::BlockArg::Value(value)],
                        );
                        fb.switch_to_block(value_ok_block);
                        return fb.block_params(value_ok_block)[0];
                    }
                }
                if keywords.is_empty() && func_name == "str" && args.len() == 1 {
                    if let DirectSimpleExprPlan::Bytes(bytes) = &args[0] {
                        let (data_ptr, data_len) =
                            intern_bytes_literal(literal_pool, bytes.as_slice());
                        let data_ptr_val = fb.ins().iconst(ptr_ty, data_ptr as i64);
                        let data_len_val = fb.ins().iconst(i64_ty, data_len);
                        let value_inst = fb
                            .ins()
                            .call(decode_literal_bytes_ref, &[data_ptr_val, data_len_val]);
                        let value = fb.inst_results(value_inst)[0];
                        let value_is_null =
                            fb.ins().icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
                        let value_ok_block = fb.create_block();
                        fb.append_block_param(value_ok_block, ptr_ty);
                        fb.ins().brif(
                            value_is_null,
                            step_null_block,
                            &step_null_block_args(ctx),
                            value_ok_block,
                            &[ir::BlockArg::Value(value)],
                        );
                        fb.switch_to_block(value_ok_block);
                        return fb.block_params(value_ok_block)[0];
                    }
                }
                if keywords.is_empty()
                    && args.is_empty()
                    && (func_name == "globals" || func_name == "__dp_globals")
                {
                    fb.ins().call(incref_ref, &[block_const]);
                    return block_const;
                }
                if keywords.is_empty() {
                    if func_name == "__dp_tuple" {
                        let mut arg_values: Vec<ir::Value> = Vec::with_capacity(args.len());
                        let mut borrowed_args: Vec<bool> = Vec::with_capacity(args.len());
                        for arg in &args {
                            let borrowed_arg = direct_simple_expr_is_borrowable(arg, local_names);
                            let value = emit_direct_simple_expr(
                                fb,
                                arg,
                                local_names,
                                local_values,
                                ctx,
                                literal_pool,
                                borrowed_arg,
                                jit_module,
                                func_imports,
                            );
                            arg_values.push(value);
                            borrowed_args.push(borrowed_arg);
                        }
                        let tuple_value =
                            emit_pack_current_values_tuple(fb, arg_values.as_slice(), ctx);
                        for (value, borrowed_arg) in
                            arg_values.into_iter().zip(borrowed_args.into_iter())
                        {
                            if !borrowed_arg {
                                fb.ins().call(decref_ref, &[value]);
                            }
                        }
                        return tuple_value;
                    }
                    if func_name == "__dp_load_deleted_name" && args.len() == 2 {
                        if let Some(name) = direct_simple_expr_const_string(args[0]) {
                            let (name_ptr, name_len) =
                                intern_bytes_literal(literal_pool, name.as_bytes());
                            let value_borrowed =
                                direct_simple_expr_is_borrowable(args[1], local_names);
                            let value_obj = emit_direct_simple_expr(
                                fb,
                                args[1],
                                local_names,
                                local_values,
                                ctx,
                                literal_pool,
                                value_borrowed,
                                jit_module,
                                func_imports,
                            );
                            let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
                            let name_len_val = fb.ins().iconst(i64_ty, name_len);
                            let call_inst = fb.ins().call(
                                load_deleted_name_ref,
                                &[name_ptr_val, name_len_val, value_obj, deleted_const],
                            );
                            if !value_borrowed {
                                fb.ins().call(decref_ref, &[value_obj]);
                            }
                            let call_value = fb.inst_results(call_inst)[0];
                            let call_is_null =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, call_value, null_ptr);
                            let call_ok_block = fb.create_block();
                            fb.append_block_param(call_ok_block, ptr_ty);
                            fb.ins().brif(
                                call_is_null,
                                step_null_block,
                                &step_null_block_args(ctx),
                                call_ok_block,
                                &[ir::BlockArg::Value(call_value)],
                            );
                            fb.switch_to_block(call_ok_block);
                            return fb.block_params(call_ok_block)[0];
                        }
                    }
                    let is_direct_cell_call = matches!(
                        (func_name.as_str(), args.len()),
                        ("__dp_make_cell", 0 | 1)
                            | ("__dp_load_cell", 1)
                            | ("__dp_store_cell", 2)
                            | ("__dp_store_cell_if_not_deleted", 2)
                    );
                    if is_direct_cell_call {
                        let mut arg_values: Vec<(ir::Value, bool)> = Vec::with_capacity(args.len());
                        for arg in &args {
                            let borrowed_arg = direct_simple_expr_is_borrowable(arg, local_names);
                            let value = emit_direct_simple_expr(
                                fb,
                                arg,
                                local_names,
                                local_values,
                                ctx,
                                literal_pool,
                                borrowed_arg,
                                jit_module,
                                func_imports,
                            );
                            arg_values.push((value, borrowed_arg));
                        }
                        let call_inst = match (func_name.as_str(), args.len()) {
                            ("__dp_make_cell", 0) => fb.ins().call(make_cell_ref, &[null_ptr]),
                            ("__dp_make_cell", 1) => {
                                fb.ins().call(make_cell_ref, &[arg_values[0].0])
                            }
                            ("__dp_load_cell", 1) => {
                                fb.ins().call(load_cell_ref, &[arg_values[0].0])
                            }
                            ("__dp_store_cell", 2) => fb
                                .ins()
                                .call(store_cell_ref, &[arg_values[0].0, arg_values[1].0]),
                            ("__dp_store_cell_if_not_deleted", 2) => fb.ins().call(
                                store_cell_if_not_deleted_ref,
                                &[arg_values[0].0, arg_values[1].0, deleted_const],
                            ),
                            _ => unreachable!("unexpected direct cell call"),
                        };
                        for (value, borrowed_arg) in arg_values {
                            if !borrowed_arg {
                                fb.ins().call(decref_ref, &[value]);
                            }
                        }
                        let call_value = fb.inst_results(call_inst)[0];
                        let call_is_null =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, call_value, null_ptr);
                        let call_ok_block = fb.create_block();
                        fb.append_block_param(call_ok_block, ptr_ty);
                        fb.ins().brif(
                            call_is_null,
                            step_null_block,
                            &step_null_block_args(ctx),
                            call_ok_block,
                            &[ir::BlockArg::Value(call_value)],
                        );
                        fb.switch_to_block(call_ok_block);
                        return fb.block_params(call_ok_block)[0];
                    }
                    let is_direct_operator_call = matches!(
                        (func_name.as_str(), args.len()),
                        ("PyObject_GetAttr", 2)
                            | ("PyObject_SetAttr", 3)
                            | ("PyObject_GetItem", 2)
                            | ("PyObject_SetItem", 3)
                            | ("__dp_sub", 2)
                            | ("__dp_mul", 2)
                            | ("__dp_matmul", 2)
                            | ("__dp_truediv", 2)
                            | ("__dp_floordiv", 2)
                            | ("__dp_mod", 2)
                            | ("__dp_pow", 2 | 3)
                            | ("__dp_lshift", 2)
                            | ("__dp_rshift", 2)
                            | ("__dp_or_", 2)
                            | ("__dp_xor", 2)
                            | ("__dp_and_", 2)
                            | ("__dp_iadd", 2)
                            | ("__dp_isub", 2)
                            | ("__dp_imul", 2)
                            | ("__dp_imatmul", 2)
                            | ("__dp_itruediv", 2)
                            | ("__dp_ifloordiv", 2)
                            | ("__dp_imod", 2)
                            | ("__dp_ipow", 2 | 3)
                            | ("__dp_ilshift", 2)
                            | ("__dp_irshift", 2)
                            | ("__dp_ior", 2)
                            | ("__dp_ixor", 2)
                            | ("__dp_iand", 2)
                            | ("__dp_pos", 1)
                            | ("__dp_neg", 1)
                            | ("__dp_invert", 1)
                            | ("__dp_not_", 1)
                            | ("__dp_truth", 1)
                            | ("__dp_eq", 2)
                            | ("__dp_ne", 2)
                            | ("__dp_lt", 2)
                            | ("__dp_le", 2)
                            | ("__dp_gt", 2)
                            | ("__dp_ge", 2)
                            | ("__dp_contains", 2)
                            | ("__dp_is_", 2)
                            | ("__dp_is_not", 2)
                    );
                    if is_direct_operator_call {
                        let mut arg_values: Vec<(ir::Value, bool)> = Vec::with_capacity(args.len());
                        for arg in &args {
                            let borrowed_arg = direct_simple_expr_is_borrowable(arg, local_names);
                            let value = emit_direct_simple_expr(
                                fb,
                                arg,
                                local_names,
                                local_values,
                                ctx,
                                literal_pool,
                                borrowed_arg,
                                jit_module,
                                func_imports,
                            );
                            arg_values.push((value, borrowed_arg));
                        }
                        match (func_name.as_str(), args.len()) {
                            ("PyObject_GetAttr", 2)
                            | ("PyObject_GetItem", 2)
                            | ("PyObject_SetAttr", 3)
                            | ("PyObject_SetItem", 3)
                            | ("__dp_sub", 2)
                            | ("__dp_mul", 2)
                            | ("__dp_matmul", 2)
                            | ("__dp_truediv", 2)
                            | ("__dp_floordiv", 2)
                            | ("__dp_mod", 2)
                            | ("__dp_pow", 2 | 3)
                            | ("__dp_lshift", 2)
                            | ("__dp_rshift", 2)
                            | ("__dp_or_", 2)
                            | ("__dp_xor", 2)
                            | ("__dp_and_", 2)
                            | ("__dp_iadd", 2)
                            | ("__dp_isub", 2)
                            | ("__dp_imul", 2)
                            | ("__dp_imatmul", 2)
                            | ("__dp_itruediv", 2)
                            | ("__dp_ifloordiv", 2)
                            | ("__dp_imod", 2)
                            | ("__dp_ipow", 2 | 3)
                            | ("__dp_ilshift", 2)
                            | ("__dp_irshift", 2)
                            | ("__dp_ior", 2)
                            | ("__dp_ixor", 2)
                            | ("__dp_iand", 2)
                            | ("__dp_pos", 1)
                            | ("__dp_neg", 1)
                            | ("__dp_invert", 1)
                            | ("__dp_eq", 2)
                            | ("__dp_ne", 2)
                            | ("__dp_lt", 2)
                            | ("__dp_le", 2)
                            | ("__dp_gt", 2)
                            | ("__dp_ge", 2) => {
                                let intrinsic_ref = match (func_name.as_str(), args.len()) {
                                    ("PyObject_GetAttr", 2) => pyobject_getattr_ref,
                                    ("PyObject_SetAttr", 3) => pyobject_setattr_ref,
                                    ("PyObject_GetItem", 2) => pyobject_getitem_ref,
                                    ("PyObject_SetItem", 3) => pyobject_setitem_ref,
                                    ("__dp_sub", 2) => operator_refs.number_subtract_ref,
                                    ("__dp_mul", 2) => operator_refs.number_multiply_ref,
                                    ("__dp_matmul", 2) => operator_refs.number_matrix_multiply_ref,
                                    ("__dp_truediv", 2) => operator_refs.number_true_divide_ref,
                                    ("__dp_floordiv", 2) => operator_refs.number_floor_divide_ref,
                                    ("__dp_mod", 2) => operator_refs.number_remainder_ref,
                                    ("__dp_pow", 2 | 3) => operator_refs.number_power_ref,
                                    ("__dp_lshift", 2) => operator_refs.number_lshift_ref,
                                    ("__dp_rshift", 2) => operator_refs.number_rshift_ref,
                                    ("__dp_or_", 2) => operator_refs.number_or_ref,
                                    ("__dp_xor", 2) => operator_refs.number_xor_ref,
                                    ("__dp_and_", 2) => operator_refs.number_and_ref,
                                    ("__dp_iadd", 2) => operator_refs.number_inplace_add_ref,
                                    ("__dp_isub", 2) => operator_refs.number_inplace_subtract_ref,
                                    ("__dp_imul", 2) => operator_refs.number_inplace_multiply_ref,
                                    ("__dp_imatmul", 2) => {
                                        operator_refs.number_inplace_matrix_multiply_ref
                                    }
                                    ("__dp_itruediv", 2) => {
                                        operator_refs.number_inplace_true_divide_ref
                                    }
                                    ("__dp_ifloordiv", 2) => {
                                        operator_refs.number_inplace_floor_divide_ref
                                    }
                                    ("__dp_imod", 2) => operator_refs.number_inplace_remainder_ref,
                                    ("__dp_ipow", 2 | 3) => operator_refs.number_inplace_power_ref,
                                    ("__dp_ilshift", 2) => operator_refs.number_inplace_lshift_ref,
                                    ("__dp_irshift", 2) => operator_refs.number_inplace_rshift_ref,
                                    ("__dp_ior", 2) => operator_refs.number_inplace_or_ref,
                                    ("__dp_ixor", 2) => operator_refs.number_inplace_xor_ref,
                                    ("__dp_iand", 2) => operator_refs.number_inplace_and_ref,
                                    ("__dp_pos", 1) => operator_refs.number_positive_ref,
                                    ("__dp_neg", 1) => operator_refs.number_negative_ref,
                                    ("__dp_invert", 1) => operator_refs.number_invert_ref,
                                    ("__dp_eq", 2)
                                    | ("__dp_ne", 2)
                                    | ("__dp_lt", 2)
                                    | ("__dp_le", 2)
                                    | ("__dp_gt", 2)
                                    | ("__dp_ge", 2) => operator_refs.richcompare_ref,
                                    _ => unreachable!("unexpected direct operator call"),
                                };
                                let call_inst = match (func_name.as_str(), args.len()) {
                                    ("__dp_pow", 2) | ("__dp_ipow", 2) => fb.ins().call(
                                        intrinsic_ref,
                                        &[arg_values[0].0, arg_values[1].0, none_const],
                                    ),
                                    ("__dp_pow", 3) | ("__dp_ipow", 3) => fb.ins().call(
                                        intrinsic_ref,
                                        &[arg_values[0].0, arg_values[1].0, arg_values[2].0],
                                    ),
                                    ("__dp_eq", 2) => {
                                        let compare_op = fb.ins().iconst(i32_ty, ffi::Py_EQ as i64);
                                        fb.ins().call(
                                            intrinsic_ref,
                                            &[arg_values[0].0, arg_values[1].0, compare_op],
                                        )
                                    }
                                    ("__dp_ne", 2) => {
                                        let compare_op = fb.ins().iconst(i32_ty, ffi::Py_NE as i64);
                                        fb.ins().call(
                                            intrinsic_ref,
                                            &[arg_values[0].0, arg_values[1].0, compare_op],
                                        )
                                    }
                                    ("__dp_lt", 2) => {
                                        let compare_op = fb.ins().iconst(i32_ty, ffi::Py_LT as i64);
                                        fb.ins().call(
                                            intrinsic_ref,
                                            &[arg_values[0].0, arg_values[1].0, compare_op],
                                        )
                                    }
                                    ("__dp_le", 2) => {
                                        let compare_op = fb.ins().iconst(i32_ty, ffi::Py_LE as i64);
                                        fb.ins().call(
                                            intrinsic_ref,
                                            &[arg_values[0].0, arg_values[1].0, compare_op],
                                        )
                                    }
                                    ("__dp_gt", 2) => {
                                        let compare_op = fb.ins().iconst(i32_ty, ffi::Py_GT as i64);
                                        fb.ins().call(
                                            intrinsic_ref,
                                            &[arg_values[0].0, arg_values[1].0, compare_op],
                                        )
                                    }
                                    ("__dp_ge", 2) => {
                                        let compare_op = fb.ins().iconst(i32_ty, ffi::Py_GE as i64);
                                        fb.ins().call(
                                            intrinsic_ref,
                                            &[arg_values[0].0, arg_values[1].0, compare_op],
                                        )
                                    }
                                    (_, 1) => fb.ins().call(intrinsic_ref, &[arg_values[0].0]),
                                    (_, 2) => fb
                                        .ins()
                                        .call(intrinsic_ref, &[arg_values[0].0, arg_values[1].0]),
                                    (_, 3) => fb.ins().call(
                                        intrinsic_ref,
                                        &[arg_values[0].0, arg_values[1].0, arg_values[2].0],
                                    ),
                                    _ => unreachable!("unexpected direct operator arity"),
                                };
                                for (value, borrowed_arg) in arg_values {
                                    if !borrowed_arg {
                                        fb.ins().call(decref_ref, &[value]);
                                    }
                                }
                                let call_value = fb.inst_results(call_inst)[0];
                                let call_is_null = fb.ins().icmp(
                                    ir::condcodes::IntCC::Equal,
                                    call_value,
                                    null_ptr,
                                );
                                let call_ok_block = fb.create_block();
                                fb.append_block_param(call_ok_block, ptr_ty);
                                fb.ins().brif(
                                    call_is_null,
                                    step_null_block,
                                    &step_null_block_args(ctx),
                                    call_ok_block,
                                    &[ir::BlockArg::Value(call_value)],
                                );
                                fb.switch_to_block(call_ok_block);
                                return fb.block_params(call_ok_block)[0];
                            }
                            ("__dp_not_", 1) | ("__dp_truth", 1) | ("__dp_contains", 2) => {
                                let intrinsic_ref = match (func_name.as_str(), args.len()) {
                                    ("__dp_not_", 1) => operator_refs.object_not_ref,
                                    ("__dp_truth", 1) => operator_refs.object_is_true_ref,
                                    ("__dp_contains", 2) => operator_refs.sequence_contains_ref,
                                    _ => unreachable!("unexpected bool-returning operator call"),
                                };
                                let call_inst = match (func_name.as_str(), args.len()) {
                                    ("__dp_not_", 1) | ("__dp_truth", 1) => {
                                        fb.ins().call(intrinsic_ref, &[arg_values[0].0])
                                    }
                                    ("__dp_contains", 2) => fb
                                        .ins()
                                        .call(intrinsic_ref, &[arg_values[0].0, arg_values[1].0]),
                                    _ => unreachable!("unexpected bool-returning operator arity"),
                                };
                                for (value, borrowed_arg) in arg_values {
                                    if !borrowed_arg {
                                        fb.ins().call(decref_ref, &[value]);
                                    }
                                }
                                return emit_owned_bool_from_i32_result(
                                    fb,
                                    fb.inst_results(call_inst)[0],
                                    ctx,
                                );
                            }
                            ("__dp_is_", 2) | ("__dp_is_not", 2) => {
                                let cond = fb.ins().icmp(
                                    if func_name == "__dp_is_" {
                                        ir::condcodes::IntCC::Equal
                                    } else {
                                        ir::condcodes::IntCC::NotEqual
                                    },
                                    arg_values[0].0,
                                    arg_values[1].0,
                                );
                                for (value, borrowed_arg) in arg_values {
                                    if !borrowed_arg {
                                        fb.ins().call(decref_ref, &[value]);
                                    }
                                }
                                return emit_owned_bool_from_cond(fb, cond, ctx);
                            }
                            _ => unreachable!("unexpected direct operator call"),
                        }
                    }
                }
            }
            let callable = emit_direct_simple_expr(
                fb,
                func.as_ref(),
                local_names,
                local_values,
                ctx,
                literal_pool,
                direct_simple_expr_is_borrowable(func.as_ref(), local_names),
                jit_module,
                func_imports,
            );
            let callable_is_borrowed = direct_simple_expr_is_borrowable(func.as_ref(), local_names);
            if keywords.is_empty() && args.len() <= 3 {
                let mut arg_values = [null_ptr, null_ptr, null_ptr];
                let mut arg_borrowed = [true, true, true];
                for (idx, arg) in args.iter().enumerate() {
                    let borrowed_arg = direct_simple_expr_is_borrowable(arg, local_names);
                    arg_borrowed[idx] = borrowed_arg;
                    arg_values[idx] = emit_direct_simple_expr(
                        fb,
                        arg,
                        local_names,
                        local_values,
                        ctx,
                        literal_pool,
                        borrowed_arg,
                        jit_module,
                        func_imports,
                    );
                }
                let call_inst = fb.ins().call(
                    py_call_ref,
                    &[
                        callable,
                        arg_values[0],
                        arg_values[1],
                        arg_values[2],
                        null_ptr,
                    ],
                );
                for idx in 0..3 {
                    if idx < args.len() && !arg_borrowed[idx] {
                        fb.ins().call(decref_ref, &[arg_values[idx]]);
                    }
                }
                if !callable_is_borrowed {
                    fb.ins().call(decref_ref, &[callable]);
                }
                let call_value = fb.inst_results(call_inst)[0];
                let call_is_null = fb
                    .ins()
                    .icmp(ir::condcodes::IntCC::Equal, call_value, null_ptr);
                let call_ok_block = fb.create_block();
                fb.append_block_param(call_ok_block, ptr_ty);
                fb.ins().brif(
                    call_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    call_ok_block,
                    &[ir::BlockArg::Value(call_value)],
                );
                fb.switch_to_block(call_ok_block);
                return fb.block_params(call_ok_block)[0];
            }

            let tuple_len = fb.ins().iconst(i64_ty, args.len() as i64);
            let tuple_inst = fb.ins().call(tuple_new_ref, &[tuple_len]);
            let call_args_tuple = fb.inst_results(tuple_inst)[0];
            let tuple_is_null =
                fb.ins()
                    .icmp(ir::condcodes::IntCC::Equal, call_args_tuple, null_ptr);
            let tuple_ok_block = fb.create_block();
            fb.append_block_param(tuple_ok_block, ptr_ty);
            fb.ins().brif(
                tuple_is_null,
                step_null_block,
                &step_null_block_args(ctx),
                tuple_ok_block,
                &[ir::BlockArg::Value(call_args_tuple)],
            );
            fb.switch_to_block(tuple_ok_block);
            let call_args_tuple = fb.block_params(tuple_ok_block)[0];
            let mut tuple_items: Vec<(ir::Value, bool)> = Vec::with_capacity(args.len());
            for arg in args {
                let borrowed_arg = direct_simple_expr_is_borrowable(arg, local_names);
                let value = emit_direct_simple_expr(
                    fb,
                    arg,
                    local_names,
                    local_values,
                    ctx,
                    literal_pool,
                    borrowed_arg,
                    jit_module,
                    func_imports,
                );
                tuple_items.push((value, borrowed_arg));
            }
            for (index, (value, borrowed_arg)) in tuple_items.iter().enumerate() {
                if *borrowed_arg {
                    fb.ins().call(incref_ref, &[*value]);
                }
                let item_index = fb.ins().iconst(i64_ty, index as i64);
                let set_inst = fb
                    .ins()
                    .call(tuple_set_item_ref, &[call_args_tuple, item_index, *value]);
                let set_result = fb.inst_results(set_inst)[0];
                let set_failed = fb
                    .ins()
                    .icmp_imm(ir::condcodes::IntCC::NotEqual, set_result, 0);
                let set_ok_block = fb.create_block();
                let set_fail_block = fb.create_block();
                fb.append_block_param(set_fail_block, ptr_ty);
                fb.ins().brif(
                    set_failed,
                    set_fail_block,
                    &[ir::BlockArg::Value(call_args_tuple)],
                    set_ok_block,
                    &[],
                );
                fb.switch_to_block(set_fail_block);
                let failed_tuple = fb.block_params(set_fail_block)[0];
                fb.ins().call(decref_ref, &[failed_tuple]);
                fb.ins().jump(step_null_block, &step_null_block_args(ctx));
                fb.switch_to_block(set_ok_block);
            }
            let call_inst = if keywords.is_empty() {
                fb.ins()
                    .call(py_call_object_ref, &[callable, call_args_tuple])
            } else {
                let dict_name_bytes = b"__dp_dict";
                let dict_name_ptr = fb.ins().iconst(ptr_ty, dict_name_bytes.as_ptr() as i64);
                let dict_name_len = fb.ins().iconst(i64_ty, dict_name_bytes.len() as i64);
                let dict_callable_inst = fb
                    .ins()
                    .call(load_name_ref, &[block_const, dict_name_ptr, dict_name_len]);
                let dict_callable = fb.inst_results(dict_callable_inst)[0];
                let dict_callable_is_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, dict_callable, null_ptr);
                let dict_callable_ok = fb.create_block();
                fb.append_block_param(dict_callable_ok, ptr_ty);
                fb.ins().brif(
                    dict_callable_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    dict_callable_ok,
                    &[ir::BlockArg::Value(dict_callable)],
                );
                fb.switch_to_block(dict_callable_ok);
                let dict_callable = fb.block_params(dict_callable_ok)[0];

                let empty_tuple_len = fb.ins().iconst(i64_ty, 0);
                let empty_tuple_inst = fb.ins().call(tuple_new_ref, &[empty_tuple_len]);
                let empty_tuple = fb.inst_results(empty_tuple_inst)[0];
                let empty_tuple_is_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, empty_tuple, null_ptr);
                let empty_tuple_ok = fb.create_block();
                fb.append_block_param(empty_tuple_ok, ptr_ty);
                fb.ins().brif(
                    empty_tuple_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    empty_tuple_ok,
                    &[ir::BlockArg::Value(empty_tuple)],
                );
                fb.switch_to_block(empty_tuple_ok);
                let empty_tuple = fb.block_params(empty_tuple_ok)[0];

                let kwargs_inst = fb
                    .ins()
                    .call(py_call_object_ref, &[dict_callable, empty_tuple]);
                fb.ins().call(decref_ref, &[empty_tuple]);
                fb.ins().call(decref_ref, &[dict_callable]);
                let kwargs_obj = fb.inst_results(kwargs_inst)[0];
                let kwargs_is_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, kwargs_obj, null_ptr);
                let kwargs_ok = fb.create_block();
                fb.append_block_param(kwargs_ok, ptr_ty);
                fb.ins().brif(
                    kwargs_is_null,
                    step_null_block,
                    &step_null_block_args(ctx),
                    kwargs_ok,
                    &[ir::BlockArg::Value(kwargs_obj)],
                );
                fb.switch_to_block(kwargs_ok);
                let kwargs_obj = fb.block_params(kwargs_ok)[0];

                for (name, value_expr) in keywords {
                    let key_bytes = name.as_bytes();
                    let (key_ptr, key_len) = intern_bytes_literal(literal_pool, key_bytes);
                    let key_ptr_val = fb.ins().iconst(ptr_ty, key_ptr as i64);
                    let key_len_val = fb.ins().iconst(i64_ty, key_len);
                    let key_inst = fb
                        .ins()
                        .call(decode_literal_bytes_ref, &[key_ptr_val, key_len_val]);
                    let key_obj = fb.inst_results(key_inst)[0];
                    let key_is_null = fb
                        .ins()
                        .icmp(ir::condcodes::IntCC::Equal, key_obj, null_ptr);
                    let key_ok = fb.create_block();
                    fb.append_block_param(key_ok, ptr_ty);
                    fb.ins().brif(
                        key_is_null,
                        step_null_block,
                        &step_null_block_args(ctx),
                        key_ok,
                        &[ir::BlockArg::Value(key_obj)],
                    );
                    fb.switch_to_block(key_ok);
                    let key_obj = fb.block_params(key_ok)[0];

                    let value_borrowed = direct_simple_expr_is_borrowable(value_expr, local_names);
                    let value_obj = emit_direct_simple_expr(
                        fb,
                        value_expr,
                        local_names,
                        local_values,
                        ctx,
                        literal_pool,
                        value_borrowed,
                        jit_module,
                        func_imports,
                    );
                    let set_inst = fb
                        .ins()
                        .call(pyobject_setitem_ref, &[kwargs_obj, key_obj, value_obj]);
                    fb.ins().call(decref_ref, &[key_obj]);
                    if !value_borrowed {
                        fb.ins().call(decref_ref, &[value_obj]);
                    }
                    let set_value = fb.inst_results(set_inst)[0];
                    let set_failed =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, set_value, null_ptr);
                    let set_ok = fb.create_block();
                    let set_fail = fb.create_block();
                    fb.append_block_param(set_fail, ptr_ty);
                    fb.ins().brif(
                        set_failed,
                        set_fail,
                        &[ir::BlockArg::Value(kwargs_obj)],
                        set_ok,
                        &[],
                    );
                    fb.switch_to_block(set_fail);
                    let failed_kwargs = fb.block_params(set_fail)[0];
                    fb.ins().call(decref_ref, &[failed_kwargs]);
                    fb.ins().call(decref_ref, &[call_args_tuple]);
                    if !callable_is_borrowed {
                        fb.ins().call(decref_ref, &[callable]);
                    }
                    fb.ins().jump(step_null_block, &step_null_block_args(ctx));
                    fb.switch_to_block(set_ok);
                    fb.ins().call(decref_ref, &[set_value]);
                }

                let call_inst = fb.ins().call(
                    py_call_with_kw_ref,
                    &[callable, call_args_tuple, kwargs_obj],
                );
                fb.ins().call(decref_ref, &[kwargs_obj]);
                call_inst
            };
            fb.ins().call(decref_ref, &[call_args_tuple]);
            if !callable_is_borrowed {
                fb.ins().call(decref_ref, &[callable]);
            }
            let call_value = fb.inst_results(call_inst)[0];
            let call_is_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, call_value, null_ptr);
            let call_ok_block = fb.create_block();
            fb.append_block_param(call_ok_block, ptr_ty);
            fb.ins().brif(
                call_is_null,
                step_null_block,
                &step_null_block_args(ctx),
                call_ok_block,
                &[ir::BlockArg::Value(call_value)],
            );
            fb.switch_to_block(call_ok_block);
            fb.block_params(call_ok_block)[0]
        }
    }
}

fn emit_prepare_target_args(
    fb: &mut FunctionBuilder<'_>,
    target_params: &[String],
    explicit_args: Option<&[DirectSimpleBlockArgPlan]>,
    local_names: &[String],
    local_values: &[ir::Value],
    ctx: &DirectSimpleEmitCtx,
    literal_pool: &mut Vec<Box<[u8]>>,
    jit_module: &mut JITModule,
    func_imports: &mut FuncBuildImports<'_>,
) -> Option<Vec<ir::BlockArg>> {
    let mut args = Vec::with_capacity(target_params.len());
    let mut forwarded_local_indices = HashMap::new();
    let explicit_start = explicit_args
        .map(|args| target_params.len().saturating_sub(args.len()))
        .unwrap_or(target_params.len());
    let owner_value = local_names
        .iter()
        .position(|candidate| candidate == "_dp_self" || candidate == "_dp_state")
        .map(|index| local_values[index]);
    for (index, name) in target_params.iter().enumerate() {
        if let Some(explicit_arg) = explicit_args
            .and_then(|args| (index >= explicit_start).then(|| &args[index - explicit_start]))
        {
            let value = match explicit_arg {
                DirectSimpleBlockArgPlan::Name(source_name) => {
                    if let Some(value_index) = local_names
                        .iter()
                        .position(|candidate| candidate == source_name)
                    {
                        let value = local_values[value_index];
                        let forwarded_count =
                            forwarded_local_indices.entry(value_index).or_insert(0usize);
                        if *forwarded_count > 0 {
                            fb.ins().call(ctx.incref_ref, &[value]);
                        }
                        *forwarded_count += 1;
                        value
                    } else if let Some(value) = lookup_ambient_value(ctx, source_name) {
                        value
                    } else {
                        return None;
                    }
                }
                DirectSimpleBlockArgPlan::Expr(expr) => emit_direct_simple_expr(
                    fb,
                    expr,
                    local_names,
                    local_values,
                    ctx,
                    literal_pool,
                    false,
                    jit_module,
                    func_imports,
                ),
                DirectSimpleBlockArgPlan::None => {
                    fb.ins().call(ctx.incref_ref, &[ctx.consts.none_const]);
                    ctx.consts.none_const
                }
                DirectSimpleBlockArgPlan::CurrentException => return None,
            };
            args.push(ir::BlockArg::Value(value));
            continue;
        }
        if let Some(value_index) = local_names.iter().position(|candidate| candidate == name) {
            let value = local_values[value_index];
            let forwarded_count = forwarded_local_indices.entry(value_index).or_insert(0usize);
            if *forwarded_count > 0 {
                fb.ins().call(ctx.incref_ref, &[value]);
            }
            *forwarded_count += 1;
            args.push(ir::BlockArg::Value(value));
            continue;
        }
        if let Some(value) = lookup_ambient_value(ctx, name) {
            args.push(ir::BlockArg::Value(value));
            continue;
        }
        if let Some(owner) = owner_value {
            let ptr_ty = ctx.consts.ptr_ty;
            let i64_ty = ctx.consts.i64_ty;
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let (name_ptr, name_len) = intern_bytes_literal(literal_pool, name.as_bytes());
            let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
            let name_len_val = fb.ins().iconst(i64_ty, name_len);
            let load_inst = fb.ins().call(
                ctx.load_local_raw_by_name_ref,
                &[owner, name_ptr_val, name_len_val],
            );
            let load_value = fb.inst_results(load_inst)[0];
            let load_is_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, load_value, null_ptr);
            let load_ok_block = fb.create_block();
            fb.append_block_param(load_ok_block, ptr_ty);
            fb.ins().brif(
                load_is_null,
                ctx.consts.step_null_block,
                &step_null_block_args(ctx),
                load_ok_block,
                &[ir::BlockArg::Value(load_value)],
            );
            fb.switch_to_block(load_ok_block);
            args.push(ir::BlockArg::Value(fb.block_params(load_ok_block)[0]));
            continue;
        }
        fb.ins().call(ctx.incref_ref, &[ctx.consts.none_const]);
        args.push(ir::BlockArg::Value(ctx.consts.none_const));
    }
    Some(args)
}

fn emit_decref_unforwarded_locals(
    fb: &mut FunctionBuilder<'_>,
    local_values: &[ir::Value],
    local_names: &[String],
    target_params: &[String],
    decref_ref: ir::FuncRef,
) {
    let mut forwarded_local_indices = HashMap::new();
    for name in target_params {
        if let Some(index) = local_names.iter().position(|candidate| candidate == name) {
            *forwarded_local_indices.entry(index).or_insert(0usize) += 1;
        }
    }
    for (index, value) in local_values.iter().enumerate() {
        if forwarded_local_indices.contains_key(&index) {
            continue;
        }
        fb.ins().call(decref_ref, &[*value]);
    }
}

fn emit_truthy_from_owned(
    fb: &mut FunctionBuilder<'_>,
    owned_value: ir::Value,
    is_true_ref: ir::FuncRef,
    decref_ref: ir::FuncRef,
    step_null_block: ir::Block,
    step_null_args: &[ir::Value],
    i32_ty: ir::Type,
) -> ir::Value {
    let truth_inst = fb.ins().call(is_true_ref, &[owned_value]);
    let truth_value = fb.inst_results(truth_inst)[0];
    fb.ins().call(decref_ref, &[owned_value]);
    let truth_error = fb.ins().iconst(i32_ty, -1);
    let is_error = fb
        .ins()
        .icmp(ir::condcodes::IntCC::Equal, truth_value, truth_error);
    let truth_ok_block = fb.create_block();
    fb.append_block_param(truth_ok_block, i32_ty);
    fb.ins().brif(
        is_error,
        step_null_block,
        &block_arg_values(step_null_args),
        truth_ok_block,
        &[ir::BlockArg::Value(truth_value)],
    );
    fb.switch_to_block(truth_ok_block);
    let truth_ok_value = fb.block_params(truth_ok_block)[0];
    let zero_i32 = fb.ins().iconst(i32_ty, 0);
    fb.ins().icmp(
        ir::condcodes::IntCC::SignedGreaterThan,
        truth_ok_value,
        zero_i32,
    )
}

fn emit_direct_simple_ops(
    fb: &mut FunctionBuilder<'_>,
    ops: &[DirectSimpleOpPlan],
    local_names: &mut Vec<String>,
    local_values: &mut Vec<ir::Value>,
    function_state_slots: &FunctionStateSlots,
    emit_ctx: &DirectSimpleEmitCtx,
    literal_pool: &mut Vec<Box<[u8]>>,
    jit_module: &mut JITModule,
    func_imports: &mut FuncBuildImports<'_>,
) -> Result<(), String> {
    let mut frame_locals_aliases: HashSet<String> = HashSet::new();
    for op in ops {
        match op {
            DirectSimpleOpPlan::Assign(assign) => {
                let value_is_frame_locals = direct_simple_expr_is_frame_locals_fetch(&assign.value)
                    || matches!(
                        &assign.value,
                        DirectSimpleExprPlan::Name(name) if frame_locals_aliases.contains(name)
                    );
                let value = emit_direct_simple_expr(
                    fb,
                    &assign.value,
                    local_names,
                    local_values,
                    emit_ctx,
                    literal_pool,
                    false,
                    jit_module,
                    func_imports,
                );
                bind_local_value(
                    fb,
                    local_names,
                    local_values,
                    assign.target.as_str(),
                    value,
                    function_state_slots,
                    emit_ctx.consts.ptr_ty,
                    emit_ctx.incref_ref,
                    emit_ctx.decref_ref,
                );
                if value_is_frame_locals {
                    frame_locals_aliases.insert(assign.target.clone());
                } else {
                    frame_locals_aliases.remove(assign.target.as_str());
                }
            }
            DirectSimpleOpPlan::Expr(expr) => {
                if let Some((obj_expr, key_expr, value_expr, key_name)) =
                    direct_simple_expr_as_frame_locals_setitem(expr, &frame_locals_aliases)
                {
                    let null_ptr = fb.ins().iconst(emit_ctx.consts.ptr_ty, 0);
                    let obj_borrowed = direct_simple_expr_is_borrowable(obj_expr, local_names);
                    let key_borrowed = direct_simple_expr_is_borrowable(key_expr, local_names);
                    let value_borrowed = direct_simple_expr_is_borrowable(value_expr, local_names);
                    let obj_value = emit_direct_simple_expr(
                        fb,
                        obj_expr,
                        local_names,
                        local_values,
                        emit_ctx,
                        literal_pool,
                        obj_borrowed,
                        jit_module,
                        func_imports,
                    );
                    let key_value = emit_direct_simple_expr(
                        fb,
                        key_expr,
                        local_names,
                        local_values,
                        emit_ctx,
                        literal_pool,
                        key_borrowed,
                        jit_module,
                        func_imports,
                    );
                    let value_value = emit_direct_simple_expr(
                        fb,
                        value_expr,
                        local_names,
                        local_values,
                        emit_ctx,
                        literal_pool,
                        value_borrowed,
                        jit_module,
                        func_imports,
                    );
                    let set_item_inst = fb.ins().call(
                        emit_ctx.pyobject_setitem_ref,
                        &[obj_value, key_value, value_value],
                    );
                    let set_item_value = fb.inst_results(set_item_inst)[0];
                    let set_item_failed =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, set_item_value, null_ptr);
                    let set_item_ok = fb.create_block();
                    let set_item_fail = fb.create_block();
                    fb.append_block_param(set_item_ok, emit_ctx.consts.ptr_ty);
                    fb.append_block_param(set_item_fail, emit_ctx.consts.ptr_ty);
                    fb.ins().brif(
                        set_item_failed,
                        set_item_fail,
                        &[ir::BlockArg::Value(set_item_value)],
                        set_item_ok,
                        &[ir::BlockArg::Value(set_item_value)],
                    );
                    fb.switch_to_block(set_item_fail);
                    let failed_set_item_value = fb.block_params(set_item_fail)[0];
                    fb.ins().call(emit_ctx.decref_ref, &[failed_set_item_value]);
                    fb.ins().jump(
                        emit_ctx.consts.step_null_block,
                        &step_null_block_args(emit_ctx),
                    );
                    fb.switch_to_block(set_item_ok);
                    let set_item_value = fb.block_params(set_item_ok)[0];
                    fb.ins().call(emit_ctx.decref_ref, &[set_item_value]);
                    let synced_inst = fb
                        .ins()
                        .call(emit_ctx.pyobject_getitem_ref, &[obj_value, key_value]);
                    let synced_value = fb.inst_results(synced_inst)[0];
                    let synced_failed =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, synced_value, null_ptr);
                    let synced_ok = fb.create_block();
                    let synced_fail = fb.create_block();
                    fb.append_block_param(synced_ok, emit_ctx.consts.ptr_ty);
                    fb.append_block_param(synced_fail, emit_ctx.consts.ptr_ty);
                    fb.ins().brif(
                        synced_failed,
                        synced_fail,
                        &[ir::BlockArg::Value(synced_value)],
                        synced_ok,
                        &[ir::BlockArg::Value(synced_value)],
                    );
                    fb.switch_to_block(synced_fail);
                    let failed_synced_value = fb.block_params(synced_fail)[0];
                    fb.ins().call(emit_ctx.decref_ref, &[failed_synced_value]);
                    fb.ins().jump(
                        emit_ctx.consts.step_null_block,
                        &step_null_block_args(emit_ctx),
                    );
                    fb.switch_to_block(synced_ok);
                    let synced_value = fb.block_params(synced_ok)[0];
                    bind_local_value(
                        fb,
                        local_names,
                        local_values,
                        key_name.as_str(),
                        synced_value,
                        function_state_slots,
                        emit_ctx.consts.ptr_ty,
                        emit_ctx.incref_ref,
                        emit_ctx.decref_ref,
                    );
                    if !obj_borrowed {
                        fb.ins().call(emit_ctx.decref_ref, &[obj_value]);
                    }
                    if !key_borrowed {
                        fb.ins().call(emit_ctx.decref_ref, &[key_value]);
                    }
                    if !value_borrowed {
                        fb.ins().call(emit_ctx.decref_ref, &[value_value]);
                    }
                    continue;
                }
                let value = emit_direct_simple_expr(
                    fb,
                    expr,
                    local_names,
                    local_values,
                    emit_ctx,
                    literal_pool,
                    false,
                    jit_module,
                    func_imports,
                );
                fb.ins().call(emit_ctx.decref_ref, &[value]);
            }
            DirectSimpleOpPlan::Delete(delete_plan) => {
                for target in &delete_plan.targets {
                    let DirectSimpleDeleteTargetPlan::LocalName(name) = target;
                    delete_local_value(
                        fb,
                        local_names,
                        local_values,
                        name.as_str(),
                        function_state_slots,
                        emit_ctx.consts.deleted_const,
                        emit_ctx.consts.ptr_ty,
                        emit_ctx.incref_ref,
                        emit_ctx.decref_ref,
                    )?;
                    frame_locals_aliases.remove(name.as_str());
                }
            }
        }
    }
    Ok(())
}

fn new_jit_builder() -> Result<JITBuilder, String> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("is_pic", "false")
        .map_err(|err| format!("failed to configure Cranelift flags: {err}"))?;
    flag_builder
        .set("preserve_frame_pointers", "true")
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

fn define_function_with_incremental_cache(
    jit_module: &mut JITModule,
    func_id: FuncId,
    ctx: &mut cranelift_codegen::Context,
    err_prefix: &str,
) -> Result<(), String> {
    let func_for_relocs = ctx.func.clone();
    let mut ctrl_plane = ControlPlane::default();
    let mut cache_store = GlobalIncrementalCacheStore {
        map: incremental_clif_cache(),
    };
    let (compiled, _cache_hit) = ctx
        .compile_with_cache(jit_module.isa(), &mut cache_store, &mut ctrl_plane)
        .map_err(|err| format!("{err_prefix}: {err:?}"))?;
    let alignment = compiled.buffer.alignment as u64;
    let relocs = compiled
        .buffer
        .relocs()
        .iter()
        .map(|reloc| ModuleReloc::from_mach_reloc(reloc, &func_for_relocs, func_id))
        .collect::<Vec<_>>();
    jit_module
        .define_function_bytes(func_id, alignment, compiled.code_buffer(), &relocs)
        .map_err(|err| format!("{err_prefix}: {err}"))?;
    Ok(())
}

fn lower_static_signature(jit_module: &mut JITModule, signature: StaticSignature) -> ir::Signature {
    let mut lowered = jit_module.make_signature();
    let lower_sig_type = |sig_type| match sig_type {
        SigType::Pointer => jit_module.target_config().pointer_type(),
        SigType::I64 => ir::types::I64,
        SigType::I32 => ir::types::I32,
    };
    for param in signature.params {
        lowered
            .params
            .push(ir::AbiParam::new(lower_sig_type(*param)));
    }
    for ret in signature.returns {
        lowered
            .returns
            .push(ir::AbiParam::new(lower_sig_type(*ret)));
    }
    lowered
}

fn declare_import_fn(
    jit_module: &mut JITModule,
    symbol: &str,
    sig: &ir::Signature,
) -> Result<FuncId, String> {
    jit_module
        .declare_function(symbol, Linkage::Import, sig)
        .map_err(|err| format!("failed to declare imported {symbol} symbol: {err}"))
}

fn declare_local_fn(
    jit_module: &mut JITModule,
    symbol: &str,
    sig: &ir::Signature,
) -> Result<FuncId, String> {
    jit_module
        .declare_function(symbol, Linkage::Local, sig)
        .map_err(|err| format!("failed to declare local {symbol} function: {err}"))
}

fn is_clif_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn rewrite_import_fn_aliases(
    clif: &str,
    import_id_to_symbol: &HashMap<u32, &'static str>,
) -> String {
    let mut import_aliases: HashMap<String, String> = HashMap::new();
    for raw_line in clif.lines() {
        let line = raw_line.trim_start();
        let Some(eq_pos) = line.find(" = u") else {
            continue;
        };
        let alias = &line[..eq_pos];
        if alias.is_empty() {
            continue;
        }
        let rest = &line[(eq_pos + 4)..];
        let Some(first_token) = rest.split_whitespace().next() else {
            continue;
        };
        let Some(colon_pos) = first_token.find(':') else {
            continue;
        };
        let import_id = &first_token[(colon_pos + 1)..];
        if import_id.is_empty() || !import_id.as_bytes().iter().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(import_id) = import_id.parse::<u32>() else {
            continue;
        };
        let Some(symbol) = import_id_to_symbol.get(&import_id) else {
            continue;
        };
        import_aliases.insert(alias.to_string(), (*symbol).to_string());
    }

    let bytes = clif.as_bytes();
    let mut out = String::with_capacity(clif.len() + 128);
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'f' && index + 2 < bytes.len() && bytes[index + 1] == b'n' {
            let start = index;
            let mut end = index + 2;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            let has_digits = end > start + 2;
            let left_boundary = start == 0 || !is_clif_ident_byte(bytes[start - 1]);
            let right_boundary = end >= bytes.len() || !is_clif_ident_byte(bytes[end]);
            if has_digits && left_boundary && right_boundary {
                let token = &clif[start..end];
                if let Some(alias) = import_aliases.get(token) {
                    out.push_str(alias);
                    index = end;
                    continue;
                }
            }
        }
        out.push(bytes[index] as char);
        index += 1;
    }
    out
}

pub fn run_cranelift_smoke(module: &BlockPyModule<PreparedBbBlockPyPass>) -> Result<(), String> {
    let function_count = module.callable_defs.len() as i64;
    let block_count = module
        .callable_defs
        .iter()
        .map(|f| f.blocks.len() as i64)
        .sum::<i64>();
    let sentinel = (function_count << 32) ^ block_count;

    let mut jit_module = new_jit_module()?;
    let mut ctx = jit_module.make_context();
    ctx.func
        .signature
        .returns
        .push(ir::AbiParam::new(ir::types::I64));
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

    let function_id = declare_local_fn(&mut jit_module, "dp_jit_smoke", &ctx.func.signature)?;
    define_function_with_incremental_cache(
        &mut jit_module,
        function_id,
        &mut ctx,
        "failed to define Cranelift function",
    )?;
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

fn build_cranelift_run_bb_specialized_function(
    jit_module: &mut JITModule,
    blocks: &[ObjPtr],
    plan: &ClifPlan,
    globals_obj: ObjPtr,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
    none_obj: ObjPtr,
    deleted_obj: ObjPtr,
    empty_tuple_obj: ObjPtr,
) -> Result<
    (
        cranelift_codegen::Context,
        cranelift_module::FuncId,
        Vec<Box<[u8]>>,
        HashMap<u32, &'static str>,
    ),
    String,
> {
    let block_count = plan.blocks.len();
    if block_count == 0 {
        return Err(format!("specialized JIT run_bb plan has no blocks"));
    }
    let has_generic_blocks = plan
        .blocks
        .iter()
        .any(|block| matches!(block.fast_path, BlockFastPath::None));
    if has_generic_blocks {
        return Err(
            "specialized JIT requires fully lowered fastpath blocks (no BlockFastPath::None)"
                .to_string(),
        );
    }
    if !blocks.is_empty() && blocks.len() != block_count {
        return Err(format!(
            "specialized JIT block table length mismatch: {} != {}",
            blocks.len(),
            block_count
        ));
    }

    let ptr_ty = jit_module.target_config().pointer_type();
    let i64_ty = ir::types::I64;
    let i32_ty = ir::types::I32;
    let mut module_imports = ModuleFuncImports::new();

    let mut incref_sig = jit_module.make_signature();
    incref_sig.params.push(ir::AbiParam::new(ptr_ty));
    let mut decref_sig = jit_module.make_signature();
    decref_sig.params.push(ir::AbiParam::new(ptr_ty));

    let mut py_call_sig = jit_module.make_signature();
    py_call_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut py_call_object_sig = jit_module.make_signature();
    py_call_object_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_object_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_object_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut py_call_with_kw_sig = jit_module.make_signature();
    py_call_with_kw_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_with_kw_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_with_kw_sig.params.push(ir::AbiParam::new(ptr_ty));
    py_call_with_kw_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut py_get_raised_exc_sig = jit_module.make_signature();
    py_get_raised_exc_sig
        .returns
        .push(ir::AbiParam::new(ptr_ty));

    let mut get_arg_item_sig = jit_module.make_signature();
    get_arg_item_sig.params.push(ir::AbiParam::new(ptr_ty));
    get_arg_item_sig.params.push(ir::AbiParam::new(i64_ty));
    get_arg_item_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut make_int_sig = jit_module.make_signature();
    make_int_sig.params.push(ir::AbiParam::new(i64_ty));
    make_int_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut make_float_sig = jit_module.make_signature();
    make_float_sig
        .params
        .push(ir::AbiParam::new(ir::types::F64));
    make_float_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut make_bytes_sig = jit_module.make_signature();
    make_bytes_sig.params.push(ir::AbiParam::new(ptr_ty));
    make_bytes_sig.params.push(ir::AbiParam::new(i64_ty));
    make_bytes_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut load_name_sig = jit_module.make_signature();
    load_name_sig.params.push(ir::AbiParam::new(ptr_ty));
    load_name_sig.params.push(ir::AbiParam::new(ptr_ty));
    load_name_sig.params.push(ir::AbiParam::new(i64_ty));
    load_name_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut load_local_raw_by_name_sig = jit_module.make_signature();
    load_local_raw_by_name_sig
        .params
        .push(ir::AbiParam::new(ptr_ty));
    load_local_raw_by_name_sig
        .params
        .push(ir::AbiParam::new(ptr_ty));
    load_local_raw_by_name_sig
        .params
        .push(ir::AbiParam::new(i64_ty));
    load_local_raw_by_name_sig
        .returns
        .push(ir::AbiParam::new(ptr_ty));

    let mut pyobject_getattr_sig = jit_module.make_signature();
    pyobject_getattr_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_getattr_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_getattr_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut pyobject_setattr_sig = jit_module.make_signature();
    pyobject_setattr_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_setattr_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_setattr_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_setattr_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut pyobject_getitem_sig = jit_module.make_signature();
    pyobject_getitem_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_getitem_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_getitem_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut pyobject_setitem_sig = jit_module.make_signature();
    pyobject_setitem_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_setitem_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_setitem_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_setitem_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut pyobject_to_i64_sig = jit_module.make_signature();
    pyobject_to_i64_sig.params.push(ir::AbiParam::new(ptr_ty));
    pyobject_to_i64_sig.returns.push(ir::AbiParam::new(i64_ty));

    let mut decode_literal_bytes_sig = jit_module.make_signature();
    decode_literal_bytes_sig
        .params
        .push(ir::AbiParam::new(ptr_ty));
    decode_literal_bytes_sig
        .params
        .push(ir::AbiParam::new(i64_ty));
    decode_literal_bytes_sig
        .returns
        .push(ir::AbiParam::new(ptr_ty));

    let mut load_deleted_name_sig = jit_module.make_signature();
    load_deleted_name_sig.params.push(ir::AbiParam::new(ptr_ty));
    load_deleted_name_sig.params.push(ir::AbiParam::new(i64_ty));
    load_deleted_name_sig.params.push(ir::AbiParam::new(ptr_ty));
    load_deleted_name_sig.params.push(ir::AbiParam::new(ptr_ty));
    load_deleted_name_sig
        .returns
        .push(ir::AbiParam::new(ptr_ty));

    let mut make_cell_sig = jit_module.make_signature();
    make_cell_sig.params.push(ir::AbiParam::new(ptr_ty));
    make_cell_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut load_cell_sig = jit_module.make_signature();
    load_cell_sig.params.push(ir::AbiParam::new(ptr_ty));
    load_cell_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut store_cell_sig = jit_module.make_signature();
    store_cell_sig.params.push(ir::AbiParam::new(ptr_ty));
    store_cell_sig.params.push(ir::AbiParam::new(ptr_ty));
    store_cell_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut store_cell_if_not_deleted_sig = jit_module.make_signature();
    store_cell_if_not_deleted_sig
        .params
        .push(ir::AbiParam::new(ptr_ty));
    store_cell_if_not_deleted_sig
        .params
        .push(ir::AbiParam::new(ptr_ty));
    store_cell_if_not_deleted_sig
        .params
        .push(ir::AbiParam::new(ptr_ty));
    store_cell_if_not_deleted_sig
        .returns
        .push(ir::AbiParam::new(ptr_ty));

    let mut tuple_new_sig = jit_module.make_signature();
    tuple_new_sig.params.push(ir::AbiParam::new(i64_ty));
    tuple_new_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut tuple_set_item_sig = jit_module.make_signature();
    tuple_set_item_sig.params.push(ir::AbiParam::new(ptr_ty));
    tuple_set_item_sig.params.push(ir::AbiParam::new(i64_ty));
    tuple_set_item_sig.params.push(ir::AbiParam::new(ptr_ty));
    tuple_set_item_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut is_true_sig = jit_module.make_signature();
    is_true_sig.params.push(ir::AbiParam::new(ptr_ty));
    is_true_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut unary_obj_sig = jit_module.make_signature();
    unary_obj_sig.params.push(ir::AbiParam::new(ptr_ty));
    unary_obj_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut unary_i32_sig = jit_module.make_signature();
    unary_i32_sig.params.push(ir::AbiParam::new(ptr_ty));
    unary_i32_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut binary_obj_sig = jit_module.make_signature();
    binary_obj_sig.params.push(ir::AbiParam::new(ptr_ty));
    binary_obj_sig.params.push(ir::AbiParam::new(ptr_ty));
    binary_obj_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut binary_i32_sig = jit_module.make_signature();
    binary_i32_sig.params.push(ir::AbiParam::new(ptr_ty));
    binary_i32_sig.params.push(ir::AbiParam::new(ptr_ty));
    binary_i32_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut ternary_obj_sig = jit_module.make_signature();
    ternary_obj_sig.params.push(ir::AbiParam::new(ptr_ty));
    ternary_obj_sig.params.push(ir::AbiParam::new(ptr_ty));
    ternary_obj_sig.params.push(ir::AbiParam::new(ptr_ty));
    ternary_obj_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut richcompare_sig = jit_module.make_signature();
    richcompare_sig.params.push(ir::AbiParam::new(ptr_ty));
    richcompare_sig.params.push(ir::AbiParam::new(ptr_ty));
    richcompare_sig.params.push(ir::AbiParam::new(i32_ty));
    richcompare_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut raise_exc_sig = jit_module.make_signature();
    raise_exc_sig.params.push(ir::AbiParam::new(ptr_ty));
    raise_exc_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut main_sig = jit_module.make_signature();
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let incref_id = declare_import_fn(jit_module, "dp_jit_incref", &incref_sig)?;
    let decref_id = declare_import_fn(jit_module, "dp_jit_decref", &decref_sig)?;
    let py_call_id = declare_import_fn(jit_module, "PyObject_CallFunctionObjArgs", &py_call_sig)?;
    let py_call_object_id =
        declare_import_fn(jit_module, "PyObject_CallObject", &py_call_object_sig)?;
    let py_call_with_kw_id =
        declare_import_fn(jit_module, "dp_jit_py_call_with_kw", &py_call_with_kw_sig)?;
    let py_get_raised_exc_id = declare_import_fn(
        jit_module,
        "PyErr_GetRaisedException",
        &py_get_raised_exc_sig,
    )?;
    let get_arg_item_id = declare_import_fn(jit_module, "dp_jit_get_arg_item", &get_arg_item_sig)?;
    let make_int_id = declare_import_fn(jit_module, "dp_jit_make_int", &make_int_sig)?;
    let make_float_id = declare_import_fn(jit_module, "dp_jit_make_float", &make_float_sig)?;
    let make_bytes_id = declare_import_fn(jit_module, "dp_jit_make_bytes", &make_bytes_sig)?;
    let load_name_id = declare_import_fn(jit_module, "dp_jit_load_name", &load_name_sig)?;
    let load_local_raw_by_name_id = declare_import_fn(
        jit_module,
        "dp_jit_load_local_raw_by_name",
        &load_local_raw_by_name_sig,
    )?;
    let pyobject_getattr_id =
        declare_import_fn(jit_module, "dp_jit_pyobject_getattr", &pyobject_getattr_sig)?;
    let pyobject_setattr_id =
        declare_import_fn(jit_module, "dp_jit_pyobject_setattr", &pyobject_setattr_sig)?;
    let pyobject_getitem_id =
        declare_import_fn(jit_module, "dp_jit_pyobject_getitem", &pyobject_getitem_sig)?;
    let pyobject_setitem_id =
        declare_import_fn(jit_module, "dp_jit_pyobject_setitem", &pyobject_setitem_sig)?;
    let pyobject_to_i64_id =
        declare_import_fn(jit_module, "dp_jit_pyobject_to_i64", &pyobject_to_i64_sig)?;
    let decode_literal_bytes_id = declare_import_fn(
        jit_module,
        "dp_jit_decode_literal_bytes",
        &decode_literal_bytes_sig,
    )?;
    let load_deleted_name_id = declare_import_fn(
        jit_module,
        "dp_jit_load_deleted_name",
        &load_deleted_name_sig,
    )?;
    let make_cell_id = declare_import_fn(jit_module, "dp_jit_make_cell", &make_cell_sig)?;
    let load_cell_id = declare_import_fn(jit_module, "dp_jit_load_cell", &load_cell_sig)?;
    let store_cell_id = declare_import_fn(jit_module, "dp_jit_store_cell", &store_cell_sig)?;
    let store_cell_if_not_deleted_id = declare_import_fn(
        jit_module,
        "dp_jit_store_cell_if_not_deleted",
        &store_cell_if_not_deleted_sig,
    )?;
    let tuple_new_id = declare_import_fn(jit_module, "dp_jit_tuple_new", &tuple_new_sig)?;
    let tuple_set_item_id =
        declare_import_fn(jit_module, "dp_jit_tuple_set_item", &tuple_set_item_sig)?;
    let is_true_id = declare_import_fn(jit_module, "dp_jit_is_true", &is_true_sig)?;
    let pyobject_richcompare_id =
        declare_import_fn(jit_module, "PyObject_RichCompare", &richcompare_sig)?;
    let pysequence_contains_id =
        declare_import_fn(jit_module, "PySequence_Contains", &binary_i32_sig)?;
    let pyobject_not_id = declare_import_fn(jit_module, "PyObject_Not", &unary_i32_sig)?;
    let pyobject_is_true_id = declare_import_fn(jit_module, "PyObject_IsTrue", &unary_i32_sig)?;
    let pynumber_subtract_id = declare_import_fn(jit_module, "PyNumber_Subtract", &binary_obj_sig)?;
    let pynumber_multiply_id = declare_import_fn(jit_module, "PyNumber_Multiply", &binary_obj_sig)?;
    let pynumber_matrix_multiply_id =
        declare_import_fn(jit_module, "PyNumber_MatrixMultiply", &binary_obj_sig)?;
    let pynumber_true_divide_id =
        declare_import_fn(jit_module, "PyNumber_TrueDivide", &binary_obj_sig)?;
    let pynumber_floor_divide_id =
        declare_import_fn(jit_module, "PyNumber_FloorDivide", &binary_obj_sig)?;
    let pynumber_remainder_id =
        declare_import_fn(jit_module, "PyNumber_Remainder", &binary_obj_sig)?;
    let pynumber_power_id = declare_import_fn(jit_module, "PyNumber_Power", &ternary_obj_sig)?;
    let pynumber_lshift_id = declare_import_fn(jit_module, "PyNumber_Lshift", &binary_obj_sig)?;
    let pynumber_rshift_id = declare_import_fn(jit_module, "PyNumber_Rshift", &binary_obj_sig)?;
    let pynumber_or_id = declare_import_fn(jit_module, "PyNumber_Or", &binary_obj_sig)?;
    let pynumber_xor_id = declare_import_fn(jit_module, "PyNumber_Xor", &binary_obj_sig)?;
    let pynumber_and_id = declare_import_fn(jit_module, "PyNumber_And", &binary_obj_sig)?;
    let pynumber_inplace_add_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceAdd", &binary_obj_sig)?;
    let pynumber_inplace_subtract_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceSubtract", &binary_obj_sig)?;
    let pynumber_inplace_multiply_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceMultiply", &binary_obj_sig)?;
    let pynumber_inplace_matrix_multiply_id = declare_import_fn(
        jit_module,
        "PyNumber_InPlaceMatrixMultiply",
        &binary_obj_sig,
    )?;
    let pynumber_inplace_true_divide_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceTrueDivide", &binary_obj_sig)?;
    let pynumber_inplace_floor_divide_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceFloorDivide", &binary_obj_sig)?;
    let pynumber_inplace_remainder_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceRemainder", &binary_obj_sig)?;
    let pynumber_inplace_power_id =
        declare_import_fn(jit_module, "PyNumber_InPlacePower", &ternary_obj_sig)?;
    let pynumber_inplace_lshift_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceLshift", &binary_obj_sig)?;
    let pynumber_inplace_rshift_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceRshift", &binary_obj_sig)?;
    let pynumber_inplace_or_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceOr", &binary_obj_sig)?;
    let pynumber_inplace_xor_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceXor", &binary_obj_sig)?;
    let pynumber_inplace_and_id =
        declare_import_fn(jit_module, "PyNumber_InPlaceAnd", &binary_obj_sig)?;
    let pynumber_positive_id = declare_import_fn(jit_module, "PyNumber_Positive", &unary_obj_sig)?;
    let pynumber_negative_id = declare_import_fn(jit_module, "PyNumber_Negative", &unary_obj_sig)?;
    let pynumber_invert_id = declare_import_fn(jit_module, "PyNumber_Invert", &unary_obj_sig)?;
    let raise_exc_id = declare_import_fn(jit_module, "dp_jit_raise_from_exc", &raise_exc_sig)?;
    let main_id = declare_local_fn(jit_module, "dp_jit_run_bb_specialized", &main_sig)?;
    let mut import_id_to_symbol: HashMap<u32, &'static str> =
        module_imports.debug_symbols().clone();
    import_id_to_symbol.insert(incref_id.as_u32(), "dp_jit_incref");
    import_id_to_symbol.insert(decref_id.as_u32(), "dp_jit_decref");
    import_id_to_symbol.insert(py_call_id.as_u32(), "PyObject_CallFunctionObjArgs");
    import_id_to_symbol.insert(py_call_object_id.as_u32(), "PyObject_CallObject");
    import_id_to_symbol.insert(py_call_with_kw_id.as_u32(), "dp_jit_py_call_with_kw");
    import_id_to_symbol.insert(py_get_raised_exc_id.as_u32(), "PyErr_GetRaisedException");
    import_id_to_symbol.insert(get_arg_item_id.as_u32(), "dp_jit_get_arg_item");
    import_id_to_symbol.insert(make_int_id.as_u32(), "dp_jit_make_int");
    import_id_to_symbol.insert(make_float_id.as_u32(), "dp_jit_make_float");
    import_id_to_symbol.insert(make_bytes_id.as_u32(), "dp_jit_make_bytes");
    import_id_to_symbol.insert(load_name_id.as_u32(), "dp_jit_load_name");
    import_id_to_symbol.insert(
        load_local_raw_by_name_id.as_u32(),
        "dp_jit_load_local_raw_by_name",
    );
    import_id_to_symbol.insert(pyobject_getattr_id.as_u32(), "dp_jit_pyobject_getattr");
    import_id_to_symbol.insert(pyobject_setattr_id.as_u32(), "dp_jit_pyobject_setattr");
    import_id_to_symbol.insert(pyobject_getitem_id.as_u32(), "dp_jit_pyobject_getitem");
    import_id_to_symbol.insert(pyobject_setitem_id.as_u32(), "dp_jit_pyobject_setitem");
    import_id_to_symbol.insert(pyobject_to_i64_id.as_u32(), "dp_jit_pyobject_to_i64");
    import_id_to_symbol.insert(
        decode_literal_bytes_id.as_u32(),
        "dp_jit_decode_literal_bytes",
    );
    import_id_to_symbol.insert(load_deleted_name_id.as_u32(), "dp_jit_load_deleted_name");
    import_id_to_symbol.insert(make_cell_id.as_u32(), "dp_jit_make_cell");
    import_id_to_symbol.insert(load_cell_id.as_u32(), "dp_jit_load_cell");
    import_id_to_symbol.insert(store_cell_id.as_u32(), "dp_jit_store_cell");
    import_id_to_symbol.insert(
        store_cell_if_not_deleted_id.as_u32(),
        "dp_jit_store_cell_if_not_deleted",
    );
    import_id_to_symbol.insert(tuple_new_id.as_u32(), "dp_jit_tuple_new");
    import_id_to_symbol.insert(tuple_set_item_id.as_u32(), "dp_jit_tuple_set_item");
    import_id_to_symbol.insert(is_true_id.as_u32(), "dp_jit_is_true");
    import_id_to_symbol.insert(pyobject_richcompare_id.as_u32(), "PyObject_RichCompare");
    import_id_to_symbol.insert(pysequence_contains_id.as_u32(), "PySequence_Contains");
    import_id_to_symbol.insert(pyobject_not_id.as_u32(), "PyObject_Not");
    import_id_to_symbol.insert(pyobject_is_true_id.as_u32(), "PyObject_IsTrue");
    import_id_to_symbol.insert(pynumber_subtract_id.as_u32(), "PyNumber_Subtract");
    import_id_to_symbol.insert(pynumber_multiply_id.as_u32(), "PyNumber_Multiply");
    import_id_to_symbol.insert(
        pynumber_matrix_multiply_id.as_u32(),
        "PyNumber_MatrixMultiply",
    );
    import_id_to_symbol.insert(pynumber_true_divide_id.as_u32(), "PyNumber_TrueDivide");
    import_id_to_symbol.insert(pynumber_floor_divide_id.as_u32(), "PyNumber_FloorDivide");
    import_id_to_symbol.insert(pynumber_remainder_id.as_u32(), "PyNumber_Remainder");
    import_id_to_symbol.insert(pynumber_power_id.as_u32(), "PyNumber_Power");
    import_id_to_symbol.insert(pynumber_lshift_id.as_u32(), "PyNumber_Lshift");
    import_id_to_symbol.insert(pynumber_rshift_id.as_u32(), "PyNumber_Rshift");
    import_id_to_symbol.insert(pynumber_or_id.as_u32(), "PyNumber_Or");
    import_id_to_symbol.insert(pynumber_xor_id.as_u32(), "PyNumber_Xor");
    import_id_to_symbol.insert(pynumber_and_id.as_u32(), "PyNumber_And");
    import_id_to_symbol.insert(pynumber_inplace_add_id.as_u32(), "PyNumber_InPlaceAdd");
    import_id_to_symbol.insert(
        pynumber_inplace_subtract_id.as_u32(),
        "PyNumber_InPlaceSubtract",
    );
    import_id_to_symbol.insert(
        pynumber_inplace_multiply_id.as_u32(),
        "PyNumber_InPlaceMultiply",
    );
    import_id_to_symbol.insert(
        pynumber_inplace_matrix_multiply_id.as_u32(),
        "PyNumber_InPlaceMatrixMultiply",
    );
    import_id_to_symbol.insert(
        pynumber_inplace_true_divide_id.as_u32(),
        "PyNumber_InPlaceTrueDivide",
    );
    import_id_to_symbol.insert(
        pynumber_inplace_floor_divide_id.as_u32(),
        "PyNumber_InPlaceFloorDivide",
    );
    import_id_to_symbol.insert(
        pynumber_inplace_remainder_id.as_u32(),
        "PyNumber_InPlaceRemainder",
    );
    import_id_to_symbol.insert(pynumber_inplace_power_id.as_u32(), "PyNumber_InPlacePower");
    import_id_to_symbol.insert(
        pynumber_inplace_lshift_id.as_u32(),
        "PyNumber_InPlaceLshift",
    );
    import_id_to_symbol.insert(
        pynumber_inplace_rshift_id.as_u32(),
        "PyNumber_InPlaceRshift",
    );
    import_id_to_symbol.insert(pynumber_inplace_or_id.as_u32(), "PyNumber_InPlaceOr");
    import_id_to_symbol.insert(pynumber_inplace_xor_id.as_u32(), "PyNumber_InPlaceXor");
    import_id_to_symbol.insert(pynumber_inplace_and_id.as_u32(), "PyNumber_InPlaceAnd");
    import_id_to_symbol.insert(pynumber_positive_id.as_u32(), "PyNumber_Positive");
    import_id_to_symbol.insert(pynumber_negative_id.as_u32(), "PyNumber_Negative");
    import_id_to_symbol.insert(pynumber_invert_id.as_u32(), "PyNumber_Invert");
    import_id_to_symbol.insert(raise_exc_id.as_u32(), "dp_jit_raise_from_exc");

    let mut ctx = jit_module.make_context();
    let mut literal_pool: Vec<Box<[u8]>> = Vec::new();
    ctx.func.signature = main_sig;
    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry_block = fb.create_block();
        let mut exec_blocks = Vec::with_capacity(block_count);
        let runtime_block_param_names = plan
            .blocks
            .iter()
            .map(|block| {
                block
                    .param_names
                    .iter()
                    .filter(|name| {
                        !plan
                            .ambient_param_names
                            .iter()
                            .any(|ambient| ambient == *name)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut cleanup_null_blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            exec_blocks.push(fb.create_block());
            cleanup_null_blocks.push(fb.create_block());
        }
        let step_null_block = fb.create_block();
        let raise_exc_direct_block = fb.create_block();
        let function_state_slots = FunctionStateSlots::new(&mut fb, &plan.slot_names);

        fb.append_block_params_for_function_params(entry_block);
        for (index, block) in exec_blocks.iter().enumerate() {
            for _ in &runtime_block_param_names[index] {
                fb.append_block_param(*block, ptr_ty);
            }
            for _ in &runtime_block_param_names[index] {
                fb.append_block_param(cleanup_null_blocks[index], ptr_ty);
            }
        }
        fb.append_block_param(step_null_block, ptr_ty); // args
        fb.append_block_param(raise_exc_direct_block, ptr_ty); // args
        fb.append_block_param(raise_exc_direct_block, ptr_ty); // exc

        fb.switch_to_block(entry_block);
        let entry_args = fb.block_params(entry_block)[0];
        let ambient_args = fb.block_params(entry_block)[1];
        let mut func_imports = FuncBuildImports::new(&mut module_imports);
        let incref_ref = jit_module.declare_func_in_func(incref_id, &mut fb.func);
        let decref_ref = jit_module.declare_func_in_func(decref_id, &mut fb.func);
        let py_call_ref = jit_module.declare_func_in_func(py_call_id, &mut fb.func);
        let py_call_object_ref = jit_module.declare_func_in_func(py_call_object_id, &mut fb.func);
        let py_call_with_kw_ref = jit_module.declare_func_in_func(py_call_with_kw_id, &mut fb.func);
        let py_get_raised_exc_ref =
            jit_module.declare_func_in_func(py_get_raised_exc_id, &mut fb.func);
        let get_arg_item_ref = jit_module.declare_func_in_func(get_arg_item_id, &mut fb.func);
        let make_int_ref = jit_module.declare_func_in_func(make_int_id, &mut fb.func);
        let is_true_ref = jit_module.declare_func_in_func(is_true_id, &mut fb.func);
        let raise_exc_ref = jit_module.declare_func_in_func(raise_exc_id, &mut fb.func);
        let make_float_ref = jit_module.declare_func_in_func(make_float_id, &mut fb.func);
        let load_name_ref = jit_module.declare_func_in_func(load_name_id, &mut fb.func);
        let load_local_raw_by_name_ref =
            jit_module.declare_func_in_func(load_local_raw_by_name_id, &mut fb.func);
        let pyobject_getattr_ref =
            jit_module.declare_func_in_func(pyobject_getattr_id, &mut fb.func);
        let pyobject_setattr_ref =
            jit_module.declare_func_in_func(pyobject_setattr_id, &mut fb.func);
        let pyobject_getitem_ref =
            jit_module.declare_func_in_func(pyobject_getitem_id, &mut fb.func);
        let pyobject_setitem_ref =
            jit_module.declare_func_in_func(pyobject_setitem_id, &mut fb.func);
        let pyobject_to_i64_ref = jit_module.declare_func_in_func(pyobject_to_i64_id, &mut fb.func);
        let decode_literal_bytes_ref =
            jit_module.declare_func_in_func(decode_literal_bytes_id, &mut fb.func);
        let load_deleted_name_ref =
            jit_module.declare_func_in_func(load_deleted_name_id, &mut fb.func);
        let make_cell_ref = jit_module.declare_func_in_func(make_cell_id, &mut fb.func);
        let load_cell_ref = jit_module.declare_func_in_func(load_cell_id, &mut fb.func);
        let store_cell_ref = jit_module.declare_func_in_func(store_cell_id, &mut fb.func);
        let store_cell_if_not_deleted_ref =
            jit_module.declare_func_in_func(store_cell_if_not_deleted_id, &mut fb.func);
        let make_bytes_ref = jit_module.declare_func_in_func(make_bytes_id, &mut fb.func);
        let tuple_new_ref = jit_module.declare_func_in_func(tuple_new_id, &mut fb.func);
        let tuple_set_item_ref = jit_module.declare_func_in_func(tuple_set_item_id, &mut fb.func);
        let pyobject_richcompare_ref =
            jit_module.declare_func_in_func(pyobject_richcompare_id, &mut fb.func);
        let pysequence_contains_ref =
            jit_module.declare_func_in_func(pysequence_contains_id, &mut fb.func);
        let pyobject_not_ref = jit_module.declare_func_in_func(pyobject_not_id, &mut fb.func);
        let pyobject_is_true_ref =
            jit_module.declare_func_in_func(pyobject_is_true_id, &mut fb.func);
        let pynumber_subtract_ref =
            jit_module.declare_func_in_func(pynumber_subtract_id, &mut fb.func);
        let pynumber_multiply_ref =
            jit_module.declare_func_in_func(pynumber_multiply_id, &mut fb.func);
        let pynumber_matrix_multiply_ref =
            jit_module.declare_func_in_func(pynumber_matrix_multiply_id, &mut fb.func);
        let pynumber_true_divide_ref =
            jit_module.declare_func_in_func(pynumber_true_divide_id, &mut fb.func);
        let pynumber_floor_divide_ref =
            jit_module.declare_func_in_func(pynumber_floor_divide_id, &mut fb.func);
        let pynumber_remainder_ref =
            jit_module.declare_func_in_func(pynumber_remainder_id, &mut fb.func);
        let pynumber_power_ref = jit_module.declare_func_in_func(pynumber_power_id, &mut fb.func);
        let pynumber_lshift_ref = jit_module.declare_func_in_func(pynumber_lshift_id, &mut fb.func);
        let pynumber_rshift_ref = jit_module.declare_func_in_func(pynumber_rshift_id, &mut fb.func);
        let pynumber_or_ref = jit_module.declare_func_in_func(pynumber_or_id, &mut fb.func);
        let pynumber_xor_ref = jit_module.declare_func_in_func(pynumber_xor_id, &mut fb.func);
        let pynumber_and_ref = jit_module.declare_func_in_func(pynumber_and_id, &mut fb.func);
        let pynumber_inplace_add_ref =
            jit_module.declare_func_in_func(pynumber_inplace_add_id, &mut fb.func);
        let pynumber_inplace_subtract_ref =
            jit_module.declare_func_in_func(pynumber_inplace_subtract_id, &mut fb.func);
        let pynumber_inplace_multiply_ref =
            jit_module.declare_func_in_func(pynumber_inplace_multiply_id, &mut fb.func);
        let pynumber_inplace_matrix_multiply_ref =
            jit_module.declare_func_in_func(pynumber_inplace_matrix_multiply_id, &mut fb.func);
        let pynumber_inplace_true_divide_ref =
            jit_module.declare_func_in_func(pynumber_inplace_true_divide_id, &mut fb.func);
        let pynumber_inplace_floor_divide_ref =
            jit_module.declare_func_in_func(pynumber_inplace_floor_divide_id, &mut fb.func);
        let pynumber_inplace_remainder_ref =
            jit_module.declare_func_in_func(pynumber_inplace_remainder_id, &mut fb.func);
        let pynumber_inplace_power_ref =
            jit_module.declare_func_in_func(pynumber_inplace_power_id, &mut fb.func);
        let pynumber_inplace_lshift_ref =
            jit_module.declare_func_in_func(pynumber_inplace_lshift_id, &mut fb.func);
        let pynumber_inplace_rshift_ref =
            jit_module.declare_func_in_func(pynumber_inplace_rshift_id, &mut fb.func);
        let pynumber_inplace_or_ref =
            jit_module.declare_func_in_func(pynumber_inplace_or_id, &mut fb.func);
        let pynumber_inplace_xor_ref =
            jit_module.declare_func_in_func(pynumber_inplace_xor_id, &mut fb.func);
        let pynumber_inplace_and_ref =
            jit_module.declare_func_in_func(pynumber_inplace_and_id, &mut fb.func);
        let pynumber_positive_ref =
            jit_module.declare_func_in_func(pynumber_positive_id, &mut fb.func);
        let pynumber_negative_ref =
            jit_module.declare_func_in_func(pynumber_negative_id, &mut fb.func);
        let pynumber_invert_ref = jit_module.declare_func_in_func(pynumber_invert_id, &mut fb.func);

        let entry_deleted_const = fb.ins().iconst(ptr_ty, deleted_obj as i64);
        function_state_slots.initialize_all_to_value(&mut fb, entry_deleted_const, incref_ref);

        let ambient_names = plan.ambient_param_names.clone();
        let mut ambient_values = Vec::with_capacity(plan.ambient_param_names.len());
        let mut entry_jump_args = Vec::with_capacity(runtime_block_param_names[0].len());
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        for (param_index, param_name) in plan.ambient_param_names.iter().enumerate() {
            let index_val = fb.ins().iconst(i64_ty, param_index as i64);
            let item_inst = fb.ins().call(get_arg_item_ref, &[ambient_args, index_val]);
            let item_val = fb.inst_results(item_inst)[0];
            let is_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, item_val, null_ptr);
            let ok_block = fb.create_block();
            fb.append_block_param(ok_block, ptr_ty);
            fb.ins().brif(
                is_null,
                step_null_block,
                &[ir::BlockArg::Value(entry_args)],
                ok_block,
                &[ir::BlockArg::Value(item_val)],
            );
            fb.switch_to_block(ok_block);
            let value = fb.block_params(ok_block)[0];
            function_state_slots
                .replace_cloned_value(&mut fb, param_name, value, ptr_ty, incref_ref, decref_ref)
                .expect("ambient slot missing from function state slots");
            ambient_values.push(value);
        }
        for (param_index, param_name) in plan.blocks[0].param_names.iter().enumerate() {
            if plan
                .ambient_param_names
                .iter()
                .any(|ambient| ambient == param_name)
            {
                continue;
            }
            let index_val = fb.ins().iconst(i64_ty, param_index as i64);
            let item_inst = fb.ins().call(get_arg_item_ref, &[entry_args, index_val]);
            let item_val = fb.inst_results(item_inst)[0];
            let is_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, item_val, null_ptr);
            let ok_block = fb.create_block();
            fb.append_block_param(ok_block, ptr_ty);
            fb.ins().brif(
                is_null,
                step_null_block,
                &[ir::BlockArg::Value(entry_args)],
                ok_block,
                &[ir::BlockArg::Value(item_val)],
            );
            fb.switch_to_block(ok_block);
            let value = fb.block_params(ok_block)[0];
            function_state_slots
                .replace_cloned_value(&mut fb, param_name, value, ptr_ty, incref_ref, decref_ref)
                .expect("entry slot missing from function state slots");
            entry_jump_args.push(ir::BlockArg::Value(value));
        }
        fb.ins().jump(exec_blocks[0], &entry_jump_args);

        let mut exception_dispatch_blocks: Vec<Option<ir::Block>> = vec![None; exec_blocks.len()];
        for (index, block) in plan.blocks.iter().enumerate() {
            if block.exc_dispatch.is_some() {
                let dispatch_block = fb.create_block();
                for _ in &runtime_block_param_names[index] {
                    fb.append_block_param(dispatch_block, ptr_ty);
                }
                exception_dispatch_blocks[index] = Some(dispatch_block);
            }
        }

        for (index, block) in exec_blocks.iter().enumerate() {
            fb.switch_to_block(*block);
            let block_param_values = fb.block_params(*block).to_vec();
            let block_const = fb.ins().iconst(ptr_ty, globals_obj as i64);
            let none_const = fb.ins().iconst(ptr_ty, none_obj as i64);
            let true_const = fb.ins().iconst(ptr_ty, true_obj as i64);
            let false_const = fb.ins().iconst(ptr_ty, false_obj as i64);
            let deleted_const = fb.ins().iconst(ptr_ty, deleted_obj as i64);
            let empty_tuple_const = fb.ins().iconst(ptr_ty, empty_tuple_obj as i64);
            let fast_step_null_block =
                exception_dispatch_blocks[index].unwrap_or(cleanup_null_blocks[index]);
            let fast_step_null_args = block_param_values.clone();
            let emit_ctx = DirectSimpleEmitCtx {
                incref_ref,
                decref_ref,
                py_call_ref,
                make_int_ref,
                consts: DirectSimpleEmitConsts {
                    step_null_block: fast_step_null_block,
                    step_null_args: fast_step_null_args,
                    ptr_ty,
                    i64_ty,
                    none_const,
                    true_const,
                    false_const,
                    deleted_const,
                    empty_tuple_const,
                    block_const,
                },
                load_name_ref,
                load_local_raw_by_name_ref,
                pyobject_getattr_ref,
                pyobject_setattr_ref,
                pyobject_getitem_ref,
                pyobject_setitem_ref,
                decode_literal_bytes_ref,
                load_deleted_name_ref,
                make_cell_ref,
                load_cell_ref,
                store_cell_ref,
                store_cell_if_not_deleted_ref,
                make_bytes_ref,
                make_float_ref,
                py_call_object_ref,
                py_call_with_kw_ref,
                tuple_new_ref,
                tuple_set_item_ref,
                operator_refs: DirectSimpleOperatorRefs {
                    richcompare_ref: pyobject_richcompare_ref,
                    sequence_contains_ref: pysequence_contains_ref,
                    object_not_ref: pyobject_not_ref,
                    object_is_true_ref: pyobject_is_true_ref,
                    number_subtract_ref: pynumber_subtract_ref,
                    number_multiply_ref: pynumber_multiply_ref,
                    number_matrix_multiply_ref: pynumber_matrix_multiply_ref,
                    number_true_divide_ref: pynumber_true_divide_ref,
                    number_floor_divide_ref: pynumber_floor_divide_ref,
                    number_remainder_ref: pynumber_remainder_ref,
                    number_power_ref: pynumber_power_ref,
                    number_lshift_ref: pynumber_lshift_ref,
                    number_rshift_ref: pynumber_rshift_ref,
                    number_or_ref: pynumber_or_ref,
                    number_xor_ref: pynumber_xor_ref,
                    number_and_ref: pynumber_and_ref,
                    number_inplace_add_ref: pynumber_inplace_add_ref,
                    number_inplace_subtract_ref: pynumber_inplace_subtract_ref,
                    number_inplace_multiply_ref: pynumber_inplace_multiply_ref,
                    number_inplace_matrix_multiply_ref: pynumber_inplace_matrix_multiply_ref,
                    number_inplace_true_divide_ref: pynumber_inplace_true_divide_ref,
                    number_inplace_floor_divide_ref: pynumber_inplace_floor_divide_ref,
                    number_inplace_remainder_ref: pynumber_inplace_remainder_ref,
                    number_inplace_power_ref: pynumber_inplace_power_ref,
                    number_inplace_lshift_ref: pynumber_inplace_lshift_ref,
                    number_inplace_rshift_ref: pynumber_inplace_rshift_ref,
                    number_inplace_or_ref: pynumber_inplace_or_ref,
                    number_inplace_xor_ref: pynumber_inplace_xor_ref,
                    number_inplace_and_ref: pynumber_inplace_and_ref,
                    number_positive_ref: pynumber_positive_ref,
                    number_negative_ref: pynumber_negative_ref,
                    number_invert_ref: pynumber_invert_ref,
                },
                ambient_names: ambient_names.clone(),
                ambient_values: ambient_values.clone(),
            };
            match &plan.blocks[index].fast_path {
                BlockFastPath::JumpPassThrough { target_index } => {
                    let jump_args = block_param_values
                        .iter()
                        .copied()
                        .map(ir::BlockArg::Value)
                        .collect::<Vec<_>>();
                    fb.ins().jump(exec_blocks[*target_index], &jump_args);
                    continue;
                }
                BlockFastPath::DirectSimpleBrIf { plan } => {
                    let local_names = runtime_block_param_names[index].clone();
                    let local_values = block_param_values.clone();

                    let test_value = emit_direct_simple_expr(
                        &mut fb,
                        &plan.test,
                        &local_names,
                        &local_values,
                        &emit_ctx,
                        &mut literal_pool,
                        false,
                        jit_module,
                        &mut func_imports,
                    );
                    let truth_inst = fb.ins().call(is_true_ref, &[test_value]);
                    let truth_value = fb.inst_results(truth_inst)[0];
                    fb.ins().call(decref_ref, &[test_value]);
                    let truth_error = fb.ins().iconst(i32_ty, -1);
                    let is_error =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, truth_value, truth_error);
                    let truth_ok_block = fb.create_block();
                    fb.append_block_param(truth_ok_block, i32_ty);
                    fb.ins().brif(
                        is_error,
                        emit_ctx.consts.step_null_block,
                        &block_arg_values(&emit_ctx.consts.step_null_args),
                        truth_ok_block,
                        &[ir::BlockArg::Value(truth_value)],
                    );
                    fb.switch_to_block(truth_ok_block);
                    let truth_ok_value = fb.block_params(truth_ok_block)[0];
                    let zero_i32 = fb.ins().iconst(i32_ty, 0);
                    let is_true = fb.ins().icmp(
                        ir::condcodes::IntCC::SignedGreaterThan,
                        truth_ok_value,
                        zero_i32,
                    );
                    let pass_args = block_param_values
                        .iter()
                        .copied()
                        .map(ir::BlockArg::Value)
                        .collect::<Vec<_>>();
                    fb.ins().brif(
                        is_true,
                        exec_blocks[plan.then_index],
                        &pass_args,
                        exec_blocks[plan.else_index],
                        &pass_args,
                    );
                    continue;
                }
                BlockFastPath::DirectSimpleRet { plan } => {
                    let mut local_names = runtime_block_param_names[index].clone();
                    let mut local_values = block_param_values.clone();
                    let mut frame_locals_aliases: HashSet<String> = HashSet::new();
                    let null_ptr = fb.ins().iconst(ptr_ty, 0);

                    for assign in &plan.assigns {
                        let value_is_frame_locals =
                            direct_simple_expr_is_frame_locals_fetch(&assign.value)
                                || matches!(
                                    &assign.value,
                                    DirectSimpleExprPlan::Name(name)
                                        if frame_locals_aliases.contains(name)
                                );
                        let value = if let Some((obj_expr, key_expr, value_expr, key_name)) =
                            direct_simple_expr_as_frame_locals_setitem(
                                &assign.value,
                                &frame_locals_aliases,
                            ) {
                            let obj_borrowed =
                                direct_simple_expr_is_borrowable(obj_expr, &local_names);
                            let key_borrowed =
                                direct_simple_expr_is_borrowable(key_expr, &local_names);
                            let value_borrowed =
                                direct_simple_expr_is_borrowable(value_expr, &local_names);
                            let obj_value = emit_direct_simple_expr(
                                &mut fb,
                                obj_expr,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                obj_borrowed,
                                jit_module,
                                &mut func_imports,
                            );
                            let key_value = emit_direct_simple_expr(
                                &mut fb,
                                key_expr,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                key_borrowed,
                                jit_module,
                                &mut func_imports,
                            );
                            let value_value = emit_direct_simple_expr(
                                &mut fb,
                                value_expr,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                value_borrowed,
                                jit_module,
                                &mut func_imports,
                            );
                            let set_item_inst = fb
                                .ins()
                                .call(pyobject_setitem_ref, &[obj_value, key_value, value_value]);
                            let set_item_value = fb.inst_results(set_item_inst)[0];
                            let set_item_failed = fb.ins().icmp(
                                ir::condcodes::IntCC::Equal,
                                set_item_value,
                                null_ptr,
                            );
                            let set_item_ok = fb.create_block();
                            let set_item_fail = fb.create_block();
                            fb.append_block_param(set_item_ok, ptr_ty);
                            fb.append_block_param(set_item_fail, ptr_ty);
                            fb.ins().brif(
                                set_item_failed,
                                set_item_fail,
                                &[ir::BlockArg::Value(set_item_value)],
                                set_item_ok,
                                &[ir::BlockArg::Value(set_item_value)],
                            );
                            fb.switch_to_block(set_item_fail);
                            let failed_set_item_value = fb.block_params(set_item_fail)[0];
                            fb.ins().call(decref_ref, &[failed_set_item_value]);
                            fb.ins().jump(
                                emit_ctx.consts.step_null_block,
                                &step_null_block_args(&emit_ctx),
                            );
                            fb.switch_to_block(set_item_ok);
                            let set_item_value = fb.block_params(set_item_ok)[0];
                            let synced_inst =
                                fb.ins().call(pyobject_getitem_ref, &[obj_value, key_value]);
                            let synced_value = fb.inst_results(synced_inst)[0];
                            let synced_failed =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, synced_value, null_ptr);
                            let synced_ok = fb.create_block();
                            let synced_fail = fb.create_block();
                            fb.append_block_param(synced_ok, ptr_ty);
                            fb.append_block_param(synced_fail, ptr_ty);
                            fb.ins().brif(
                                synced_failed,
                                synced_fail,
                                &[ir::BlockArg::Value(synced_value)],
                                synced_ok,
                                &[ir::BlockArg::Value(synced_value)],
                            );
                            fb.switch_to_block(synced_fail);
                            let failed_synced_value = fb.block_params(synced_fail)[0];
                            fb.ins().call(decref_ref, &[failed_synced_value]);
                            fb.ins().jump(
                                emit_ctx.consts.step_null_block,
                                &step_null_block_args(&emit_ctx),
                            );
                            fb.switch_to_block(synced_ok);
                            let synced_value = fb.block_params(synced_ok)[0];
                            bind_local_value(
                                &mut fb,
                                &mut local_names,
                                &mut local_values,
                                key_name.as_str(),
                                synced_value,
                                &function_state_slots,
                                ptr_ty,
                                incref_ref,
                                decref_ref,
                            );
                            if !obj_borrowed {
                                fb.ins().call(decref_ref, &[obj_value]);
                            }
                            if !key_borrowed {
                                fb.ins().call(decref_ref, &[key_value]);
                            }
                            if !value_borrowed {
                                fb.ins().call(decref_ref, &[value_value]);
                            }
                            set_item_value
                        } else {
                            emit_direct_simple_expr(
                                &mut fb,
                                &assign.value,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                false,
                                jit_module,
                                &mut func_imports,
                            )
                        };

                        bind_local_value(
                            &mut fb,
                            &mut local_names,
                            &mut local_values,
                            assign.target.as_str(),
                            value,
                            &function_state_slots,
                            ptr_ty,
                            incref_ref,
                            decref_ref,
                        );
                        if value_is_frame_locals {
                            frame_locals_aliases.insert(assign.target.clone());
                        } else {
                            frame_locals_aliases.remove(assign.target.as_str());
                        }
                    }

                    let ret_value = emit_direct_simple_expr(
                        &mut fb,
                        &plan.ret,
                        &local_names,
                        &local_values,
                        &emit_ctx,
                        &mut literal_pool,
                        false,
                        jit_module,
                        &mut func_imports,
                    );

                    emit_decref_ambient_values(&mut fb, &emit_ctx);
                    for value in local_values {
                        fb.ins().call(decref_ref, &[value]);
                    }
                    function_state_slots.decref_all(&mut fb, ptr_ty, decref_ref);
                    fb.ins().return_(&[ret_value]);
                    continue;
                }
                BlockFastPath::DirectSimpleBlock { plan: block_plan } => {
                    let mut local_names = runtime_block_param_names[index].clone();
                    let mut local_values = block_param_values.clone();

                    emit_direct_simple_ops(
                        &mut fb,
                        &block_plan.ops,
                        &mut local_names,
                        &mut local_values,
                        &function_state_slots,
                        &emit_ctx,
                        &mut literal_pool,
                        jit_module,
                        &mut func_imports,
                    )?;

                    match &block_plan.term {
                        DirectSimpleTermPlan::Jump {
                            target_index,
                            target_params: _,
                            target_args,
                        } => {
                            let target_params = &runtime_block_param_names[*target_index];
                            let mut jump_args = Vec::with_capacity(target_params.len());
                            jump_args.extend(
                                emit_prepare_target_args(
                                    &mut fb,
                                    target_params,
                                    Some(target_args),
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    jit_module,
                                    &mut func_imports,
                                )
                                .ok_or_else(|| {
                                    format!(
                                        "missing local mapping for jump block params in block {}",
                                        plan.blocks[index].label
                                    )
                                })?,
                            );
                            emit_decref_unforwarded_locals(
                                &mut fb,
                                &local_values,
                                &local_names,
                                target_params,
                                decref_ref,
                            );
                            fb.ins().jump(exec_blocks[*target_index], &jump_args);
                        }
                        DirectSimpleTermPlan::BrIf {
                            test,
                            then_index,
                            then_params: _,
                            else_index,
                            else_params: _,
                        } => {
                            let test_value = emit_direct_simple_expr(
                                &mut fb,
                                test,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                false,
                                jit_module,
                                &mut func_imports,
                            );
                            let is_true = emit_truthy_from_owned(
                                &mut fb,
                                test_value,
                                is_true_ref,
                                decref_ref,
                                emit_ctx.consts.step_null_block,
                                &emit_ctx.consts.step_null_args,
                                i32_ty,
                            );

                            let then_branch = fb.create_block();
                            let else_branch = fb.create_block();
                            fb.ins().brif(is_true, then_branch, &[], else_branch, &[]);

                            fb.switch_to_block(then_branch);
                            let then_params = &runtime_block_param_names[*then_index];
                            let mut then_jump_args = Vec::with_capacity(then_params.len());
                            then_jump_args.extend(
                                emit_prepare_target_args(
                                    &mut fb,
                                    then_params,
                                    None,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    jit_module,
                                    &mut func_imports,
                                )
                                .ok_or_else(|| {
                                    format!(
                                        "missing local mapping for then-branch block params in block {}",
                                        plan.blocks[index].label
                                    )
                                })?,
                            );
                            emit_decref_unforwarded_locals(
                                &mut fb,
                                &local_values,
                                &local_names,
                                then_params,
                                decref_ref,
                            );
                            fb.ins().jump(exec_blocks[*then_index], &then_jump_args);

                            fb.switch_to_block(else_branch);
                            let else_params = &runtime_block_param_names[*else_index];
                            let mut else_jump_args = Vec::with_capacity(else_params.len());
                            else_jump_args.extend(
                                emit_prepare_target_args(
                                    &mut fb,
                                    else_params,
                                    None,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    jit_module,
                                    &mut func_imports,
                                )
                                .ok_or_else(|| {
                                    format!(
                                        "missing local mapping for else-branch block params in block {}",
                                        plan.blocks[index].label
                                    )
                                })?,
                            );
                            emit_decref_unforwarded_locals(
                                &mut fb,
                                &local_values,
                                &local_names,
                                else_params,
                                decref_ref,
                            );
                            fb.ins().jump(exec_blocks[*else_index], &else_jump_args);
                        }
                        DirectSimpleTermPlan::BrTable {
                            index: table_index_expr,
                            targets,
                            default_index,
                            default_params: _,
                        } => {
                            let index_obj = emit_direct_simple_expr(
                                &mut fb,
                                table_index_expr,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                false,
                                jit_module,
                                &mut func_imports,
                            );
                            let index_i64_inst = fb.ins().call(pyobject_to_i64_ref, &[index_obj]);
                            let index_i64 = fb.inst_results(index_i64_inst)[0];
                            fb.ins().call(decref_ref, &[index_obj]);
                            let index_error = fb.ins().iconst(i64_ty, i64::MIN);
                            let is_error =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, index_i64, index_error);
                            let dispatch_block = fb.create_block();
                            fb.append_block_param(dispatch_block, i64_ty);
                            fb.ins().brif(
                                is_error,
                                emit_ctx.consts.step_null_block,
                                &block_arg_values(&emit_ctx.consts.step_null_args),
                                dispatch_block,
                                &[ir::BlockArg::Value(index_i64)],
                            );

                            let default_block = fb.create_block();
                            let mut switch = Switch::new();
                            let mut case_blocks = Vec::with_capacity(targets.len());
                            for (case_index, _) in targets.iter().enumerate() {
                                let case_block = fb.create_block();
                                switch.set_entry(case_index as u128, case_block);
                                case_blocks.push(case_block);
                            }

                            fb.switch_to_block(dispatch_block);
                            let dispatch_value = fb.block_params(dispatch_block)[0];
                            switch.emit(&mut fb, dispatch_value, default_block);

                            for ((target_index, _), case_block) in
                                targets.iter().zip(case_blocks.iter())
                            {
                                fb.switch_to_block(*case_block);
                                let target_params = &runtime_block_param_names[*target_index];
                                let mut case_jump_args = Vec::with_capacity(target_params.len());
                                case_jump_args.extend(
                                    emit_prepare_target_args(
                                    &mut fb,
                                    target_params,
                                    None,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    jit_module,
                                    &mut func_imports,
                                    )
                                    .ok_or_else(|| {
                                        format!(
                                            "missing local mapping for br_table case block params in block {}",
                                            plan.blocks[index].label
                                        )
                                    })?,
                                );
                                emit_decref_unforwarded_locals(
                                    &mut fb,
                                    &local_values,
                                    &local_names,
                                    target_params,
                                    decref_ref,
                                );
                                fb.ins().jump(exec_blocks[*target_index], &case_jump_args);
                            }

                            fb.switch_to_block(default_block);
                            let default_params = &runtime_block_param_names[*default_index];
                            let mut default_jump_args = Vec::with_capacity(default_params.len());
                            default_jump_args.extend(
                                emit_prepare_target_args(
                                    &mut fb,
                                    default_params,
                                    None,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    jit_module,
                                    &mut func_imports,
                                )
                                .ok_or_else(|| {
                                    format!(
                                        "missing local mapping for br_table default block params in block {}",
                                        plan.blocks[index].label
                                    )
                                })?,
                            );
                            emit_decref_unforwarded_locals(
                                &mut fb,
                                &local_values,
                                &local_names,
                                default_params,
                                decref_ref,
                            );
                            fb.ins()
                                .jump(exec_blocks[*default_index], &default_jump_args);
                        }
                        DirectSimpleTermPlan::Ret { value } => {
                            let ret_value = emit_direct_simple_expr(
                                &mut fb,
                                value,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                false,
                                jit_module,
                                &mut func_imports,
                            );
                            emit_decref_ambient_values(&mut fb, &emit_ctx);
                            for value in &local_values {
                                fb.ins().call(decref_ref, &[*value]);
                            }
                            function_state_slots.decref_all(&mut fb, ptr_ty, decref_ref);
                            fb.ins().return_(&[ret_value]);
                        }
                        DirectSimpleTermPlan::Raise { exc } => {
                            let (raise_name_ptr, raise_name_len) =
                                intern_bytes_literal(&mut literal_pool, b"__dp_raise_from");
                            let raise_name_ptr_val = fb.ins().iconst(ptr_ty, raise_name_ptr as i64);
                            let raise_name_len_val = fb.ins().iconst(i64_ty, raise_name_len);
                            let raise_fn_inst = fb.ins().call(
                                load_name_ref,
                                &[block_const, raise_name_ptr_val, raise_name_len_val],
                            );
                            let raise_fn = fb.inst_results(raise_fn_inst)[0];
                            let raise_fn_null =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, raise_fn, null_ptr);
                            let raise_fn_ok = fb.create_block();
                            fb.append_block_param(raise_fn_ok, ptr_ty);
                            fb.ins().brif(
                                raise_fn_null,
                                emit_ctx.consts.step_null_block,
                                &step_null_block_args(&emit_ctx),
                                raise_fn_ok,
                                &[ir::BlockArg::Value(raise_fn)],
                            );

                            fb.switch_to_block(raise_fn_ok);
                            let rfo_raise_fn = fb.block_params(raise_fn_ok)[0];
                            let exc_value = if let Some(exc_expr) = exc.as_ref() {
                                emit_direct_simple_expr(
                                    &mut fb,
                                    exc_expr,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    false,
                                    jit_module,
                                    &mut func_imports,
                                )
                            } else {
                                fb.ins().call(incref_ref, &[none_const]);
                                none_const
                            };
                            fb.ins().call(incref_ref, &[none_const]);
                            let cause_value = none_const;
                            let raise_call_inst = fb.ins().call(
                                py_call_ref,
                                &[rfo_raise_fn, exc_value, cause_value, null_ptr, null_ptr],
                            );
                            let raise_exc_obj = fb.inst_results(raise_call_inst)[0];
                            fb.ins().call(decref_ref, &[cause_value]);
                            fb.ins().call(decref_ref, &[exc_value]);
                            fb.ins().call(decref_ref, &[rfo_raise_fn]);
                            let raise_exc_null =
                                fb.ins()
                                    .icmp(ir::condcodes::IntCC::Equal, raise_exc_obj, null_ptr);
                            let raise_exc_ok = fb.create_block();
                            fb.append_block_param(raise_exc_ok, ptr_ty);
                            fb.ins().brif(
                                raise_exc_null,
                                emit_ctx.consts.step_null_block,
                                &step_null_block_args(&emit_ctx),
                                raise_exc_ok,
                                &[ir::BlockArg::Value(raise_exc_obj)],
                            );

                            fb.switch_to_block(raise_exc_ok);
                            let reo_exc_obj = fb.block_params(raise_exc_ok)[0];
                            let raise_inst = fb.ins().call(raise_exc_ref, &[reo_exc_obj]);
                            let raise_rc = fb.inst_results(raise_inst)[0];
                            fb.ins().call(decref_ref, &[reo_exc_obj]);
                            let raise_rc_fail = fb.create_block();
                            let raise_rc_ok = fb.create_block();
                            let raise_ok =
                                fb.ins().icmp_imm(ir::condcodes::IntCC::Equal, raise_rc, 0);
                            fb.ins()
                                .brif(raise_ok, raise_rc_ok, &[], raise_rc_fail, &[]);

                            fb.switch_to_block(raise_rc_fail);
                            fb.ins().jump(
                                emit_ctx.consts.step_null_block,
                                &step_null_block_args(&emit_ctx),
                            );

                            fb.switch_to_block(raise_rc_ok);
                            let current_params = &runtime_block_param_names[index];
                            let current_step_null_args = emit_prepare_target_args(
                                &mut fb,
                                current_params,
                                None,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                jit_module,
                                &mut func_imports,
                            )
                            .ok_or_else(|| {
                                format!(
                                    "missing local mapping for raise terminator cleanup args in block {}",
                                    plan.blocks[index].label
                                )
                            })?;
                            emit_decref_unforwarded_locals(
                                &mut fb,
                                &local_values,
                                &local_names,
                                current_params,
                                decref_ref,
                            );
                            fb.ins()
                                .jump(emit_ctx.consts.step_null_block, &current_step_null_args);
                        }
                    }
                    continue;
                }
                BlockFastPath::None => {
                    return Err(format!(
                        "specialized JIT encountered unexpected slow-path block {}",
                        plan.blocks[index].label
                    ));
                }
            }
        }

        for (index, maybe_dispatch_block) in exception_dispatch_blocks.iter().enumerate() {
            let Some(dispatch_block) = *maybe_dispatch_block else {
                continue;
            };
            let Some(dispatch_plan) = plan.blocks[index].exc_dispatch.as_ref() else {
                continue;
            };

            fb.switch_to_block(dispatch_block);
            let dispatch_values = fb.block_params(dispatch_block).to_vec();
            let dispatch_runtime_names = &runtime_block_param_names[index];
            let dispatch_original_names = &plan.blocks[index].param_names;
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let none_const = fb.ins().iconst(ptr_ty, none_obj as i64);
            let dispatch_step_null_args = dispatch_values
                .iter()
                .copied()
                .map(ir::BlockArg::Value)
                .collect::<Vec<_>>();

            let raised_exc_inst = fb.ins().call(py_get_raised_exc_ref, &[]);
            let raised_exc = fb.inst_results(raised_exc_inst)[0];
            let raised_exc_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, raised_exc, null_ptr);
            let raised_exc_ok = fb.create_block();
            fb.append_block_param(raised_exc_ok, ptr_ty);
            fb.ins().brif(
                raised_exc_null,
                cleanup_null_blocks[index],
                &dispatch_step_null_args,
                raised_exc_ok,
                &[ir::BlockArg::Value(raised_exc)],
            );

            fb.switch_to_block(raised_exc_ok);
            let start_exc = fb.block_params(raised_exc_ok)[0];
            let dispatch_build_start = fb.create_block();
            fb.append_block_param(dispatch_build_start, ptr_ty);
            fb.ins()
                .jump(dispatch_build_start, &[ir::BlockArg::Value(start_exc)]);

            let mut build_block = dispatch_build_start;
            let mut carried_value_count = 0usize;
            for source in dispatch_plan.arg_sources.iter() {
                fb.switch_to_block(build_block);
                let build_params = fb.block_params(build_block).to_vec();
                let b_exc = build_params[0];
                let carried_values = build_params[1..].to_vec();
                let value = match source {
                    BlockExcArgSource::SourceParam { name } => {
                        let value = resolve_named_value(
                            name,
                            dispatch_runtime_names,
                            &dispatch_values,
                            &ambient_names,
                            &ambient_values,
                        )
                        .ok_or_else(|| {
                            format!(
                                "missing exception dispatch source param {name} in block {}; runtime={:?}; original={:?}; ambient={:?}; target={:?}",
                                plan.blocks[index].label,
                                dispatch_runtime_names,
                                dispatch_original_names,
                                ambient_names,
                                runtime_block_param_names[dispatch_plan.target_index]
                            )
                        })?;
                        fb.ins().call(incref_ref, &[value]);
                        value
                    }
                    BlockExcArgSource::Exception => {
                        fb.ins().call(incref_ref, &[b_exc]);
                        b_exc
                    }
                    BlockExcArgSource::NoneValue => {
                        fb.ins().call(incref_ref, &[none_const]);
                        none_const
                    }
                    BlockExcArgSource::FrameLocal { name } => {
                        let owner_name = dispatch_plan
                            .owner_param_name
                            .as_ref()
                            .expect("missing owner param name for frame-local exception dispatch");
                        let owner = resolve_named_value(
                            owner_name,
                            dispatch_runtime_names,
                            &dispatch_values,
                            &ambient_names,
                            &ambient_values,
                        )
                        .ok_or_else(|| {
                            format!(
                                "missing exception dispatch owner {owner_name} in block {}",
                                plan.blocks[index].label
                            )
                        })?;
                        let (name_ptr, name_len) =
                            intern_bytes_literal(&mut literal_pool, name.as_bytes());
                        let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
                        let name_len_val = fb.ins().iconst(i64_ty, name_len);
                        let local_inst = fb.ins().call(
                            load_local_raw_by_name_ref,
                            &[owner, name_ptr_val, name_len_val],
                        );
                        fb.inst_results(local_inst)[0]
                    }
                };
                let value_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
                let value_fail = fb.create_block();
                fb.append_block_param(value_fail, ptr_ty);
                for _ in 0..carried_value_count {
                    fb.append_block_param(value_fail, ptr_ty);
                }
                let value_ok = fb.create_block();
                fb.append_block_param(value_ok, ptr_ty);
                fb.append_block_param(value_ok, ptr_ty);
                let mut value_fail_args = vec![ir::BlockArg::Value(b_exc)];
                value_fail_args.extend(carried_values.iter().copied().map(ir::BlockArg::Value));
                fb.ins().brif(
                    value_null,
                    value_fail,
                    &value_fail_args,
                    value_ok,
                    &[ir::BlockArg::Value(b_exc), ir::BlockArg::Value(value)],
                );

                fb.switch_to_block(value_fail);
                let vf_exc = fb.block_params(value_fail)[0];
                let vf_carried_values = fb.block_params(value_fail)[1..].to_vec();
                for carried_value in vf_carried_values {
                    fb.ins().call(decref_ref, &[carried_value]);
                }
                fb.ins().call(decref_ref, &[vf_exc]);
                fb.ins()
                    .jump(cleanup_null_blocks[index], &dispatch_step_null_args);

                fb.switch_to_block(value_ok);
                let vo_exc = fb.block_params(value_ok)[0];
                let vo_value = fb.block_params(value_ok)[1];
                let next_build_block = fb.create_block();
                fb.append_block_param(next_build_block, ptr_ty);
                for _ in 0..=carried_value_count {
                    fb.append_block_param(next_build_block, ptr_ty);
                }
                let mut next_build_args = vec![ir::BlockArg::Value(vo_exc)];
                next_build_args.extend(carried_values.iter().copied().map(ir::BlockArg::Value));
                next_build_args.push(ir::BlockArg::Value(vo_value));
                fb.ins().jump(next_build_block, &next_build_args);

                build_block = next_build_block;
                carried_value_count += 1;
            }

            fb.switch_to_block(build_block);
            let build_params = fb.block_params(build_block).to_vec();
            let bd_exc = build_params[0];
            let carried_values = build_params[1..].to_vec();
            fb.ins().call(decref_ref, &[bd_exc]);
            let mut target_jump_args =
                Vec::with_capacity(runtime_block_param_names[dispatch_plan.target_index].len());
            target_jump_args.extend(carried_values.iter().copied().map(ir::BlockArg::Value));
            for value in &dispatch_values {
                fb.ins().call(decref_ref, &[*value]);
            }
            fb.ins()
                .jump(exec_blocks[dispatch_plan.target_index], &target_jump_args);
        }

        for block in &cleanup_null_blocks {
            fb.switch_to_block(*block);
            let cleanup_args = fb.block_params(*block).to_vec();
            for value in cleanup_args {
                fb.ins().call(decref_ref, &[value]);
            }
            for value in &ambient_values {
                fb.ins().call(decref_ref, &[*value]);
            }
            function_state_slots.decref_all(&mut fb, ptr_ty, decref_ref);
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            fb.ins().return_(&[null_ptr]);
        }

        fb.switch_to_block(step_null_block);
        let step_null_args = fb.block_params(step_null_block)[0];
        function_state_slots.decref_all(&mut fb, ptr_ty, decref_ref);
        fb.ins().call(decref_ref, &[step_null_args]);
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(raise_exc_direct_block);
        let red_args = fb.block_params(raise_exc_direct_block)[0];
        let red_exc = fb.block_params(raise_exc_direct_block)[1];
        let red_null = fb.ins().iconst(ptr_ty, 0);
        let red_exc_null = fb
            .ins()
            .icmp(ir::condcodes::IntCC::Equal, red_exc, red_null);
        let red_set_block = fb.create_block();
        fb.append_block_param(red_set_block, ptr_ty);
        let red_done_block = fb.create_block();
        fb.ins().brif(
            red_exc_null,
            red_done_block,
            &[],
            red_set_block,
            &[ir::BlockArg::Value(red_exc)],
        );
        fb.switch_to_block(red_set_block);
        let red_set_exc = fb.block_params(red_set_block)[0];
        let _ = fb.ins().call(raise_exc_ref, &[red_set_exc]);
        fb.ins().call(decref_ref, &[red_set_exc]);
        fb.ins().jump(red_done_block, &[]);
        fb.switch_to_block(red_done_block);
        fb.ins().call(decref_ref, &[red_args]);
        for value in &ambient_values {
            fb.ins().call(decref_ref, &[*value]);
        }
        function_state_slots.decref_all(&mut fb, ptr_ty, decref_ref);
        fb.ins().return_(&[red_null]);

        fb.seal_all_blocks();
        fb.finalize();
    }

    import_id_to_symbol.extend(
        module_imports
            .debug_symbols()
            .iter()
            .map(|(import_id, symbol)| (*import_id, *symbol)),
    );

    Ok((ctx, main_id, literal_pool, import_id_to_symbol))
}

pub unsafe fn render_cranelift_run_bb_specialized_with_cfg(
    blocks: &[ObjPtr],
    plan: &ClifPlan,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
    deleted_obj: ObjPtr,
    empty_tuple_obj: ObjPtr,
) -> Result<RenderedSpecializedClif, String> {
    if blocks.is_empty() {
        return Err("specialized JIT run_bb requires at least one block".to_string());
    }

    let mut builder = new_jit_builder()?;
    register_specialized_jit_symbols(&mut builder);
    let mut jit_module = JITModule::new(builder);
    let (ctx, _, _literal_pool, import_id_to_symbol) = build_cranelift_run_bb_specialized_function(
        &mut jit_module,
        blocks,
        plan,
        ptr::null_mut(),
        true_obj,
        false_obj,
        ptr::null_mut(),
        deleted_obj,
        empty_tuple_obj,
    )?;
    let mut out = String::new();
    out.push_str("; import fn aliases (Cranelift display id -> symbol)\n");
    let mut symbols: Vec<&'static str> = import_id_to_symbol.values().copied().collect();
    symbols.sort_unstable();
    symbols.dedup();
    for symbol in symbols {
        out.push_str("; ");
        out.push_str(symbol);
        out.push('\n');
    }
    out.push('\n');
    let (compiled_clif, cfg_dot, vcode_disasm) =
        render_compiled_clif_and_vcode_disasm(&mut jit_module, ctx, &import_id_to_symbol)?;
    out.push_str(&compiled_clif);
    Ok(RenderedSpecializedClif {
        clif: out,
        cfg_dot,
        vcode_disasm,
    })
}

fn render_compiled_clif_and_vcode_disasm(
    jit_module: &mut JITModule,
    mut ctx: cranelift_codegen::Context,
    import_id_to_symbol: &HashMap<u32, &'static str>,
) -> Result<(String, String, String), String> {
    let mut ctrl_plane = ControlPlane::default();
    ctx.optimize(jit_module.isa(), &mut ctrl_plane)
        .map_err(|err| format!("failed to optimize specialized jit run_bb function: {err:?}"))?;

    let cfg_dot = CFGPrinter::new(&ctx.func).to_string();

    let mut clif = String::new();
    clif.push_str("; ---- post-opt CLIF fed to Cranelift backend ----\n");
    clif.push_str(&rewrite_import_fn_aliases(
        ctx.func.display().to_string().as_str(),
        import_id_to_symbol,
    ));

    let compiled = jit_module
        .isa()
        .compile_function(&ctx.func, &ctx.domtree, true, &mut ctrl_plane)
        .map_err(|err| format!("failed to compile specialized jit run_bb function: {err:?}"))?;

    let mut vcode_disasm = String::new();
    vcode_disasm.push_str("; ---- emitted VCode disassembly ----\n");
    match compiled.vcode {
        Some(disasm) if !disasm.trim().is_empty() => vcode_disasm.push_str(&disasm),
        _ => vcode_disasm.push_str("; emitted disassembly unavailable for this backend\n"),
    }

    Ok((clif, cfg_dot, vcode_disasm))
}

pub unsafe fn compile_cranelift_run_bb_specialized_cached(
    blocks: &[ObjPtr],
    plan: &ClifPlan,
    globals_obj: ObjPtr,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
    none_obj: ObjPtr,
    deleted_obj: ObjPtr,
    empty_tuple_obj: ObjPtr,
) -> Result<ObjPtr, String> {
    if globals_obj.is_null() {
        return Err("invalid null globals object passed to specialized JIT run_bb".to_string());
    }
    let mut builder = new_jit_builder()?;
    register_specialized_jit_symbols(&mut builder);
    let mut compiled = Box::new(CompiledSpecializedRunner {
        _jit_module: JITModule::new(builder),
        _literal_pool: Vec::new(),
        entry: None,
    });
    let (mut ctx, main_id, literal_pool, _import_id_to_symbol) =
        build_cranelift_run_bb_specialized_function(
            &mut compiled._jit_module,
            blocks,
            plan,
            globals_obj,
            true_obj,
            false_obj,
            none_obj,
            deleted_obj,
            empty_tuple_obj,
        )?;
    define_function_with_incremental_cache(
        &mut compiled._jit_module,
        main_id,
        &mut ctx,
        "failed to define specialized jit run_bb function",
    )?;
    compiled._jit_module.clear_context(&mut ctx);
    compiled
        ._jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize specialized jit run_bb function: {err}"))?;
    let code_ptr = compiled._jit_module.get_finalized_function(main_id);
    compiled.entry = Some(std::mem::transmute(code_ptr));
    compiled._literal_pool = literal_pool;
    Ok(Box::into_raw(compiled) as ObjPtr)
}

pub unsafe fn compile_cranelift_vectorcall_trampoline(
    build_bb_args_fn: unsafe extern "C" fn(ObjPtr, *const ObjPtr, usize, ObjPtr, ObjPtr) -> ObjPtr,
    run_compiled_fn: unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr,
    data_ptr: ObjPtr,
    compiled_handle: ObjPtr,
) -> Result<(ObjPtr, VectorcallEntryFn), String> {
    if data_ptr.is_null() {
        return Err("invalid null vectorcall data pointer".to_string());
    }
    if compiled_handle.is_null() {
        return Err("invalid null compiled handle for vectorcall trampoline".to_string());
    }

    let mut builder = new_jit_builder()?;
    builder.symbol(
        "dp_jit_vectorcall_build_bb_args",
        build_bb_args_fn as *const u8,
    );
    builder.symbol(
        "dp_jit_vectorcall_run_compiled",
        run_compiled_fn as *const u8,
    );
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    let mut jit_module = JITModule::new(builder);
    let ptr_ty = jit_module.target_config().pointer_type();

    let mut build_sig = jit_module.make_signature();
    build_sig.params.push(ir::AbiParam::new(ptr_ty));
    build_sig.params.push(ir::AbiParam::new(ptr_ty));
    build_sig.params.push(ir::AbiParam::new(ptr_ty));
    build_sig.params.push(ir::AbiParam::new(ptr_ty));
    build_sig.params.push(ir::AbiParam::new(ptr_ty));
    build_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut run_sig = jit_module.make_signature();
    run_sig.params.push(ir::AbiParam::new(ptr_ty));
    run_sig.params.push(ir::AbiParam::new(ptr_ty));
    run_sig.params.push(ir::AbiParam::new(ptr_ty));
    run_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut decref_sig = jit_module.make_signature();
    decref_sig.params.push(ir::AbiParam::new(ptr_ty));

    let mut main_sig = jit_module.make_signature();
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let build_id = declare_import_fn(
        &mut jit_module,
        "dp_jit_vectorcall_build_bb_args",
        &build_sig,
    )?;
    let run_id = declare_import_fn(&mut jit_module, "dp_jit_vectorcall_run_compiled", &run_sig)?;
    let decref_id = declare_import_fn(&mut jit_module, "dp_jit_decref", &decref_sig)?;
    let main_id = declare_local_fn(&mut jit_module, "dp_jit_vectorcall_trampoline", &main_sig)?;

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
        let args_val = fb.block_params(entry)[1];
        let nargsf_val = fb.block_params(entry)[2];
        let kwnames_val = fb.block_params(entry)[3];

        let build_ref = jit_module.declare_func_in_func(build_id, &mut fb.func);
        let run_ref = jit_module.declare_func_in_func(run_id, &mut fb.func);
        let decref_ref = jit_module.declare_func_in_func(decref_id, &mut fb.func);

        let data_const = fb.ins().iconst(ptr_ty, data_ptr as i64);
        let compiled_const = fb.ins().iconst(ptr_ty, compiled_handle as i64);
        let build_inst = fb.ins().call(
            build_ref,
            &[callable_val, args_val, nargsf_val, kwnames_val, data_const],
        );
        let bb_args = fb.inst_results(build_inst)[0];
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        let args_missing = fb
            .ins()
            .icmp(ir::condcodes::IntCC::Equal, bb_args, null_ptr);
        let build_failed = fb.create_block();
        let build_ok = fb.create_block();
        fb.append_block_param(build_ok, ptr_ty);
        fb.ins().brif(
            args_missing,
            build_failed,
            &[],
            build_ok,
            &[ir::BlockArg::Value(bb_args)],
        );
        fb.seal_block(build_failed);
        fb.seal_block(build_ok);

        fb.switch_to_block(build_failed);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(build_ok);
        let built_args = fb.block_params(build_ok)[0];
        let run_inst = fb
            .ins()
            .call(run_ref, &[compiled_const, built_args, data_const]);
        let result = fb.inst_results(run_inst)[0];
        fb.ins().call(decref_ref, &[built_args]);
        fb.ins().return_(&[result]);
        fb.seal_all_blocks();
        fb.finalize();
    }

    define_function_with_incremental_cache(
        &mut jit_module,
        main_id,
        &mut ctx,
        "failed to define vectorcall trampoline",
    )?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize vectorcall trampoline: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(main_id);
    let entry: VectorcallEntryFn = std::mem::transmute(code_ptr);
    let compiled = Box::new(CompiledVectorcallRunner {
        _jit_module: jit_module,
    });
    Ok((Box::into_raw(compiled) as ObjPtr, entry))
}

pub unsafe fn run_cranelift_run_bb_specialized_cached(
    compiled_handle: ObjPtr,
    args: ObjPtr,
    ambient_args: ObjPtr,
    hooks: &SpecializedJitHooks,
) -> Result<ObjPtr, String> {
    if compiled_handle.is_null() {
        return Err("invalid null compiled handle passed to specialized JIT run_bb".to_string());
    }
    if args.is_null() {
        return Err("invalid null args passed to specialized JIT run_bb".to_string());
    }
    if ambient_args.is_null() {
        return Err("invalid null ambient args passed to specialized JIT run_bb".to_string());
    }
    install_specialized_hooks(hooks);
    let compiled = &*(compiled_handle as *const CompiledSpecializedRunner);
    let Some(entry) = compiled.entry else {
        return Err("invalid compiled handle without entrypoint".to_string());
    };
    Ok(entry(args, ambient_args))
}

pub unsafe fn free_cranelift_vectorcall_trampoline(compiled_handle: ObjPtr) {
    if compiled_handle.is_null() {
        return;
    }
    let _ = Box::from_raw(compiled_handle as *mut CompiledVectorcallRunner);
}

pub unsafe fn free_cranelift_run_bb_specialized_cached(compiled_handle: ObjPtr) {
    if compiled_handle.is_null() {
        return;
    }
    let _ = Box::from_raw(compiled_handle as *mut CompiledSpecializedRunner);
}

pub unsafe fn run_cranelift_run_bb_specialized(
    blocks: &[ObjPtr],
    plan: &ClifPlan,
    globals_obj: ObjPtr,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
    args: ObjPtr,
    ambient_args: ObjPtr,
    hooks: &SpecializedJitHooks,
    none_obj: ObjPtr,
    deleted_obj: ObjPtr,
    empty_tuple_obj: ObjPtr,
) -> Result<ObjPtr, String> {
    if args.is_null() {
        return Err("invalid null args passed to specialized JIT run_bb".to_string());
    }
    if globals_obj.is_null() {
        return Err("invalid null globals object passed to specialized JIT run_bb".to_string());
    }
    if ambient_args.is_null() {
        return Err("invalid null ambient args passed to specialized JIT run_bb".to_string());
    }
    install_specialized_hooks(hooks);
    let mut builder = new_jit_builder()?;
    register_specialized_jit_symbols(&mut builder);
    let mut jit_module = JITModule::new(builder);
    let (mut ctx, main_id, _literal_pool, _import_id_to_symbol) =
        build_cranelift_run_bb_specialized_function(
            &mut jit_module,
            blocks,
            plan,
            globals_obj,
            true_obj,
            false_obj,
            none_obj,
            deleted_obj,
            empty_tuple_obj,
        )?;

    define_function_with_incremental_cache(
        &mut jit_module,
        main_id,
        &mut ctx,
        "failed to define specialized jit run_bb function",
    )?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize specialized jit run_bb function: {err}"))?;
    let code_ptr = jit_module.get_finalized_function(main_id);
    let compiled: extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr = std::mem::transmute(code_ptr);
    Ok(compiled(args, ambient_args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dp_transform::block_py::{BlockPyRaise, BlockPyTerm, CoreBlockPyExpr};

    fn test_term() -> BlockPyTerm<CoreBlockPyExpr> {
        BlockPyTerm::Raise(BlockPyRaise { exc: None })
    }

    #[test]
    fn render_specialized_jit_clif_smoke() {
        let blocks = [1usize as ObjPtr, 2usize as ObjPtr, 3usize as ObjPtr];
        let plan = ClifPlan {
            ambient_param_names: vec![],
            slot_names: vec![],
            blocks: vec![
                ClifBlockPlan {
                    label: "b0".into(),
                    param_names: vec![],
                    term: test_term(),
                    exc_target: None,
                    exc_dispatch: None,
                    fast_path: BlockFastPath::None,
                },
                ClifBlockPlan {
                    label: "b1".into(),
                    param_names: vec![],
                    term: test_term(),
                    exc_target: None,
                    exc_dispatch: None,
                    fast_path: BlockFastPath::None,
                },
                ClifBlockPlan {
                    label: "b2".into(),
                    param_names: vec![],
                    term: test_term(),
                    exc_target: None,
                    exc_dispatch: None,
                    fast_path: BlockFastPath::None,
                },
            ],
        };
        let err = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect_err("specialized JIT CLIF render should reject slow-path blocks");
        assert!(
            err.contains("fully lowered fastpath blocks"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn render_specialized_jit_operator_calls_use_python_capi() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            ambient_param_names: vec![],
            slot_names: vec![],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Intrinsic {
                            intrinsic: &intrinsics::ADD_INTRINSIC,
                            parts: vec![
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(1)),
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(2)),
                            ],
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call PyNumber_Add"),
            "operator lowering should use PyNumber_Add in rendered CLIF:\n{rendered}"
        );
        assert!(
            !rendered.contains("call PyObject_CallFunctionObjArgs"),
            "direct operator lowering should avoid generic Python helper calls:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_compare_calls_use_richcompare() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            ambient_param_names: vec![],
            slot_names: vec![],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Call {
                            func: Box::new(DirectSimpleExprPlan::Name("__dp_lt".into())),
                            parts: vec![
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(1)),
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(2)),
                            ],
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call PyObject_RichCompare"),
            "comparison lowering should use PyObject_RichCompare in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_allocates_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            ambient_param_names: vec![],
            slot_names: vec!["x".into(), "y".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Int(7),
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.matches("explicit_slot 8").count() >= 2,
            "slot-backed JIT plans should allocate explicit stack slots:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_assignments_sync_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            ambient_param_names: vec![],
            slot_names: vec!["x".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleBlock {
                    plan: DirectSimpleBlockPlan {
                        params: vec![],
                        ops: vec![DirectSimpleOpPlan::Assign(DirectSimpleAssignPlan {
                            target: "x".into(),
                            value: DirectSimpleExprPlan::Int(7),
                        })],
                        term: DirectSimpleTermPlan::Ret {
                            value: DirectSimpleExprPlan::Name("x".into()),
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("store.i64") || rendered.contains("stack_store"),
            "assignment-backed JIT plans should update mirrored function-state slots:\n{rendered}"
        );
    }
}

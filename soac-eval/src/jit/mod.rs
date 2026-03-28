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
use dp_transform::block_py::{
    BlockPyModule, LocatedName, NameLocation, intrinsics as blockpy_intrinsics,
};
use dp_transform::passes::PreparedBbBlockPyPass;
use ruff_python_ast as ast;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

mod intrinsics;
mod planning;
mod specialized_helpers;

pub use planning::{
    BlockExcArgSource, BlockExcDispatchPlan, BlockFastPath, ClifBindingParam, ClifBindingParamKind,
    ClifBlockPlan, ClifEntryParamDefaultSource, ClifPlan, DirectSimpleAssignPlan,
    DirectSimpleBlockArgPlan, DirectSimpleBlockPlan, DirectSimpleBrIfPlan, DirectSimpleCallPart,
    DirectSimpleDeletePlan, DirectSimpleDeleteTargetPlan, DirectSimpleExprPlan, DirectSimpleOpPlan,
    DirectSimpleRetPlan, DirectSimpleTermPlan, lookup_blockpy_function, lookup_clif_plan,
    register_clif_module_plans,
};
pub use specialized_helpers::ObjPtr;
use specialized_helpers::{dp_jit_decref, register_specialized_jit_symbols};

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
    F64,
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
    const fn new(
        symbol: &'static str,
        params: &'static [SigType],
        returns: &'static [SigType],
    ) -> Self {
        Self {
            symbol,
            signature: StaticSignature::new(params, returns),
            internal_id: OnceLock::new(),
        }
    }

    fn internal_id(&'static self) -> usize {
        *self
            .internal_id
            .get_or_init(|| NEXT_IMPORT_SPEC_ID.fetch_add(1, Ordering::Relaxed))
    }
}

static DP_JIT_INCREF_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_incref", &[SigType::Pointer], &[]);
static DP_JIT_DECREF_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_decref", &[SigType::Pointer], &[]);
static DP_JIT_PY_CALL_POSITIONAL_THREE_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_py_call_positional_three",
    &[
        SigType::Pointer,
        SigType::Pointer,
        SigType::Pointer,
        SigType::Pointer,
        SigType::Pointer,
    ],
    &[SigType::Pointer],
);
static DP_JIT_PY_CALL_OBJECT_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_py_call_object",
    &[SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);
static DP_JIT_PY_CALL_WITH_KW_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_py_call_with_kw",
    &[SigType::Pointer, SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);
static DP_JIT_GET_RAISED_EXCEPTION_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_get_raised_exception", &[], &[SigType::Pointer]);
static DP_JIT_MAKE_INT_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_make_int", &[SigType::I64], &[SigType::Pointer]);
static DP_JIT_MAKE_FLOAT_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_make_float", &[SigType::F64], &[SigType::Pointer]);
static DP_JIT_MAKE_BYTES_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_make_bytes",
    &[SigType::Pointer, SigType::I64],
    &[SigType::Pointer],
);
static DP_JIT_LOAD_NAME_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_load_name",
    &[SigType::Pointer, SigType::Pointer, SigType::I64],
    &[SigType::Pointer],
);
static DP_JIT_FUNCTION_GLOBALS_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_function_globals",
    &[SigType::Pointer],
    &[SigType::Pointer],
);
static DP_JIT_FUNCTION_CLOSURE_CELL_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_function_closure_cell",
    &[SigType::Pointer, SigType::I64],
    &[SigType::Pointer],
);
static DP_JIT_FUNCTION_POSITIONAL_DEFAULT_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_function_positional_default",
    &[
        SigType::Pointer,
        SigType::Pointer,
        SigType::I64,
        SigType::I64,
    ],
    &[SigType::Pointer],
);
static DP_JIT_FUNCTION_KWONLY_DEFAULT_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_function_kwonly_default",
    &[SigType::Pointer, SigType::Pointer, SigType::I64],
    &[SigType::Pointer],
);
static DP_JIT_PYOBJECT_GETATTR_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_pyobject_getattr",
    &[SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);
static DP_JIT_PYOBJECT_SETATTR_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_pyobject_setattr",
    &[SigType::Pointer, SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);
static DP_JIT_PYOBJECT_GETITEM_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_pyobject_getitem",
    &[SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);
static DP_JIT_PYOBJECT_SETITEM_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_pyobject_setitem",
    &[SigType::Pointer, SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);
static DP_JIT_PYOBJECT_TO_I64_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_pyobject_to_i64",
    &[SigType::Pointer],
    &[SigType::I64],
);
static DP_JIT_DECODE_LITERAL_BYTES_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_decode_literal_bytes",
    &[SigType::Pointer, SigType::I64],
    &[SigType::Pointer],
);
static DP_JIT_LOAD_DELETED_NAME_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_load_deleted_name",
    &[
        SigType::Pointer,
        SigType::I64,
        SigType::Pointer,
        SigType::Pointer,
    ],
    &[SigType::Pointer],
);
static DP_JIT_MAKE_CELL_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_make_cell", &[SigType::Pointer], &[SigType::Pointer]);
static DP_JIT_LOAD_CELL_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_load_cell", &[SigType::Pointer], &[SigType::Pointer]);
static DP_JIT_STORE_CELL_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_store_cell",
    &[SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);
static DP_JIT_TUPLE_NEW_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_tuple_new", &[SigType::I64], &[SigType::Pointer]);
static DP_JIT_TUPLE_SET_ITEM_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_tuple_set_item",
    &[SigType::Pointer, SigType::I64, SigType::Pointer],
    &[SigType::I32],
);
static DP_JIT_IS_TRUE_IMPORT: ImportSpec =
    ImportSpec::new("dp_jit_is_true", &[SigType::Pointer], &[SigType::I32]);
static DP_JIT_RAISE_FROM_EXC_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_raise_from_exc",
    &[SigType::Pointer],
    &[SigType::I32],
);
static DP_JIT_VECTORCALL_BIND_DIRECT_ARGS_IMPORT: ImportSpec = ImportSpec::new(
    "dp_jit_vectorcall_bind_direct_args",
    &[
        SigType::Pointer,
        SigType::Pointer,
        SigType::Pointer,
        SigType::Pointer,
        SigType::Pointer,
        SigType::Pointer,
        SigType::I64,
    ],
    &[SigType::I32],
);
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

    fn get_or_panic(
        &mut self,
        jit_module: &mut JITModule,
        func: &mut ir::Function,
        spec: &'static ImportSpec,
    ) -> ir::FuncRef {
        self.get(jit_module, func, spec).unwrap_or_else(|err| {
            panic!(
                "failed to bind import {} during JIT codegen: {}",
                spec.symbol, err
            )
        })
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
    entry: Option<CompiledRunnerEntry>,
}

pub type VectorcallEntryFn = unsafe extern "C" fn(ObjPtr, *const ObjPtr, usize, ObjPtr) -> ObjPtr;

struct CompiledVectorcallRunner {
    _jit_module: JITModule,
}

#[derive(Clone, Copy)]
enum CompiledRunnerEntry {
    Direct {
        code_ptr: *const u8,
        param_count: usize,
    },
}

fn direct_simple_expr_is_borrowable(
    expr: &DirectSimpleExprPlan,
    local_names: &[String],
    function_state_slots: &FunctionStateSlots,
) -> bool {
    match expr {
        DirectSimpleExprPlan::Name(name) => {
            if matches!(
                name.location,
                NameLocation::OwnedCell { .. }
                    | NameLocation::CapturedCellSource { .. }
                    | NameLocation::ClosureCell { .. }
            ) {
                return false;
            }
            local_names
                .iter()
                .any(|candidate| candidate == name.id.as_str())
                || function_state_slots.has_name(name.id.as_str())
        }
        DirectSimpleExprPlan::Int(_)
        | DirectSimpleExprPlan::Float(_)
        | DirectSimpleExprPlan::Bytes(_)
        | DirectSimpleExprPlan::Op(_)
        | DirectSimpleExprPlan::Intrinsic { .. }
        | DirectSimpleExprPlan::Call { .. } => false,
    }
}

enum DirectSimpleCallCallee<'a> {
    Name(&'a str),
    Op(&'a blockpy_intrinsics::Operation<DirectSimpleExprPlan>),
    Intrinsic(&'static dyn blockpy_intrinsics::Intrinsic),
}

impl DirectSimpleCallCallee<'_> {
    fn name(&self) -> &str {
        match self {
            DirectSimpleCallCallee::Name(name) => name,
            DirectSimpleCallCallee::Op(operation) => operation.helper_name(),
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
                DirectSimpleCallCallee::Name(func_name.id.as_str()),
                parts.as_slice(),
            )
        }
        DirectSimpleExprPlan::Intrinsic { intrinsic, parts } => (
            DirectSimpleCallCallee::Intrinsic(*intrinsic),
            parts.as_slice(),
        ),
        DirectSimpleExprPlan::Op(operation) => {
            return Some((
                DirectSimpleCallCallee::Op(operation.as_ref()),
                operation.call_args(),
            ));
        }
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
        DirectSimpleExprPlan::Op(_)
        | DirectSimpleExprPlan::Intrinsic { .. }
        | DirectSimpleExprPlan::Call { .. } => {
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

fn intern_bytes_literal(literal_pool: &mut Vec<Box<[u8]>>, bytes: &[u8]) -> (*const u8, i64) {
    let boxed = bytes.to_vec().into_boxed_slice();
    let ptr = boxed.as_ptr();
    let len = boxed.len() as i64;
    literal_pool.push(boxed);
    (ptr, len)
}

fn compat_global_name(id: &str) -> LocatedName {
    LocatedName {
        id: id.into(),
        ctx: ast::ExprContext::Load,
        range: Default::default(),
        node_index: Default::default(),
        location: NameLocation::Global,
    }
}

struct DirectSimpleEmitConsts {
    step_null_block: ir::Block,
    step_null_args: Vec<ir::Value>,
    ptr_ty: ir::Type,
    i64_ty: ir::Type,
    callable_value: ir::Value,
    none_const: ir::Value,
    true_const: ir::Value,
    false_const: ir::Value,
    deleted_const: ir::Value,
    empty_tuple_const: ir::Value,
    block_const: ir::Value,
}

struct DirectSimpleEmitCtx {
    owned_cell_slot_names: Vec<String>,
    incref_ref: ir::FuncRef,
    decref_ref: ir::FuncRef,
    py_call_positional_three_ref: ir::FuncRef,
    make_int_ref: ir::FuncRef,
    consts: DirectSimpleEmitConsts,
    load_name_ref: ir::FuncRef,
    function_globals_ref: ir::FuncRef,
    function_closure_cell_ref: ir::FuncRef,
    pyobject_getattr_ref: ir::FuncRef,
    pyobject_setattr_ref: ir::FuncRef,
    pyobject_getitem_ref: ir::FuncRef,
    pyobject_setitem_ref: ir::FuncRef,
    decode_literal_bytes_ref: ir::FuncRef,
    load_deleted_name_ref: ir::FuncRef,
    make_cell_ref: ir::FuncRef,
    load_cell_ref: ir::FuncRef,
    store_cell_ref: ir::FuncRef,
    make_bytes_ref: ir::FuncRef,
    make_float_ref: ir::FuncRef,
    py_call_object_ref: ir::FuncRef,
    py_call_with_kw_ref: ir::FuncRef,
    tuple_new_ref: ir::FuncRef,
    tuple_set_item_ref: ir::FuncRef,
    function_state_slots: FunctionStateSlots,
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

#[derive(Clone)]
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

    fn has_name(&self, name: &str) -> bool {
        self.slot_for_name(name).is_some()
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
        let previous = local_values.remove(existing_index);
        local_names.remove(existing_index);
        fb.ins().call(decref_ref, &[previous]);
    }
    if function_state_slots.has_name(name) {
        function_state_slots
            .replace_cloned_value(fb, name, value, ptr_ty, incref_ref, decref_ref)
            .expect("slot-backed local missing from function state slots");
        fb.ins().call(decref_ref, &[value]);
    } else {
        local_names.push(name.to_string());
        local_values.push(value);
    }
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
    if let Some(index) = local_names.iter().position(|candidate| candidate == name) {
        let previous = local_values.remove(index);
        local_names.remove(index);
        fb.ins().call(decref_ref, &[previous]);
    } else if !function_state_slots.has_name(name) {
        return Err(format!("missing local binding for delete target: {name}"));
    }
    if function_state_slots.has_name(name) {
        function_state_slots
            .replace_cloned_value(fb, name, deleted_const, ptr_ty, incref_ref, decref_ref)
            .expect("slot-backed delete target missing from function state slots");
    }
    Ok(())
}

impl DirectSimpleIntrinsicEmitState<'_, '_, '_, '_> {
    fn positional_args_for_intrinsic<'a>(
        &self,
        intrinsic: &dyn blockpy_intrinsics::Intrinsic,
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
        assert!(
            intrinsic.accepts_arity(args.len()),
            "intrinsic {} received unsupported arity {} in JIT plan",
            intrinsic.name(),
            args.len(),
        );
        args
    }

    fn emit_arg_values(&mut self, args: &[&DirectSimpleExprPlan]) -> Vec<(ir::Value, bool)> {
        let mut arg_values = Vec::with_capacity(args.len());
        for arg in args {
            let borrowed_arg = direct_simple_expr_is_borrowable(
                arg,
                self.local_names,
                &self.ctx.function_state_slots,
            );
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
            .get_or_panic(self.jit_module, &mut self.fb.func, spec)
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
}

fn jit_intrinsic_by_intrinsic(
    intrinsic: &'static dyn blockpy_intrinsics::Intrinsic,
) -> Option<&'static dyn intrinsics::JitIntrinsic> {
    intrinsics::jit_intrinsic_by_intrinsic(intrinsic)
}

fn load_function_state_value(
    fb: &mut FunctionBuilder<'_>,
    function_state_slots: &FunctionStateSlots,
    name: &str,
    ptr_ty: ir::Type,
    borrowed: bool,
    incref_ref: ir::FuncRef,
) -> Option<ir::Value> {
    let slot = function_state_slots.slot_for_name(name)?;
    let value = fb.ins().stack_load(ptr_ty, slot, 0);
    if !borrowed {
        fb.ins().call(incref_ref, &[value]);
    }
    Some(value)
}

fn block_arg_values(values: &[ir::Value]) -> Vec<ir::BlockArg> {
    values.iter().copied().map(ir::BlockArg::Value).collect()
}

fn step_null_block_args(ctx: &DirectSimpleEmitCtx) -> Vec<ir::BlockArg> {
    block_arg_values(&ctx.consts.step_null_args)
}

fn emit_raw_cell_object_for_name(
    fb: &mut FunctionBuilder<'_>,
    name: &LocatedName,
    local_names: &[String],
    local_values: &[ir::Value],
    ctx: &DirectSimpleEmitCtx,
) -> ir::Value {
    let ptr_ty = ctx.consts.ptr_ty;
    let i64_ty = ctx.consts.i64_ty;
    let null_ptr = fb.ins().iconst(ptr_ty, 0);

    match name.location {
        NameLocation::OwnedCell { slot } => {
            let storage_name = ctx
                .owned_cell_slot_names
                .get(slot as usize)
                .unwrap_or_else(|| {
                    panic!(
                        "missing owned cell slot mapping for {} at local cell slot {}",
                        name.id, slot
                    )
                });
            let mut candidate_names = vec![storage_name.as_str()];
            if name.id.as_str() != storage_name.as_str() {
                candidate_names.push(name.id.as_str());
            }
            for candidate_name in &candidate_names {
                if let Some(slot_index) = local_names
                    .iter()
                    .position(|candidate| candidate == *candidate_name)
                {
                    let slot_value = local_values[slot_index];
                    fb.ins().call(ctx.incref_ref, &[slot_value]);
                    return slot_value;
                }
                if let Some(slot_value) = load_function_state_value(
                    fb,
                    &ctx.function_state_slots,
                    candidate_name,
                    ptr_ty,
                    false,
                    ctx.incref_ref,
                ) {
                    return slot_value;
                }
            }
            panic!(
                "missing owned cell {} in direct JIT state via names {:?} (slot {slot})",
                name.id, candidate_names
            );
        }
        NameLocation::ClosureCell { slot } | NameLocation::CapturedCellSource { slot } => {
            let slot_value = fb.ins().iconst(i64_ty, slot as i64);
            let raw_cell_inst = fb.ins().call(
                ctx.function_closure_cell_ref,
                &[ctx.consts.callable_value, slot_value],
            );
            let raw_cell_value = fb.inst_results(raw_cell_inst)[0];
            let raw_cell_is_null =
                fb.ins()
                    .icmp(ir::condcodes::IntCC::Equal, raw_cell_value, null_ptr);
            let raw_cell_ok_block = fb.create_block();
            fb.append_block_param(raw_cell_ok_block, ptr_ty);
            fb.ins().brif(
                raw_cell_is_null,
                ctx.consts.step_null_block,
                &step_null_block_args(ctx),
                raw_cell_ok_block,
                &[ir::BlockArg::Value(raw_cell_value)],
            );
            fb.switch_to_block(raw_cell_ok_block);
            fb.block_params(raw_cell_ok_block)[0]
        }
        NameLocation::Local { .. } | NameLocation::Global => {
            panic!(
                "raw cell access should target a cell-backed name, got {} at {:?}",
                name.id, name.location
            );
        }
    }
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
    let py_call_ref = ctx.py_call_positional_three_ref;
    let make_int_ref = ctx.make_int_ref;
    let step_null_block = ctx.consts.step_null_block;
    let ptr_ty = ctx.consts.ptr_ty;
    let i64_ty = ctx.consts.i64_ty;
    let callable_value = ctx.consts.callable_value;
    let deleted_const = ctx.consts.deleted_const;
    let empty_tuple_const = ctx.consts.empty_tuple_const;
    let block_const = ctx.consts.block_const;
    let load_name_ref = ctx.load_name_ref;
    let function_globals_ref = ctx.function_globals_ref;
    let function_closure_cell_ref = ctx.function_closure_cell_ref;
    let pyobject_getattr_ref = ctx.pyobject_getattr_ref;
    let pyobject_setitem_ref = ctx.pyobject_setitem_ref;
    let decode_literal_bytes_ref = ctx.decode_literal_bytes_ref;
    let load_deleted_name_ref = ctx.load_deleted_name_ref;
    let make_cell_ref = ctx.make_cell_ref;
    let load_cell_ref = ctx.load_cell_ref;
    let store_cell_ref = ctx.store_cell_ref;
    let make_bytes_ref = ctx.make_bytes_ref;
    let make_float_ref = ctx.make_float_ref;
    let py_call_object_ref = ctx.py_call_object_ref;
    let py_call_with_kw_ref = ctx.py_call_with_kw_ref;
    let tuple_new_ref = ctx.tuple_new_ref;
    let tuple_set_item_ref = ctx.tuple_set_item_ref;

    match expr {
        DirectSimpleExprPlan::Name(name) => {
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            match name.location {
                NameLocation::Local { slot: _ } => {
                    if let Some(slot_index) = local_names
                        .iter()
                        .position(|candidate| candidate == name.id.as_str())
                    {
                        let slot_value = local_values[slot_index];
                        if !borrowed {
                            fb.ins().call(incref_ref, &[slot_value]);
                        }
                        return slot_value;
                    }
                    if let Some(slot_value) = load_function_state_value(
                        fb,
                        &ctx.function_state_slots,
                        name.id.as_str(),
                        ptr_ty,
                        borrowed,
                        incref_ref,
                    ) {
                        return slot_value;
                    }
                    panic!("missing located local {} in direct JIT state", name.id);
                }
                NameLocation::OwnedCell { .. } | NameLocation::ClosureCell { .. } => {
                    assert!(
                        !borrowed,
                        "cell-backed name loads must produce owned references"
                    );
                    let cell_obj =
                        emit_raw_cell_object_for_name(fb, name, local_names, local_values, ctx);
                    let value_inst = fb.ins().call(load_cell_ref, &[cell_obj]);
                    let value = fb.inst_results(value_inst)[0];
                    fb.ins().call(decref_ref, &[cell_obj]);
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
                    return fb.block_params(value_ok_block)[0];
                }
                NameLocation::CapturedCellSource { .. } => {
                    assert!(
                        !borrowed,
                        "captured cell source loads must produce owned references"
                    );
                    return emit_raw_cell_object_for_name(fb, name, local_names, local_values, ctx);
                }
                NameLocation::Global => {
                    let globals_inst = fb.ins().call(function_globals_ref, &[callable_value]);
                    let globals_value = fb.inst_results(globals_inst)[0];
                    let globals_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, globals_value, null_ptr);
                    let globals_ok_block = fb.create_block();
                    fb.append_block_param(globals_ok_block, ptr_ty);
                    fb.ins().brif(
                        globals_is_null,
                        step_null_block,
                        &step_null_block_args(ctx),
                        globals_ok_block,
                        &[ir::BlockArg::Value(globals_value)],
                    );
                    fb.switch_to_block(globals_ok_block);
                    let globals_obj = fb.block_params(globals_ok_block)[0];
                    let (name_ptr, name_len) =
                        intern_bytes_literal(literal_pool, name.id.as_str().as_bytes());
                    let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
                    let name_len_val = fb.ins().iconst(i64_ty, name_len);
                    let value_inst = fb
                        .ins()
                        .call(load_name_ref, &[globals_obj, name_ptr_val, name_len_val]);
                    let value = fb.inst_results(value_inst)[0];
                    fb.ins().call(decref_ref, &[globals_obj]);
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
                    return fb.block_params(value_ok_block)[0];
                }
            }
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
        DirectSimpleExprPlan::Op(operation) => {
            assert!(
                !borrowed,
                "direct simple plan must not use borrowed operation expression"
            );
            let mut parts = Vec::new();
            for arg in operation.clone().into_call_args() {
                parts.push(DirectSimpleCallPart::Pos(arg));
            }
            let mut intrinsic_state = DirectSimpleIntrinsicEmitState {
                fb,
                local_names,
                local_values,
                ctx,
                literal_pool,
                jit_module,
                func_imports,
            };
            if let Some(jit_intrinsic) = intrinsics::jit_intrinsic_by_operation(operation.as_ref())
            {
                return jit_intrinsic.emit_direct_simple(&mut intrinsic_state, &parts);
            }
            let fallback = DirectSimpleExprPlan::Call {
                func: Box::new(DirectSimpleExprPlan::Name(compat_global_name(
                    operation.helper_name(),
                ))),
                parts,
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
                return jit_intrinsic.emit_direct_simple(&mut intrinsic_state, parts);
            }
            let fallback = DirectSimpleExprPlan::Call {
                func: Box::new(DirectSimpleExprPlan::Name(compat_global_name(
                    intrinsic.name(),
                ))),
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
                    && func_name.id.as_str() == "__dp_decode_literal_bytes"
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
                    && func_name.id.as_str() == "str"
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
                    && (func_name.id.as_str() == "globals"
                        || func_name.id.as_str() == "__dp_globals")
                {
                    fb.ins().call(incref_ref, &[block_const]);
                    return block_const;
                }
            }
            if has_unpack {
                let callable_is_borrowed = direct_simple_expr_is_borrowable(
                    func.as_ref(),
                    local_names,
                    &ctx.function_state_slots,
                );
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
                            let value_borrowed = direct_simple_expr_is_borrowable(
                                value_expr,
                                local_names,
                                &ctx.function_state_slots,
                            );
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
                            let value_borrowed = direct_simple_expr_is_borrowable(
                                value,
                                local_names,
                                &ctx.function_state_slots,
                            );
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
                            let value_borrowed = direct_simple_expr_is_borrowable(
                                value_expr,
                                local_names,
                                &ctx.function_state_slots,
                            );
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
                    && func_name.id.as_str() == "__dp_decode_literal_bytes"
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
                if keywords.is_empty() && func_name.id.as_str() == "str" && args.len() == 1 {
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
                    && (func_name.id.as_str() == "globals"
                        || func_name.id.as_str() == "__dp_globals")
                {
                    let globals_inst = fb.ins().call(function_globals_ref, &[callable_value]);
                    let globals_value = fb.inst_results(globals_inst)[0];
                    let globals_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, globals_value, null_ptr);
                    let globals_ok_block = fb.create_block();
                    fb.append_block_param(globals_ok_block, ptr_ty);
                    fb.ins().brif(
                        globals_is_null,
                        step_null_block,
                        &step_null_block_args(ctx),
                        globals_ok_block,
                        &[ir::BlockArg::Value(globals_value)],
                    );
                    fb.switch_to_block(globals_ok_block);
                    return fb.block_params(globals_ok_block)[0];
                }
                if keywords.is_empty() {
                    if func_name.id.as_str() == "__dp_tuple" {
                        let mut arg_values: Vec<ir::Value> = Vec::with_capacity(args.len());
                        let mut borrowed_args: Vec<bool> = Vec::with_capacity(args.len());
                        for arg in &args {
                            let borrowed_arg = direct_simple_expr_is_borrowable(
                                arg,
                                local_names,
                                &ctx.function_state_slots,
                            );
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
                    if func_name.id.as_str() == "__dp_load_deleted_name" && args.len() == 2 {
                        if let Some(name) = direct_simple_expr_const_string(args[0]) {
                            let (name_ptr, name_len) =
                                intern_bytes_literal(literal_pool, name.as_bytes());
                            let value_borrowed = direct_simple_expr_is_borrowable(
                                args[1],
                                local_names,
                                &ctx.function_state_slots,
                            );
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
                        (func_name.id.as_str(), args.len()),
                        ("__dp_cell_ref", 1)
                            | ("__dp_make_cell", 1)
                            | ("__dp_load_cell", 1)
                            | ("__dp_store_cell", 2)
                    );
                    if is_direct_cell_call {
                        if matches!((func_name.id.as_str(), args.len()), ("__dp_cell_ref", 1)) {
                            let cell_expr = &args[0];
                            let DirectSimpleExprPlan::Name(cell_name) = cell_expr else {
                                panic!(
                                    "__dp_cell_ref should lower to a located name arg, got {:?}",
                                    cell_expr
                                );
                            };
                            match cell_name.location {
                                NameLocation::OwnedCell { .. }
                                | NameLocation::ClosureCell { .. }
                                | NameLocation::CapturedCellSource { .. } => {
                                    assert!(
                                        !borrowed,
                                        "__dp_cell_ref should produce an owned cell object"
                                    );
                                    return emit_raw_cell_object_for_name(
                                        fb,
                                        cell_name,
                                        local_names,
                                        local_values,
                                        ctx,
                                    );
                                }
                                _ => {
                                    panic!(
                                        "__dp_cell_ref should target a cell-backed name, got {} at {:?}",
                                        cell_name.id, cell_name.location
                                    );
                                }
                            }
                        }
                        let mut arg_values: Vec<(ir::Value, bool)> = Vec::with_capacity(args.len());
                        for (arg_index, arg) in args.iter().enumerate() {
                            let raw_cell_arg = arg_index == 0
                                && matches!(
                                    func_name.id.as_str(),
                                    "__dp_load_cell" | "__dp_store_cell"
                                );
                            if raw_cell_arg {
                                let DirectSimpleExprPlan::Name(cell_name) = arg else {
                                    panic!(
                                        "{} should lower to a located name arg, got {:?}",
                                        func_name.id.as_str(),
                                        arg
                                    );
                                };
                                match cell_name.location {
                                    NameLocation::OwnedCell { .. }
                                    | NameLocation::ClosureCell { .. }
                                    | NameLocation::CapturedCellSource { .. } => {
                                        let value = emit_raw_cell_object_for_name(
                                            fb,
                                            cell_name,
                                            local_names,
                                            local_values,
                                            ctx,
                                        );
                                        arg_values.push((value, false));
                                        continue;
                                    }
                                    _ => {
                                        panic!(
                                            "{} should target a cell-backed name, got {} at {:?}",
                                            func_name.id, cell_name.id, cell_name.location
                                        );
                                    }
                                }
                            }
                            let borrowed_arg = direct_simple_expr_is_borrowable(
                                arg,
                                local_names,
                                &ctx.function_state_slots,
                            );
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
                        let call_inst = match (func_name.id.as_str(), args.len()) {
                            ("__dp_make_cell", 1) => {
                                fb.ins().call(make_cell_ref, &[arg_values[0].0])
                            }
                            ("__dp_load_cell", 1) => {
                                fb.ins().call(load_cell_ref, &[arg_values[0].0])
                            }
                            ("__dp_store_cell", 2) => fb
                                .ins()
                                .call(store_cell_ref, &[arg_values[0].0, arg_values[1].0]),
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
                }
            }
            let callable = emit_direct_simple_expr(
                fb,
                func.as_ref(),
                local_names,
                local_values,
                ctx,
                literal_pool,
                direct_simple_expr_is_borrowable(
                    func.as_ref(),
                    local_names,
                    &ctx.function_state_slots,
                ),
                jit_module,
                func_imports,
            );
            let callable_is_borrowed = direct_simple_expr_is_borrowable(
                func.as_ref(),
                local_names,
                &ctx.function_state_slots,
            );
            if keywords.is_empty() && args.len() <= 3 {
                let mut arg_values = [null_ptr, null_ptr, null_ptr];
                let mut arg_borrowed = [true, true, true];
                for (idx, arg) in args.iter().enumerate() {
                    let borrowed_arg = direct_simple_expr_is_borrowable(
                        arg,
                        local_names,
                        &ctx.function_state_slots,
                    );
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
                let borrowed_arg =
                    direct_simple_expr_is_borrowable(arg, local_names, &ctx.function_state_slots);
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

                    let value_borrowed = direct_simple_expr_is_borrowable(
                        value_expr,
                        local_names,
                        &ctx.function_state_slots,
                    );
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
    full_target_params: Option<&[String]>,
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
    let explicit_arg_offsets = match (full_target_params, explicit_args) {
        (Some(full_target_params), Some(explicit_args)) => {
            let explicit_start = full_target_params.len().saturating_sub(explicit_args.len());
            Some(
                full_target_params[explicit_start..]
                    .iter()
                    .enumerate()
                    .map(|(offset, name)| (name.as_str(), offset))
                    .collect::<HashMap<_, _>>(),
            )
        }
        _ => None,
    };
    for name in target_params {
        if let Some(explicit_arg) = explicit_args.and_then(|args| {
            explicit_arg_offsets
                .as_ref()
                .and_then(|offsets| offsets.get(name.as_str()).copied())
                .and_then(|offset| args.get(offset))
        }) {
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
                    } else if let Some(value) = load_function_state_value(
                        fb,
                        &ctx.function_state_slots,
                        source_name,
                        ctx.consts.ptr_ty,
                        false,
                        ctx.incref_ref,
                    ) {
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
        if let Some(value) = load_function_state_value(
            fb,
            &ctx.function_state_slots,
            name,
            ctx.consts.ptr_ty,
            false,
            ctx.incref_ref,
        ) {
            args.push(ir::BlockArg::Value(value));
            continue;
        }
        fb.ins().call(ctx.incref_ref, &[ctx.consts.none_const]);
        args.push(ir::BlockArg::Value(ctx.consts.none_const));
    }
    Some(args)
}

fn emit_explicit_target_slot_writes(
    fb: &mut FunctionBuilder<'_>,
    full_target_params: &[String],
    runtime_target_params: &[String],
    explicit_args: &[DirectSimpleBlockArgPlan],
    local_names: &[String],
    local_values: &[ir::Value],
    ctx: &DirectSimpleEmitCtx,
    literal_pool: &mut Vec<Box<[u8]>>,
    jit_module: &mut JITModule,
    func_imports: &mut FuncBuildImports<'_>,
) -> Option<()> {
    let explicit_start = full_target_params.len().saturating_sub(explicit_args.len());
    for (offset, arg) in explicit_args.iter().enumerate() {
        let target_name = &full_target_params[explicit_start + offset];
        if runtime_target_params.iter().any(|name| name == target_name) {
            continue;
        }
        let (value, owned_value) = match arg {
            DirectSimpleBlockArgPlan::Name(source_name) => {
                if let Some(index) = local_names
                    .iter()
                    .position(|candidate| candidate == source_name)
                {
                    (local_values[index], false)
                } else if let Some(value) = load_function_state_value(
                    fb,
                    &ctx.function_state_slots,
                    source_name,
                    ctx.consts.ptr_ty,
                    true,
                    ctx.incref_ref,
                ) {
                    (value, false)
                } else {
                    return None;
                }
            }
            DirectSimpleBlockArgPlan::Expr(expr) => (
                emit_direct_simple_expr(
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
                true,
            ),
            DirectSimpleBlockArgPlan::None => (ctx.consts.none_const, false),
            DirectSimpleBlockArgPlan::CurrentException => return None,
        };
        ctx.function_state_slots
            .replace_cloned_value(
                fb,
                target_name,
                value,
                ctx.consts.ptr_ty,
                ctx.incref_ref,
                ctx.decref_ref,
            )
            .expect("explicit edge slot target missing from function state slots");
        if owned_value {
            fb.ins().call(ctx.decref_ref, &[value]);
        }
    }
    Some(())
}

fn emit_exception_dispatch_slot_writes(
    fb: &mut FunctionBuilder<'_>,
    slot_writes: &[(String, BlockExcArgSource)],
    dispatch_exc: ir::Value,
    function_state_slots: &FunctionStateSlots,
    ptr_ty: ir::Type,
    none_const: ir::Value,
    incref_ref: ir::FuncRef,
    decref_ref: ir::FuncRef,
) -> Result<(), String> {
    for (target_name, source) in slot_writes {
        let value = match source {
            BlockExcArgSource::Name(source_name) => load_function_state_value(
                fb,
                function_state_slots,
                source_name,
                ptr_ty,
                true,
                incref_ref,
            )
            .ok_or_else(|| {
                format!(
                    "missing exception dispatch slot source {source_name} for target {target_name}"
                )
            })?,
            BlockExcArgSource::Exception => dispatch_exc,
            BlockExcArgSource::NoneValue => none_const,
        };
        function_state_slots
            .replace_cloned_value(fb, target_name, value, ptr_ty, incref_ref, decref_ref)
            .expect("exception dispatch slot target missing from function state slots");
    }
    Ok(())
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
    for op in ops {
        match op {
            DirectSimpleOpPlan::Assign(assign) => {
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
                    assign.target.id.as_str(),
                    value,
                    function_state_slots,
                    emit_ctx.consts.ptr_ty,
                    emit_ctx.incref_ref,
                    emit_ctx.decref_ref,
                );
            }
            DirectSimpleOpPlan::Expr(expr) => {
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
                        name.id.as_str(),
                        function_state_slots,
                        emit_ctx.consts.deleted_const,
                        emit_ctx.consts.ptr_ty,
                        emit_ctx.incref_ref,
                        emit_ctx.decref_ref,
                    )?;
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
        SigType::F64 => ir::types::F64,
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

    let mut main_sig = jit_module.make_signature();
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    for _ in &plan.entry_param_names {
        main_sig.params.push(ir::AbiParam::new(ptr_ty));
    }
    main_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let main_id = declare_local_fn(jit_module, "dp_jit_run_bb_specialized", &main_sig)?;

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
            .map(|block| block.runtime_param_names.clone())
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
        }
        fb.append_block_param(step_null_block, ptr_ty); // args
        fb.append_block_param(raise_exc_direct_block, ptr_ty); // args
        fb.append_block_param(raise_exc_direct_block, ptr_ty); // exc

        fb.switch_to_block(entry_block);
        let entry_block_params = fb.block_params(entry_block).to_vec();
        let callable = entry_block_params[0];
        let direct_entry_args = entry_block_params[1..].to_vec();
        let mut func_imports = FuncBuildImports::new(&mut module_imports);
        let incref_ref = func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_INCREF_IMPORT);
        let decref_ref = func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_DECREF_IMPORT);
        let py_call_positional_three_ref = func_imports.get_or_panic(
            jit_module,
            &mut fb.func,
            &DP_JIT_PY_CALL_POSITIONAL_THREE_IMPORT,
        );
        let py_call_object_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_PY_CALL_OBJECT_IMPORT);
        let py_call_with_kw_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_PY_CALL_WITH_KW_IMPORT);
        let get_raised_exception_ref = func_imports.get_or_panic(
            jit_module,
            &mut fb.func,
            &DP_JIT_GET_RAISED_EXCEPTION_IMPORT,
        );
        let make_int_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_MAKE_INT_IMPORT);
        let is_true_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_IS_TRUE_IMPORT);
        let raise_exc_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_RAISE_FROM_EXC_IMPORT);
        let make_float_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_MAKE_FLOAT_IMPORT);
        let load_name_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_LOAD_NAME_IMPORT);
        let function_globals_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_FUNCTION_GLOBALS_IMPORT);
        let function_closure_cell_ref = func_imports.get_or_panic(
            jit_module,
            &mut fb.func,
            &DP_JIT_FUNCTION_CLOSURE_CELL_IMPORT,
        );
        let function_positional_default_ref = func_imports.get_or_panic(
            jit_module,
            &mut fb.func,
            &DP_JIT_FUNCTION_POSITIONAL_DEFAULT_IMPORT,
        );
        let function_kwonly_default_ref = func_imports.get_or_panic(
            jit_module,
            &mut fb.func,
            &DP_JIT_FUNCTION_KWONLY_DEFAULT_IMPORT,
        );
        let pyobject_getattr_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_PYOBJECT_GETATTR_IMPORT);
        let pyobject_setattr_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_PYOBJECT_SETATTR_IMPORT);
        let pyobject_getitem_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_PYOBJECT_GETITEM_IMPORT);
        let pyobject_setitem_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_PYOBJECT_SETITEM_IMPORT);
        let pyobject_to_i64_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_PYOBJECT_TO_I64_IMPORT);
        let decode_literal_bytes_ref = func_imports.get_or_panic(
            jit_module,
            &mut fb.func,
            &DP_JIT_DECODE_LITERAL_BYTES_IMPORT,
        );
        let load_deleted_name_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_LOAD_DELETED_NAME_IMPORT);
        let make_cell_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_MAKE_CELL_IMPORT);
        let load_cell_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_LOAD_CELL_IMPORT);
        let store_cell_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_STORE_CELL_IMPORT);
        let make_bytes_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_MAKE_BYTES_IMPORT);
        let tuple_new_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_TUPLE_NEW_IMPORT);
        let tuple_set_item_ref =
            func_imports.get_or_panic(jit_module, &mut fb.func, &DP_JIT_TUPLE_SET_ITEM_IMPORT);

        let entry_deleted_const = fb.ins().iconst(ptr_ty, deleted_obj as i64);
        function_state_slots.initialize_all_to_value(&mut fb, entry_deleted_const, incref_ref);

        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        let entry_failure_block = cleanup_null_blocks[0];
        let entry_failure_args = Vec::new();
        assert_eq!(
            direct_entry_args.len(),
            plan.entry_param_names.len(),
            "direct JIT entry arity does not match entry_param_names",
        );
        assert_eq!(
            plan.entry_param_names.len(),
            plan.entry_param_default_sources.len(),
            "direct JIT entry default metadata does not match entry params",
        );
        for ((param_name, default_source), value) in plan
            .entry_param_names
            .iter()
            .zip(plan.entry_param_default_sources.iter())
            .zip(direct_entry_args.iter())
        {
            match default_source {
                Some(ClifEntryParamDefaultSource::Positional(default_index)) => {
                    let arg_is_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, *value, null_ptr);
                    let use_default_block = fb.create_block();
                    let use_arg_block = fb.create_block();
                    let after_block = fb.create_block();
                    fb.ins()
                        .brif(arg_is_null, use_default_block, &[], use_arg_block, &[]);

                    fb.switch_to_block(use_default_block);
                    let (name_ptr, name_len) =
                        intern_bytes_literal(&mut literal_pool, param_name.as_bytes());
                    let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
                    let name_len_val = fb.ins().iconst(i64_ty, name_len);
                    let default_index_val = fb.ins().iconst(i64_ty, *default_index as i64);
                    let default_inst = fb.ins().call(
                        function_positional_default_ref,
                        &[callable, name_ptr_val, name_len_val, default_index_val],
                    );
                    let default_value = fb.inst_results(default_inst)[0];
                    let default_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, default_value, null_ptr);
                    let default_ok_block = fb.create_block();
                    fb.append_block_param(default_ok_block, ptr_ty);
                    fb.ins().brif(
                        default_is_null,
                        entry_failure_block,
                        &entry_failure_args,
                        default_ok_block,
                        &[ir::BlockArg::Value(default_value)],
                    );
                    fb.switch_to_block(default_ok_block);
                    let default_value = fb.block_params(default_ok_block)[0];
                    function_state_slots
                        .replace_cloned_value(
                            &mut fb,
                            param_name,
                            default_value,
                            ptr_ty,
                            incref_ref,
                            decref_ref,
                        )
                        .expect("entry slot missing from function state slots");
                    fb.ins().call(decref_ref, &[default_value]);
                    fb.ins().jump(after_block, &[]);

                    fb.switch_to_block(use_arg_block);
                    function_state_slots
                        .replace_cloned_value(
                            &mut fb, param_name, *value, ptr_ty, incref_ref, decref_ref,
                        )
                        .expect("entry slot missing from function state slots");
                    fb.ins().jump(after_block, &[]);

                    fb.switch_to_block(after_block);
                }
                Some(ClifEntryParamDefaultSource::KeywordOnly(default_name)) => {
                    let arg_is_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, *value, null_ptr);
                    let use_default_block = fb.create_block();
                    let use_arg_block = fb.create_block();
                    let after_block = fb.create_block();
                    fb.ins()
                        .brif(arg_is_null, use_default_block, &[], use_arg_block, &[]);

                    fb.switch_to_block(use_default_block);
                    let (name_ptr, name_len) =
                        intern_bytes_literal(&mut literal_pool, default_name.as_bytes());
                    let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
                    let name_len_val = fb.ins().iconst(i64_ty, name_len);
                    let default_inst = fb.ins().call(
                        function_kwonly_default_ref,
                        &[callable, name_ptr_val, name_len_val],
                    );
                    let default_value = fb.inst_results(default_inst)[0];
                    let default_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, default_value, null_ptr);
                    let default_ok_block = fb.create_block();
                    fb.append_block_param(default_ok_block, ptr_ty);
                    fb.ins().brif(
                        default_is_null,
                        entry_failure_block,
                        &entry_failure_args,
                        default_ok_block,
                        &[ir::BlockArg::Value(default_value)],
                    );
                    fb.switch_to_block(default_ok_block);
                    let default_value = fb.block_params(default_ok_block)[0];
                    function_state_slots
                        .replace_cloned_value(
                            &mut fb,
                            param_name,
                            default_value,
                            ptr_ty,
                            incref_ref,
                            decref_ref,
                        )
                        .expect("entry slot missing from function state slots");
                    fb.ins().call(decref_ref, &[default_value]);
                    fb.ins().jump(after_block, &[]);

                    fb.switch_to_block(use_arg_block);
                    function_state_slots
                        .replace_cloned_value(
                            &mut fb, param_name, *value, ptr_ty, incref_ref, decref_ref,
                        )
                        .expect("entry slot missing from function state slots");
                    fb.ins().jump(after_block, &[]);

                    fb.switch_to_block(after_block);
                }
                None => {
                    function_state_slots
                        .replace_cloned_value(
                            &mut fb, param_name, *value, ptr_ty, incref_ref, decref_ref,
                        )
                        .expect("entry slot missing from function state slots");
                }
            }
        }

        let mut entry_jump_args = Vec::with_capacity(runtime_block_param_names[0].len());
        for param_name in &runtime_block_param_names[0] {
            let value = load_function_state_value(
                &mut fb,
                &function_state_slots,
                param_name,
                ptr_ty,
                false,
                incref_ref,
            )
            .expect("entry runtime param missing from function state slots");
            entry_jump_args.push(ir::BlockArg::Value(value));
        }
        fb.ins().jump(exec_blocks[0], &entry_jump_args);

        let mut exception_dispatch_blocks: Vec<Option<ir::Block>> = vec![None; exec_blocks.len()];
        for (index, block) in plan.blocks.iter().enumerate() {
            if block.exc_dispatch.is_some() {
                let dispatch_block = fb.create_block();
                exception_dispatch_blocks[index] = Some(dispatch_block);
            }
        }

        for (index, block) in exec_blocks.iter().enumerate() {
            fb.switch_to_block(*block);
            let block_param_values = fb.block_params(*block).to_vec();
            for (param_name, param_value) in runtime_block_param_names[index]
                .iter()
                .zip(block_param_values.iter())
            {
                function_state_slots
                    .replace_cloned_value(
                        &mut fb,
                        param_name,
                        *param_value,
                        ptr_ty,
                        incref_ref,
                        decref_ref,
                    )
                    .expect("runtime block param missing from function state slots");
                fb.ins().call(decref_ref, &[*param_value]);
            }
            let block_const = fb.ins().iconst(ptr_ty, globals_obj as i64);
            let none_const = fb.ins().iconst(ptr_ty, none_obj as i64);
            let true_const = fb.ins().iconst(ptr_ty, true_obj as i64);
            let false_const = fb.ins().iconst(ptr_ty, false_obj as i64);
            let deleted_const = fb.ins().iconst(ptr_ty, deleted_obj as i64);
            let empty_tuple_const = fb.ins().iconst(ptr_ty, empty_tuple_obj as i64);
            let fast_step_null_block =
                exception_dispatch_blocks[index].unwrap_or(cleanup_null_blocks[index]);
            let fast_step_null_args = Vec::new();
            let emit_ctx = DirectSimpleEmitCtx {
                owned_cell_slot_names: plan.owned_cell_slot_names.clone(),
                incref_ref,
                decref_ref,
                py_call_positional_three_ref,
                make_int_ref,
                consts: DirectSimpleEmitConsts {
                    step_null_block: fast_step_null_block,
                    step_null_args: fast_step_null_args,
                    ptr_ty,
                    i64_ty,
                    callable_value: callable,
                    none_const,
                    true_const,
                    false_const,
                    deleted_const,
                    empty_tuple_const,
                    block_const,
                },
                load_name_ref,
                function_globals_ref,
                function_closure_cell_ref,
                pyobject_getattr_ref,
                pyobject_setattr_ref,
                pyobject_getitem_ref,
                pyobject_setitem_ref,
                decode_literal_bytes_ref,
                load_deleted_name_ref,
                make_cell_ref,
                load_cell_ref,
                store_cell_ref,
                make_bytes_ref,
                make_float_ref,
                py_call_object_ref,
                py_call_with_kw_ref,
                tuple_new_ref,
                tuple_set_item_ref,
                function_state_slots: function_state_slots.clone(),
            };
            match &plan.blocks[index].fast_path {
                BlockFastPath::JumpPassThrough { target_index } => {
                    fb.ins().jump(exec_blocks[*target_index], &[]);
                    continue;
                }
                BlockFastPath::DirectSimpleBrIf { plan } => {
                    let local_names = Vec::new();
                    let local_values = Vec::new();

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
                    fb.ins().brif(
                        is_true,
                        exec_blocks[plan.then_index],
                        &[],
                        exec_blocks[plan.else_index],
                        &[],
                    );
                    continue;
                }
                BlockFastPath::DirectSimpleRet { plan } => {
                    let mut local_names = Vec::new();
                    let mut local_values = Vec::new();

                    for assign in &plan.assigns {
                        let value = emit_direct_simple_expr(
                            &mut fb,
                            &assign.value,
                            &local_names,
                            &local_values,
                            &emit_ctx,
                            &mut literal_pool,
                            false,
                            jit_module,
                            &mut func_imports,
                        );

                        bind_local_value(
                            &mut fb,
                            &mut local_names,
                            &mut local_values,
                            assign.target.id.as_str(),
                            value,
                            &function_state_slots,
                            ptr_ty,
                            incref_ref,
                            decref_ref,
                        );
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

                    for value in local_values {
                        fb.ins().call(decref_ref, &[value]);
                    }
                    function_state_slots.decref_all(&mut fb, ptr_ty, decref_ref);
                    fb.ins().return_(&[ret_value]);
                    continue;
                }
                BlockFastPath::DirectSimpleBlock { plan: block_plan } => {
                    let mut local_names = Vec::new();
                    let mut local_values = Vec::new();

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
                            full_target_params,
                            target_args,
                        } => {
                            let target_params = &runtime_block_param_names[*target_index];
                            emit_explicit_target_slot_writes(
                                &mut fb,
                                full_target_params,
                                target_params,
                                target_args,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                jit_module,
                                &mut func_imports,
                            )
                            .ok_or_else(|| {
                                format!(
                                    "missing local mapping for jump slot updates in block {}",
                                    plan.blocks[index].label
                                )
                            })?;
                            let mut jump_args = Vec::with_capacity(target_params.len());
                            jump_args.extend(
                                emit_prepare_target_args(
                                    &mut fb,
                                    target_params,
                                    Some(full_target_params),
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
                                py_call_positional_three_ref,
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
                            emit_decref_unforwarded_locals(
                                &mut fb,
                                &local_values,
                                &local_names,
                                &[],
                                decref_ref,
                            );
                            fb.ins().jump(
                                emit_ctx.consts.step_null_block,
                                &step_null_block_args(&emit_ctx),
                            );
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
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let none_const = fb.ins().iconst(ptr_ty, none_obj as i64);
            let dispatch_step_null_args = Vec::new();

            let raised_exc_inst = fb.ins().call(get_raised_exception_ref, &[]);
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
            let dispatch_exc = fb.block_params(raised_exc_ok)[0];
            emit_exception_dispatch_slot_writes(
                &mut fb,
                &dispatch_plan.slot_writes,
                dispatch_exc,
                &function_state_slots,
                ptr_ty,
                none_const,
                incref_ref,
                decref_ref,
            )?;
            let target_runtime_params = &runtime_block_param_names[dispatch_plan.target_index];
            let mut target_jump_args = Vec::with_capacity(target_runtime_params.len());
            if target_runtime_params.is_empty() {
                fb.ins().call(decref_ref, &[dispatch_exc]);
            } else {
                target_jump_args.push(ir::BlockArg::Value(dispatch_exc));
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
        function_state_slots.decref_all(&mut fb, ptr_ty, decref_ref);
        fb.ins().return_(&[red_null]);

        fb.seal_all_blocks();
        fb.finalize();
    }

    Ok((
        ctx,
        main_id,
        literal_pool,
        module_imports.debug_symbols().clone(),
    ))
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
    compiled.entry = Some(CompiledRunnerEntry::Direct {
        code_ptr,
        param_count: plan.entry_param_names.len(),
    });
    compiled._literal_pool = literal_pool;
    Ok(Box::into_raw(compiled) as ObjPtr)
}

fn compiled_direct_runner_info(compiled_handle: ObjPtr) -> Result<(*const u8, usize), String> {
    if compiled_handle.is_null() {
        return Err("invalid null compiled handle for direct vectorcall trampoline".to_string());
    }
    let compiled = unsafe { &*(compiled_handle as *const CompiledSpecializedRunner) };
    match compiled.entry {
        Some(CompiledRunnerEntry::Direct {
            code_ptr,
            param_count,
        }) => Ok((code_ptr, param_count)),
        None => Err("invalid compiled handle without entrypoint".to_string()),
    }
}

pub unsafe fn compile_cranelift_vectorcall_direct_trampoline(
    bind_direct_args_fn: unsafe extern "C" fn(
        ObjPtr,
        *const ObjPtr,
        usize,
        ObjPtr,
        ObjPtr,
        *mut ObjPtr,
        i64,
    ) -> i32,
    data_ptr: ObjPtr,
    compiled_handle: ObjPtr,
) -> Result<(ObjPtr, VectorcallEntryFn), String> {
    if data_ptr.is_null() {
        return Err("invalid null vectorcall data pointer".to_string());
    }
    let (direct_code_ptr, param_count) = compiled_direct_runner_info(compiled_handle)?;

    let mut builder = new_jit_builder()?;
    builder.symbol(
        "dp_jit_vectorcall_bind_direct_args",
        bind_direct_args_fn as *const u8,
    );
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    let mut jit_module = JITModule::new(builder);
    let ptr_ty = jit_module.target_config().pointer_type();
    let i64_ty = ir::types::I64;
    let mut module_imports = ModuleFuncImports::new();

    let mut main_sig = jit_module.make_signature();
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.params.push(ir::AbiParam::new(ptr_ty));
    main_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let main_id = declare_local_fn(
        &mut jit_module,
        "dp_jit_vectorcall_direct_trampoline",
        &main_sig,
    )?;

    let mut direct_sig = jit_module.make_signature();
    direct_sig.params.push(ir::AbiParam::new(ptr_ty));
    for _ in 0..param_count {
        direct_sig.params.push(ir::AbiParam::new(ptr_ty));
    }
    direct_sig.returns.push(ir::AbiParam::new(ptr_ty));

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

        let mut func_imports = FuncBuildImports::new(&mut module_imports);
        let bind_ref = func_imports.get_or_panic(
            &mut jit_module,
            &mut fb.func,
            &DP_JIT_VECTORCALL_BIND_DIRECT_ARGS_IMPORT,
        );
        let decref_ref =
            func_imports.get_or_panic(&mut jit_module, &mut fb.func, &DP_JIT_DECREF_IMPORT);

        let data_const = fb.ins().iconst(ptr_ty, data_ptr as i64);
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        let bound_args_slot = if param_count == 0 {
            None
        } else {
            Some(fb.create_sized_stack_slot(ir::StackSlotData::new(
                ir::StackSlotKind::ExplicitSlot,
                (param_count * std::mem::size_of::<u64>()) as u32,
                0,
            )))
        };
        let bound_args_ptr = if let Some(slot) = bound_args_slot {
            fb.ins().stack_addr(ptr_ty, slot, 0)
        } else {
            null_ptr
        };
        let out_len = fb.ins().iconst(i64_ty, param_count as i64);
        let bind_inst = fb.ins().call(
            bind_ref,
            &[
                callable_val,
                args_val,
                nargsf_val,
                kwnames_val,
                data_const,
                bound_args_ptr,
                out_len,
            ],
        );
        let bind_ok = fb.inst_results(bind_inst)[0];
        let bind_failed = fb.ins().icmp_imm(ir::condcodes::IntCC::Equal, bind_ok, 0);
        let fail_block = fb.create_block();
        let ok_block = fb.create_block();
        fb.ins().brif(bind_failed, fail_block, &[], ok_block, &[]);
        fb.seal_block(fail_block);
        fb.seal_block(ok_block);

        fb.switch_to_block(fail_block);
        fb.ins().return_(&[null_ptr]);

        fb.switch_to_block(ok_block);
        let direct_sig_ref = fb.import_signature(direct_sig);
        let mut call_args = Vec::with_capacity(param_count + 1);
        call_args.push(callable_val);
        let mut owned_args = Vec::with_capacity(param_count);
        if let Some(slot) = bound_args_slot {
            for index in 0..param_count {
                let value =
                    fb.ins()
                        .stack_load(ptr_ty, slot, (index * std::mem::size_of::<u64>()) as i32);
                owned_args.push(value);
                call_args.push(value);
            }
        }
        let callee_ptr = fb.ins().iconst(ptr_ty, direct_code_ptr as i64);
        let call_inst = fb
            .ins()
            .call_indirect(direct_sig_ref, callee_ptr, &call_args);
        let result = fb.inst_results(call_inst)[0];
        for value in owned_args {
            fb.ins().call(decref_ref, &[value]);
        }
        fb.ins().return_(&[result]);
        fb.seal_all_blocks();
        fb.finalize();
    }

    define_function_with_incremental_cache(
        &mut jit_module,
        main_id,
        &mut ctx,
        "failed to define direct vectorcall trampoline",
    )?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize direct vectorcall trampoline: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(main_id);
    let entry: VectorcallEntryFn = std::mem::transmute(code_ptr);
    let compiled = Box::new(CompiledVectorcallRunner {
        _jit_module: jit_module,
    });
    Ok((Box::into_raw(compiled) as ObjPtr, entry))
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

#[cfg(test)]
mod test;

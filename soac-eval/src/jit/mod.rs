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
use dp_transform::basic_block::bb_ir::{BbBlock, BbExpr, BbModule, BbOp, BbTerm};
use ruff_python_ast::Number;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ptr;
use std::sync::{Mutex, OnceLock};

mod exception_pass;

type ObjPtr = *mut c_void;
type IncrefFn = unsafe extern "C" fn(ObjPtr);
type DecrefFn = unsafe extern "C" fn(ObjPtr);
type CallOneArgFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
type CallTwoArgsFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
type CallVarArgsFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
type CallObjectFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
type CallWithKwFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
type GetRaisedExceptionFn = unsafe extern "C" fn() -> ObjPtr;
type GetArgItemFn = unsafe extern "C" fn(ObjPtr, i64) -> ObjPtr;
type MakeIntFn = unsafe extern "C" fn(i64) -> ObjPtr;
type MakeFloatFn = unsafe extern "C" fn(f64) -> ObjPtr;
type MakeBytesFn = unsafe extern "C" fn(*const u8, i64) -> ObjPtr;
type LoadNameFn = unsafe extern "C" fn(ObjPtr, *const u8, i64) -> ObjPtr;
type LoadLocalRawByNameFn = unsafe extern "C" fn(ObjPtr, *const u8, i64) -> ObjPtr;
type PyObjectGetAttrFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
type PyObjectSetAttrFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
type PyObjectGetItemFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
type PyObjectSetItemFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
type PyObjectToI64Fn = unsafe extern "C" fn(ObjPtr) -> i64;
type DecodeLiteralBytesFn = unsafe extern "C" fn(*const u8, i64) -> ObjPtr;
type TupleNewFn = unsafe extern "C" fn(i64) -> ObjPtr;
type TupleSetItemFn = unsafe extern "C" fn(ObjPtr, i64, ObjPtr) -> i32;
type IsTrueFn = unsafe extern "C" fn(ObjPtr) -> i32;
type CompareEqFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> i32;
type CompareObjFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
type RaiseFromExcFn = unsafe extern "C" fn(ObjPtr) -> i32;

static mut DP_JIT_INCREF_FN: Option<IncrefFn> = None;
static mut DP_JIT_DECREF_FN: Option<DecrefFn> = None;
static mut DP_JIT_CALL_ONE_ARG_FN: Option<CallOneArgFn> = None;
static mut DP_JIT_CALL_TWO_ARGS_FN: Option<CallTwoArgsFn> = None;
static mut DP_JIT_CALL_VAR_ARGS_FN: Option<CallVarArgsFn> = None;
static mut DP_JIT_CALL_OBJECT_FN: Option<CallObjectFn> = None;
static mut DP_JIT_CALL_WITH_KW_FN: Option<CallWithKwFn> = None;
static mut DP_JIT_GET_RAISED_EXCEPTION_FN: Option<GetRaisedExceptionFn> = None;
static mut DP_JIT_GET_ARG_ITEM_FN: Option<GetArgItemFn> = None;
static mut DP_JIT_MAKE_INT_FN: Option<MakeIntFn> = None;
static mut DP_JIT_MAKE_FLOAT_FN: Option<MakeFloatFn> = None;
static mut DP_JIT_MAKE_BYTES_FN: Option<MakeBytesFn> = None;
static mut DP_JIT_LOAD_NAME_FN: Option<LoadNameFn> = None;
static mut DP_JIT_LOAD_LOCAL_RAW_BY_NAME_FN: Option<LoadLocalRawByNameFn> = None;
static mut DP_JIT_PYOBJECT_GETATTR_FN: Option<PyObjectGetAttrFn> = None;
static mut DP_JIT_PYOBJECT_SETATTR_FN: Option<PyObjectSetAttrFn> = None;
static mut DP_JIT_PYOBJECT_GETITEM_FN: Option<PyObjectGetItemFn> = None;
static mut DP_JIT_PYOBJECT_SETITEM_FN: Option<PyObjectSetItemFn> = None;
static mut DP_JIT_PYOBJECT_TO_I64_FN: Option<PyObjectToI64Fn> = None;
static mut DP_JIT_DECODE_LITERAL_BYTES_FN: Option<DecodeLiteralBytesFn> = None;
static mut DP_JIT_TUPLE_NEW_FN: Option<TupleNewFn> = None;
static mut DP_JIT_TUPLE_SET_ITEM_FN: Option<TupleSetItemFn> = None;
static mut DP_JIT_IS_TRUE_FN: Option<IsTrueFn> = None;
static mut DP_JIT_COMPARE_EQ_OBJ_FN: Option<CompareObjFn> = None;
static mut DP_JIT_COMPARE_LT_OBJ_FN: Option<CompareObjFn> = None;
static mut DP_JIT_RAISE_FROM_EXC_FN: Option<RaiseFromExcFn> = None;

static INCREMENTAL_CLIF_CACHE: OnceLock<Mutex<HashMap<Vec<u8>, Vec<u8>>>> = OnceLock::new();

fn incremental_clif_cache() -> &'static Mutex<HashMap<Vec<u8>, Vec<u8>>> {
    INCREMENTAL_CLIF_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

struct GlobalIncrementalCacheStore<'a> {
    map: &'a Mutex<HashMap<Vec<u8>, Vec<u8>>>,
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

#[derive(Clone, Debug)]
pub struct EntryBlockPlan {
    pub entry_index: usize,
    pub block_labels: Vec<String>,
    pub block_param_names: Vec<Vec<String>>,
    pub block_terms: Vec<BlockTermPlan>,
    pub block_exc_targets: Vec<Option<usize>>,
    pub block_exc_dispatches: Vec<Option<BlockExcDispatchPlan>>,
    pub block_fast_paths: Vec<BlockFastPath>,
}

#[derive(Clone, Debug)]
pub struct BlockExcDispatchPlan {
    pub target_index: usize,
    pub owner_param_index: Option<usize>,
    pub arg_sources: Vec<BlockExcArgSource>,
}

#[derive(Clone, Debug)]
pub enum BlockExcArgSource {
    SourceParam { index: usize },
    Exception,
    NoneValue,
    FrameLocal { name: String },
}

#[derive(Clone, Debug)]
pub enum BlockTermPlan {
    Jump {
        target_index: usize,
    },
    BrIf {
        then_index: usize,
        else_index: usize,
    },
    BrTable {
        targets: Vec<usize>,
        default_index: usize,
    },
    Raise,
    Ret,
}

#[derive(Clone, Debug)]
pub enum BlockFastPath {
    None,
    JumpPassThrough { target_index: usize },
    ReturnNone,
    DirectSimpleExprRetNone { plan: DirectSimpleExprRetNonePlan },
    DirectSimpleBrIf { plan: DirectSimpleBrIfPlan },
    DirectSimpleRet { plan: DirectSimpleRetPlan },
    DirectSimpleBlock { plan: DirectSimpleBlockPlan },
}

#[derive(Clone, Debug)]
pub enum DirectSimpleExprPlan {
    Name(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    None,
    Bytes(Vec<u8>),
    Tuple(Vec<DirectSimpleExprPlan>),
    Call {
        func: Box<DirectSimpleExprPlan>,
        parts: Vec<DirectSimpleCallPart>,
    },
}

#[derive(Clone, Debug)]
pub enum DirectSimpleCallPart {
    Pos(DirectSimpleExprPlan),
    Star(DirectSimpleExprPlan),
    Kw {
        name: String,
        value: DirectSimpleExprPlan,
    },
    KwStar(DirectSimpleExprPlan),
}

#[derive(Clone, Debug)]
pub struct DirectSimpleAssignPlan {
    pub target: String,
    pub value: DirectSimpleExprPlan,
}

#[derive(Clone, Debug)]
pub struct DirectSimpleRetPlan {
    pub params: Vec<String>,
    pub assigns: Vec<DirectSimpleAssignPlan>,
    pub ret: DirectSimpleExprPlan,
}

#[derive(Clone, Debug)]
pub struct DirectSimpleBrIfPlan {
    pub params: Vec<String>,
    pub test: DirectSimpleExprPlan,
    pub then_index: usize,
    pub else_index: usize,
}

#[derive(Clone, Debug)]
pub struct DirectSimpleExprRetNonePlan {
    pub params: Vec<String>,
    pub exprs: Vec<DirectSimpleExprPlan>,
}

#[derive(Clone, Debug)]
pub enum DirectSimpleOpPlan {
    Assign(DirectSimpleAssignPlan),
    Expr(DirectSimpleExprPlan),
    Delete(DirectSimpleDeletePlan),
}

#[derive(Clone, Debug)]
pub enum DirectSimpleTermPlan {
    Jump {
        target_index: usize,
        target_params: Vec<String>,
    },
    BrIf {
        test: DirectSimpleExprPlan,
        then_index: usize,
        then_params: Vec<String>,
        else_index: usize,
        else_params: Vec<String>,
    },
    BrTable {
        index: DirectSimpleExprPlan,
        targets: Vec<(usize, Vec<String>)>,
        default_index: usize,
        default_params: Vec<String>,
    },
    Ret {
        value: Option<DirectSimpleExprPlan>,
    },
    Raise {
        exc: Option<DirectSimpleExprPlan>,
        cause: Option<DirectSimpleExprPlan>,
    },
}

#[derive(Clone, Debug)]
pub struct DirectSimpleDeletePlan {
    pub targets: Vec<DirectSimpleDeleteTargetPlan>,
}

#[derive(Clone, Debug)]
pub enum DirectSimpleDeleteTargetPlan {
    LocalName(String),
}

#[derive(Clone, Debug)]
pub struct DirectSimpleBlockPlan {
    pub params: Vec<String>,
    pub ops: Vec<DirectSimpleOpPlan>,
    pub term: DirectSimpleTermPlan,
}

type PlanRegistry = HashMap<PlanKey, EntryBlockPlan>;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PlanKey {
    pub module: String,
    pub qualname: String,
}

#[derive(Debug, Clone)]
pub struct RenderedSpecializedClif {
    pub clif: String,
    pub cfg_dot: String,
}

struct CompiledSpecializedRunner {
    _jit_module: JITModule,
    _literal_pool: Vec<Box<[u8]>>,
    entry: Option<extern "C" fn(ObjPtr) -> ObjPtr>,
}

static BB_PLAN_REGISTRY: OnceLock<Mutex<PlanRegistry>> = OnceLock::new();

fn bb_plan_registry() -> &'static Mutex<PlanRegistry> {
    BB_PLAN_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn direct_simple_expr_from(expr: &BbExpr) -> Option<DirectSimpleExprPlan> {
    match expr {
        BbExpr::Await(_) => None,
        BbExpr::Name(name) => Some(DirectSimpleExprPlan::Name(name.id.to_string())),
        BbExpr::IntLiteral(number) => {
            let Number::Int(value) = &number.value else {
                panic!("BbExpr::IntLiteral contained a non-int value");
            };
            value.as_i64().map(DirectSimpleExprPlan::Int)
        }
        BbExpr::FloatLiteral(number) => {
            let Number::Float(value) = &number.value else {
                panic!("BbExpr::FloatLiteral contained a non-float value");
            };
            Some(DirectSimpleExprPlan::Float(*value))
        }
        BbExpr::BytesLiteral(bytes) => {
            let value: Cow<[u8]> = (&bytes.value).into();
            Some(DirectSimpleExprPlan::Bytes(value.into_owned()))
        }
        BbExpr::Starred(_) => None,
        BbExpr::Call(call) => {
            let func = direct_simple_expr_from(call.func.as_ref())?;
            let mut parts = Vec::with_capacity(call.args.len() + call.keywords.len());
            for arg in &call.args {
                match arg {
                    BbExpr::Starred(starred_expr) => {
                        let starred_value = BbExpr::from_expr(*starred_expr.value.clone());
                        parts.push(DirectSimpleCallPart::Star(direct_simple_expr_from(
                            &starred_value,
                        )?));
                    }
                    _ => {
                        parts.push(DirectSimpleCallPart::Pos(direct_simple_expr_from(arg)?));
                    }
                }
            }
            if call.template.arguments.keywords.len() != call.keywords.len() {
                return None;
            }
            for (keyword, value) in call
                .template
                .arguments
                .keywords
                .iter()
                .zip(call.keywords.iter())
            {
                let value = direct_simple_expr_from(value)?;
                if let Some(name) = keyword.arg.as_ref() {
                    parts.push(DirectSimpleCallPart::Kw {
                        name: name.to_string(),
                        value,
                    });
                } else {
                    parts.push(DirectSimpleCallPart::KwStar(value));
                }
            }
            Some(DirectSimpleExprPlan::Call {
                func: Box::new(func),
                parts,
            })
        }
    }
}

fn direct_simple_plan_from_block(
    block: &dp_transform::basic_block::bb_ir::BbBlock,
) -> Option<DirectSimpleRetPlan> {
    let mut known_names: Vec<String> = block.params.clone();
    let mut assigns = Vec::new();
    for op in &block.ops {
        let BbOp::Assign(assign) = op else {
            return None;
        };
        let value = direct_simple_expr_from(&assign.value)?;
        if !known_names
            .iter()
            .any(|candidate| candidate == assign.target.id.as_str())
        {
            known_names.push(assign.target.id.to_string());
        }
        assigns.push(DirectSimpleAssignPlan {
            target: assign.target.id.to_string(),
            value,
        });
    }
    let BbTerm::Ret(ret_value) = &block.term else {
        return None;
    };
    let ret = if let Some(value) = ret_value.as_ref() {
        direct_simple_expr_from(value)?
    } else {
        DirectSimpleExprPlan::None
    };
    Some(DirectSimpleRetPlan {
        params: block.params.clone(),
        assigns,
        ret,
    })
}

fn direct_simple_brif_plan_from_block(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
    block: &dp_transform::basic_block::bb_ir::BbBlock,
    label_to_index: &HashMap<String, usize>,
) -> Option<DirectSimpleBrIfPlan> {
    if !block.ops.is_empty() {
        return None;
    }
    let BbTerm::BrIf {
        test,
        then_label,
        else_label,
    } = &block.term
    else {
        return None;
    };
    let then_index = *label_to_index.get(then_label.as_str())?;
    let else_index = *label_to_index.get(else_label.as_str())?;
    let source_params = block.params.as_slice();
    if function.blocks[then_index].params.as_slice() != source_params
        || function.blocks[else_index].params.as_slice() != source_params
    {
        return None;
    }
    let test = direct_simple_expr_from(test)?;
    Some(DirectSimpleBrIfPlan {
        params: block.params.clone(),
        test,
        then_index,
        else_index,
    })
}

fn direct_simple_expr_ret_none_plan_from_block(
    block: &dp_transform::basic_block::bb_ir::BbBlock,
) -> Option<DirectSimpleExprRetNonePlan> {
    let mut exprs = Vec::new();
    for op in &block.ops {
        let BbOp::Expr(expr_op) = op else {
            return None;
        };
        let expr = direct_simple_expr_from(&expr_op.value)?;
        exprs.push(expr);
    }
    let BbTerm::Ret(ret_value) = &block.term else {
        return None;
    };
    if ret_value.is_some() {
        return None;
    }
    Some(DirectSimpleExprRetNonePlan {
        params: block.params.clone(),
        exprs,
    })
}

fn target_params_from_index(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
    target_index: usize,
) -> Option<Vec<String>> {
    Some(function.blocks.get(target_index)?.params.clone())
}

fn direct_simple_delete_plan_from_targets(
    targets: &[BbExpr],
    known_names: &mut Vec<String>,
) -> Option<DirectSimpleDeletePlan> {
    let mut plan_targets = Vec::with_capacity(targets.len());
    for target in targets {
        let BbExpr::Name(name) = target else {
            return None;
        };
        let target_name = name.id.to_string();
        if !known_names.iter().any(|known| known == &target_name) {
            return None;
        }
        plan_targets.push(DirectSimpleDeleteTargetPlan::LocalName(target_name.clone()));
        known_names.retain(|known| known != &target_name);
    }
    Some(DirectSimpleDeletePlan {
        targets: plan_targets,
    })
}

fn direct_simple_op_from_bb_op(
    op: &BbOp,
    known_names: &mut Vec<String>,
) -> Option<DirectSimpleOpPlan> {
    match op {
        BbOp::Expr(expr_stmt) => {
            let value = direct_simple_expr_from(&expr_stmt.value)?;
            Some(DirectSimpleOpPlan::Expr(value))
        }
        BbOp::Assign(assign) => {
            let value = direct_simple_expr_from(&assign.value)?;
            let target_name = assign.target.id.to_string();
            if !known_names.iter().any(|known| known == &target_name) {
                known_names.push(target_name.clone());
            }
            Some(DirectSimpleOpPlan::Assign(DirectSimpleAssignPlan {
                target: target_name,
                value,
            }))
        }
        BbOp::Delete(delete_stmt) => {
            let delete_plan =
                direct_simple_delete_plan_from_targets(&delete_stmt.targets, known_names)?;
            Some(DirectSimpleOpPlan::Delete(delete_plan))
        }
    }
}

fn bb_op_kind(op: &BbOp) -> &'static str {
    match op {
        BbOp::Assign(_) => "Assign",
        BbOp::Expr(_) => "Expr",
        BbOp::Delete(_) => "Delete",
    }
}

fn bb_term_kind(term: &BbTerm) -> &'static str {
    match term {
        BbTerm::Jump(_) => "Jump",
        BbTerm::BrIf { .. } => "BrIf",
        BbTerm::BrTable { .. } => "BrTable",
        BbTerm::TryJump { .. } => "TryJump",
        BbTerm::Raise { .. } => "Raise",
        BbTerm::Ret(_) => "Ret",
    }
}

fn unsupported_fastpath_block_message(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
    block: &BbBlock,
) -> String {
    let op_kinds = block
        .ops
        .iter()
        .map(bb_op_kind)
        .collect::<Vec<_>>()
        .join(", ");
    let op_debug = block
        .ops
        .iter()
        .map(|op| format!("{op:?}"))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "unsupported JIT block shape in {}:{}: term={}, ops=[{}], params={:?}, exc_target={:?}; op_debug=[{}]; expected direct-simple lowered block",
        function.qualname,
        block.label,
        bb_term_kind(&block.term),
        op_kinds,
        block.params,
        block.exc_target_label,
        op_debug,
    )
}

fn direct_simple_block_plan_from_block(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
    block: &dp_transform::basic_block::bb_ir::BbBlock,
    label_to_index: &HashMap<String, usize>,
) -> Option<DirectSimpleBlockPlan> {
    let mut known_names: Vec<String> = block.params.clone();
    let mut ops = Vec::new();
    for op in &block.ops {
        let stmt_op = direct_simple_op_from_bb_op(op, &mut known_names)?;
        ops.push(stmt_op);
    }
    let term = match &block.term {
        BbTerm::Jump(target_label) => {
            let target_index = *label_to_index.get(target_label.as_str())?;
            let target_params = target_params_from_index(function, target_index)?;
            DirectSimpleTermPlan::Jump {
                target_index,
                target_params,
            }
        }
        BbTerm::BrIf {
            test,
            then_label,
            else_label,
        } => {
            let test_expr = direct_simple_expr_from(test)?;
            let then_index = *label_to_index.get(then_label.as_str())?;
            let then_params = target_params_from_index(function, then_index)?;
            let else_index = *label_to_index.get(else_label.as_str())?;
            let else_params = target_params_from_index(function, else_index)?;
            DirectSimpleTermPlan::BrIf {
                test: test_expr,
                then_index,
                then_params,
                else_index,
                else_params,
            }
        }
        BbTerm::BrTable {
            index,
            targets,
            default_label,
        } => {
            let index_expr = direct_simple_expr_from(index)?;
            let mut target_plans = Vec::with_capacity(targets.len());
            for target_label in targets {
                let target_index = *label_to_index.get(target_label.as_str())?;
                let target_params = target_params_from_index(function, target_index)?;
                target_plans.push((target_index, target_params));
            }
            let default_index = *label_to_index.get(default_label.as_str())?;
            let default_params = target_params_from_index(function, default_index)?;
            DirectSimpleTermPlan::BrTable {
                index: index_expr,
                targets: target_plans,
                default_index,
                default_params,
            }
        }
        BbTerm::Ret(ret_value) => {
            let value = if let Some(expr) = ret_value.as_ref() {
                Some(direct_simple_expr_from(expr)?)
            } else {
                None
            };
            DirectSimpleTermPlan::Ret { value }
        }
        BbTerm::Raise { exc, cause } => {
            let exc = if let Some(expr) = exc.as_ref() {
                Some(direct_simple_expr_from(expr)?)
            } else {
                None
            };
            let cause = if let Some(expr) = cause.as_ref() {
                Some(direct_simple_expr_from(expr)?)
            } else {
                None
            };
            DirectSimpleTermPlan::Raise { exc, cause }
        }
        _ => return None,
    };
    Some(DirectSimpleBlockPlan {
        params: block.params.clone(),
        ops,
        term,
    })
}

fn direct_simple_expr_is_borrowable(expr: &DirectSimpleExprPlan, local_names: &[String]) -> bool {
    match expr {
        DirectSimpleExprPlan::Name(name) => local_names.iter().any(|candidate| candidate == name),
        DirectSimpleExprPlan::Bool(_) | DirectSimpleExprPlan::None => true,
        DirectSimpleExprPlan::Int(_)
        | DirectSimpleExprPlan::Float(_)
        | DirectSimpleExprPlan::Bytes(_)
        | DirectSimpleExprPlan::Tuple(_)
        | DirectSimpleExprPlan::Call { .. } => false,
    }
}

fn direct_simple_call_positional_args<'a>(
    expr: &'a DirectSimpleExprPlan,
) -> Option<(&'a str, Vec<&'a DirectSimpleExprPlan>)> {
    let DirectSimpleExprPlan::Call { func, parts } = expr else {
        return None;
    };
    let DirectSimpleExprPlan::Name(func_name) = func.as_ref() else {
        return None;
    };
    let mut args = Vec::with_capacity(parts.len());
    for part in parts {
        let DirectSimpleCallPart::Pos(value) = part else {
            return None;
        };
        args.push(value);
    }
    Some((func_name.as_str(), args))
}

fn direct_simple_expr_const_string(expr: &DirectSimpleExprPlan) -> Option<String> {
    match expr {
        DirectSimpleExprPlan::Bytes(bytes) => String::from_utf8(bytes.clone()).ok(),
        DirectSimpleExprPlan::Call { .. } => {
            let (func_name, args) = direct_simple_call_positional_args(expr)?;
            if args.len() != 1 {
                return None;
            }
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
    let Some((func_name, args)) = direct_simple_call_positional_args(expr) else {
        return false;
    };
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
    let (func_name, args) = direct_simple_call_positional_args(expr)?;
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

#[derive(Clone, Copy)]
struct DirectSimpleEmitCtx {
    incref_ref: ir::FuncRef,
    decref_ref: ir::FuncRef,
    py_call_ref: ir::FuncRef,
    make_int_ref: ir::FuncRef,
    step_null_block: ir::Block,
    exec_args: ir::Value,
    ptr_ty: ir::Type,
    i64_ty: ir::Type,
    none_const: ir::Value,
    true_const: ir::Value,
    false_const: ir::Value,
    empty_tuple_const: ir::Value,
    block_const: ir::Value,
    load_name_ref: ir::FuncRef,
    load_local_raw_by_name_ref: ir::FuncRef,
    pyobject_getattr_ref: ir::FuncRef,
    pyobject_setattr_ref: ir::FuncRef,
    pyobject_getitem_ref: ir::FuncRef,
    pyobject_setitem_ref: ir::FuncRef,
    pyobject_to_i64_ref: ir::FuncRef,
    decode_literal_bytes_ref: ir::FuncRef,
    make_bytes_ref: ir::FuncRef,
    make_float_ref: ir::FuncRef,
    py_call_object_ref: ir::FuncRef,
    py_call_with_kw_ref: ir::FuncRef,
    tuple_new_ref: ir::FuncRef,
    tuple_set_item_ref: ir::FuncRef,
    compare_eq_obj_ref: ir::FuncRef,
    compare_lt_obj_ref: ir::FuncRef,
}

fn emit_direct_simple_expr(
    fb: &mut FunctionBuilder<'_>,
    expr: &DirectSimpleExprPlan,
    local_names: &[String],
    local_values: &[ir::Value],
    ctx: &DirectSimpleEmitCtx,
    literal_pool: &mut Vec<Box<[u8]>>,
    borrowed: bool,
) -> ir::Value {
    let incref_ref = ctx.incref_ref;
    let decref_ref = ctx.decref_ref;
    let py_call_ref = ctx.py_call_ref;
    let make_int_ref = ctx.make_int_ref;
    let step_null_block = ctx.step_null_block;
    let exec_args = ctx.exec_args;
    let ptr_ty = ctx.ptr_ty;
    let i64_ty = ctx.i64_ty;
    let none_const = ctx.none_const;
    let true_const = ctx.true_const;
    let false_const = ctx.false_const;
    let empty_tuple_const = ctx.empty_tuple_const;
    let block_const = ctx.block_const;
    let load_name_ref = ctx.load_name_ref;
    let pyobject_getattr_ref = ctx.pyobject_getattr_ref;
    let pyobject_setattr_ref = ctx.pyobject_setattr_ref;
    let pyobject_getitem_ref = ctx.pyobject_getitem_ref;
    let pyobject_setitem_ref = ctx.pyobject_setitem_ref;
    let decode_literal_bytes_ref = ctx.decode_literal_bytes_ref;
    let make_bytes_ref = ctx.make_bytes_ref;
    let make_float_ref = ctx.make_float_ref;
    let py_call_object_ref = ctx.py_call_object_ref;
    let py_call_with_kw_ref = ctx.py_call_with_kw_ref;
    let tuple_new_ref = ctx.tuple_new_ref;
    let tuple_set_item_ref = ctx.tuple_set_item_ref;
    let compare_eq_obj_ref = ctx.compare_eq_obj_ref;
    let compare_lt_obj_ref = ctx.compare_lt_obj_ref;

    match expr {
        DirectSimpleExprPlan::Name(name) => {
            if let Some(slot_index) = local_names.iter().position(|candidate| candidate == name) {
                let slot_value = local_values[slot_index];
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
                &[ir::BlockArg::Value(exec_args)],
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
                &[ir::BlockArg::Value(exec_args)],
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
                &[ir::BlockArg::Value(exec_args)],
                float_ok_block,
                &[ir::BlockArg::Value(float_value)],
            );
            fb.switch_to_block(float_ok_block);
            fb.block_params(float_ok_block)[0]
        }
        DirectSimpleExprPlan::Bool(value) => {
            let bool_const = if *value { true_const } else { false_const };
            if !borrowed {
                fb.ins().call(incref_ref, &[bool_const]);
            }
            bool_const
        }
        DirectSimpleExprPlan::None => {
            if !borrowed {
                fb.ins().call(incref_ref, &[none_const]);
            }
            none_const
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
                &[ir::BlockArg::Value(exec_args)],
                value_ok_block,
                &[ir::BlockArg::Value(value)],
            );
            fb.switch_to_block(value_ok_block);
            fb.block_params(value_ok_block)[0]
        }
        DirectSimpleExprPlan::Tuple(items) => {
            assert!(!borrowed, "tuple expression must produce owned references");
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let tuple_len = fb.ins().iconst(i64_ty, items.len() as i64);
            let tuple_inst = fb.ins().call(tuple_new_ref, &[tuple_len]);
            let tuple_obj = fb.inst_results(tuple_inst)[0];
            let tuple_is_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, tuple_obj, null_ptr);
            let tuple_ok_block = fb.create_block();
            fb.append_block_param(tuple_ok_block, ptr_ty);
            fb.ins().brif(
                tuple_is_null,
                step_null_block,
                &[ir::BlockArg::Value(exec_args)],
                tuple_ok_block,
                &[ir::BlockArg::Value(tuple_obj)],
            );
            fb.switch_to_block(tuple_ok_block);
            let tuple_obj = fb.block_params(tuple_ok_block)[0];
            for (index, item) in items.iter().enumerate() {
                let borrowed_item = direct_simple_expr_is_borrowable(item, local_names);
                let value = emit_direct_simple_expr(
                    fb,
                    item,
                    local_names,
                    local_values,
                    ctx,
                    literal_pool,
                    borrowed_item,
                );
                if borrowed_item {
                    fb.ins().call(incref_ref, &[value]);
                }
                let item_index = fb.ins().iconst(i64_ty, index as i64);
                let set_inst = fb
                    .ins()
                    .call(tuple_set_item_ref, &[tuple_obj, item_index, value]);
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
                    &[ir::BlockArg::Value(tuple_obj)],
                    set_ok_block,
                    &[],
                );
                fb.switch_to_block(set_fail_block);
                let failed_tuple = fb.block_params(set_fail_block)[0];
                fb.ins().call(decref_ref, &[failed_tuple]);
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(exec_args)]);
                fb.switch_to_block(set_ok_block);
            }
            tuple_obj
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
                            &[ir::BlockArg::Value(exec_args)],
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
                            &[ir::BlockArg::Value(exec_args)],
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
                if !has_unpack
                    && simple_keywords.is_empty()
                    && simple_args.is_empty()
                    && (func_name == "__dp_locals" || func_name == "__dp_dir_")
                {
                    if let Some(frame_index) = local_names
                        .iter()
                        .position(|candidate| candidate == "_dp_frame")
                    {
                        let frame_obj = local_values[frame_index];
                        let normalize_name_bytes: &[u8] = if func_name == "__dp_locals" {
                            b"__dp_normalize_mapping"
                        } else {
                            b"__dp_dir_from_locals_mapping"
                        };
                        let normalize_name_ptr = fb
                            .ins()
                            .iconst(ptr_ty, normalize_name_bytes.as_ptr() as i64);
                        let normalize_name_len =
                            fb.ins().iconst(i64_ty, normalize_name_bytes.len() as i64);
                        let normalize_callable_inst = fb.ins().call(
                            load_name_ref,
                            &[block_const, normalize_name_ptr, normalize_name_len],
                        );
                        let normalize_callable = fb.inst_results(normalize_callable_inst)[0];
                        let normalize_callable_is_null = fb.ins().icmp(
                            ir::condcodes::IntCC::Equal,
                            normalize_callable,
                            null_ptr,
                        );
                        let normalize_callable_ok = fb.create_block();
                        fb.append_block_param(normalize_callable_ok, ptr_ty);
                        fb.ins().brif(
                            normalize_callable_is_null,
                            step_null_block,
                            &[ir::BlockArg::Value(exec_args)],
                            normalize_callable_ok,
                            &[ir::BlockArg::Value(normalize_callable)],
                        );
                        fb.switch_to_block(normalize_callable_ok);
                        let normalize_callable = fb.block_params(normalize_callable_ok)[0];
                        let normalized_inst = fb.ins().call(
                            py_call_ref,
                            &[normalize_callable, frame_obj, null_ptr, null_ptr, null_ptr],
                        );
                        fb.ins().call(decref_ref, &[normalize_callable]);
                        let normalized_obj = fb.inst_results(normalized_inst)[0];
                        let normalized_is_null =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, normalized_obj, null_ptr);
                        let normalized_ok = fb.create_block();
                        fb.append_block_param(normalized_ok, ptr_ty);
                        fb.ins().brif(
                            normalized_is_null,
                            step_null_block,
                            &[ir::BlockArg::Value(exec_args)],
                            normalized_ok,
                            &[ir::BlockArg::Value(normalized_obj)],
                        );
                        fb.switch_to_block(normalized_ok);
                        return fb.block_params(normalized_ok)[0];
                    }
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
                        &[ir::BlockArg::Value(exec_args)],
                        dict_callable_ok,
                        &[ir::BlockArg::Value(dict_callable)],
                    );
                    fb.switch_to_block(dict_callable_ok);
                    let dict_callable = fb.block_params(dict_callable_ok)[0];
                    let dict_obj_inst = fb
                        .ins()
                        .call(py_call_object_ref, &[dict_callable, empty_tuple_const]);
                    fb.ins().call(decref_ref, &[dict_callable]);
                    let dict_obj = fb.inst_results(dict_obj_inst)[0];
                    let dict_obj_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, dict_obj, null_ptr);
                    let dict_obj_ok = fb.create_block();
                    fb.append_block_param(dict_obj_ok, ptr_ty);
                    fb.ins().brif(
                        dict_obj_is_null,
                        step_null_block,
                        &[ir::BlockArg::Value(exec_args)],
                        dict_obj_ok,
                        &[ir::BlockArg::Value(dict_obj)],
                    );
                    fb.switch_to_block(dict_obj_ok);
                    let dict_obj = fb.block_params(dict_obj_ok)[0];

                    for (idx, local_name) in local_names.iter().enumerate() {
                        if local_name.starts_with("_dp_") && !local_name.starts_with("_dp_cell_") {
                            continue;
                        }
                        let export_name = local_name.clone();
                        let value_obj = local_values[idx];

                        let (name_ptr, name_len) =
                            intern_bytes_literal(literal_pool, export_name.as_bytes());
                        let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
                        let name_len_val = fb.ins().iconst(i64_ty, name_len);
                        let key_inst = fb
                            .ins()
                            .call(decode_literal_bytes_ref, &[name_ptr_val, name_len_val]);
                        let key_obj = fb.inst_results(key_inst)[0];
                        let key_is_null =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, key_obj, null_ptr);
                        let key_ok = fb.create_block();
                        fb.append_block_param(key_ok, ptr_ty);
                        fb.ins().brif(
                            key_is_null,
                            step_null_block,
                            &[ir::BlockArg::Value(exec_args)],
                            key_ok,
                            &[ir::BlockArg::Value(key_obj)],
                        );
                        fb.switch_to_block(key_ok);
                        let key_obj = fb.block_params(key_ok)[0];
                        let set_item_inst = fb
                            .ins()
                            .call(pyobject_setitem_ref, &[dict_obj, key_obj, value_obj]);
                        let set_item_value = fb.inst_results(set_item_inst)[0];
                        fb.ins().call(decref_ref, &[key_obj]);
                        let set_item_failed =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, set_item_value, null_ptr);
                        let set_item_ok = fb.create_block();
                        fb.ins().brif(
                            set_item_failed,
                            step_null_block,
                            &[ir::BlockArg::Value(exec_args)],
                            set_item_ok,
                            &[],
                        );
                        fb.switch_to_block(set_item_ok);
                        fb.ins().call(decref_ref, &[set_item_value]);
                    }
                    let normalize_name_bytes: &[u8] = if func_name == "__dp_locals" {
                        b"__dp_normalize_mapping"
                    } else {
                        b"__dp_dir_from_locals_mapping"
                    };
                    let normalize_name_ptr = fb
                        .ins()
                        .iconst(ptr_ty, normalize_name_bytes.as_ptr() as i64);
                    let normalize_name_len =
                        fb.ins().iconst(i64_ty, normalize_name_bytes.len() as i64);
                    let normalize_callable_inst = fb.ins().call(
                        load_name_ref,
                        &[block_const, normalize_name_ptr, normalize_name_len],
                    );
                    let normalize_callable = fb.inst_results(normalize_callable_inst)[0];
                    let normalize_callable_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, normalize_callable, null_ptr);
                    let normalize_callable_ok = fb.create_block();
                    fb.append_block_param(normalize_callable_ok, ptr_ty);
                    fb.ins().brif(
                        normalize_callable_is_null,
                        step_null_block,
                        &[ir::BlockArg::Value(exec_args)],
                        normalize_callable_ok,
                        &[ir::BlockArg::Value(normalize_callable)],
                    );
                    fb.switch_to_block(normalize_callable_ok);
                    let normalize_callable = fb.block_params(normalize_callable_ok)[0];
                    let normalized_inst = fb.ins().call(
                        py_call_ref,
                        &[normalize_callable, dict_obj, null_ptr, null_ptr, null_ptr],
                    );
                    fb.ins().call(decref_ref, &[normalize_callable]);
                    let normalized_obj = fb.inst_results(normalized_inst)[0];
                    let normalized_is_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, normalized_obj, null_ptr);
                    let normalized_ok = fb.create_block();
                    fb.append_block_param(normalized_ok, ptr_ty);
                    fb.ins().brif(
                        normalized_is_null,
                        step_null_block,
                        &[ir::BlockArg::Value(exec_args)],
                        normalized_ok,
                        &[ir::BlockArg::Value(normalized_obj)],
                    );
                    fb.switch_to_block(normalized_ok);
                    fb.ins().call(decref_ref, &[dict_obj]);
                    return fb.block_params(normalized_ok)[0];
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
                    &[ir::BlockArg::Value(exec_args)],
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
                    &[ir::BlockArg::Value(exec_args)],
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
                        &[ir::BlockArg::Value(exec_args)],
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
                        &[ir::BlockArg::Value(exec_args)],
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
                                &[ir::BlockArg::Value(exec_args)],
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
                                &[ir::BlockArg::Value(exec_args)],
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
                                &[ir::BlockArg::Value(exec_args)],
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
                                &[ir::BlockArg::Value(exec_args)],
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
                            fb.ins()
                                .jump(step_null_block, &[ir::BlockArg::Value(exec_args)]);
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
                                &[ir::BlockArg::Value(exec_args)],
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
                                &[ir::BlockArg::Value(exec_args)],
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
                                &[ir::BlockArg::Value(exec_args)],
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
                    &[ir::BlockArg::Value(exec_args)],
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
                    &[ir::BlockArg::Value(exec_args)],
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
                    &[ir::BlockArg::Value(exec_args)],
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
                            &[ir::BlockArg::Value(exec_args)],
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
                            &[ir::BlockArg::Value(exec_args)],
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
                let intrinsic_ref = match (func_name.as_str(), args.len()) {
                    ("PyObject_GetAttr", 2) => Some(pyobject_getattr_ref),
                    ("PyObject_SetAttr", 3) => Some(pyobject_setattr_ref),
                    ("PyObject_GetItem", 2) => Some(pyobject_getitem_ref),
                    ("PyObject_SetItem", 3) => Some(pyobject_setitem_ref),
                    _ => None,
                };
                let compare_ref = match (func_name.as_str(), args.len()) {
                    ("__dp_eq", 2) => Some(compare_eq_obj_ref),
                    ("__dp_lt", 2) => Some(compare_lt_obj_ref),
                    _ => None,
                };
                if keywords.is_empty() {
                    if let Some(compare_ref) = compare_ref {
                        let mut arg_values: Vec<(ir::Value, bool)> = Vec::with_capacity(args.len());
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
                            );
                            arg_values.push((value, borrowed_arg));
                        }
                        let call_inst = fb
                            .ins()
                            .call(compare_ref, &[arg_values[0].0, arg_values[1].0]);
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
                            &[ir::BlockArg::Value(exec_args)],
                            call_ok_block,
                            &[ir::BlockArg::Value(call_value)],
                        );
                        fb.switch_to_block(call_ok_block);
                        return fb.block_params(call_ok_block)[0];
                    }
                    if let Some(intrinsic_ref) = intrinsic_ref {
                        let mut arg_values: Vec<(ir::Value, bool)> = Vec::with_capacity(args.len());
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
                            );
                            arg_values.push((value, borrowed_arg));
                        }
                        let call_inst = if arg_values.len() == 2 {
                            fb.ins()
                                .call(intrinsic_ref, &[arg_values[0].0, arg_values[1].0])
                        } else {
                            fb.ins().call(
                                intrinsic_ref,
                                &[arg_values[0].0, arg_values[1].0, arg_values[2].0],
                            )
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
                            &[ir::BlockArg::Value(exec_args)],
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
                direct_simple_expr_is_borrowable(func.as_ref(), local_names),
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
                    &[ir::BlockArg::Value(exec_args)],
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
                &[ir::BlockArg::Value(exec_args)],
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
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(exec_args)]);
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
                    &[ir::BlockArg::Value(exec_args)],
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
                    &[ir::BlockArg::Value(exec_args)],
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
                    &[ir::BlockArg::Value(exec_args)],
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
                        &[ir::BlockArg::Value(exec_args)],
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
                    fb.ins()
                        .jump(step_null_block, &[ir::BlockArg::Value(exec_args)]);
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
                &[ir::BlockArg::Value(exec_args)],
                call_ok_block,
                &[ir::BlockArg::Value(call_value)],
            );
            fb.switch_to_block(call_ok_block);
            fb.block_params(call_ok_block)[0]
        }
    }
}

fn emit_pack_target_args(
    fb: &mut FunctionBuilder<'_>,
    target_params: &[String],
    local_names: &[String],
    local_values: &[ir::Value],
    ctx: &DirectSimpleEmitCtx,
    literal_pool: &mut Vec<Box<[u8]>>,
) -> Option<ir::Value> {
    if target_params.is_empty() {
        fb.ins().call(ctx.incref_ref, &[ctx.empty_tuple_const]);
        return Some(ctx.empty_tuple_const);
    }
    let ptr_ty = ctx.ptr_ty;
    let i64_ty = ctx.i64_ty;
    let null_ptr = fb.ins().iconst(ptr_ty, 0);
    let tuple_len = fb.ins().iconst(i64_ty, target_params.len() as i64);
    let tuple_inst = fb.ins().call(ctx.tuple_new_ref, &[tuple_len]);
    let tuple_obj = fb.inst_results(tuple_inst)[0];
    let tuple_is_null = fb
        .ins()
        .icmp(ir::condcodes::IntCC::Equal, tuple_obj, null_ptr);
    let tuple_ok_block = fb.create_block();
    fb.append_block_param(tuple_ok_block, ptr_ty);
    fb.ins().brif(
        tuple_is_null,
        ctx.step_null_block,
        &[ir::BlockArg::Value(ctx.exec_args)],
        tuple_ok_block,
        &[ir::BlockArg::Value(tuple_obj)],
    );
    fb.switch_to_block(tuple_ok_block);
    let tuple_obj = fb.block_params(tuple_ok_block)[0];
    let owner_value = local_names
        .iter()
        .position(|candidate| candidate == "_dp_self" || candidate == "_dp_state")
        .map(|index| local_values[index]);
    for (index, name) in target_params.iter().enumerate() {
        let value =
            if let Some(value_index) = local_names.iter().position(|candidate| candidate == name) {
                local_values[value_index]
            } else if let Some(owner) = owner_value {
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
                    ctx.step_null_block,
                    &[ir::BlockArg::Value(ctx.exec_args)],
                    load_ok_block,
                    &[ir::BlockArg::Value(load_value)],
                );
                fb.switch_to_block(load_ok_block);
                fb.block_params(load_ok_block)[0]
            } else {
                ctx.none_const
            };
        // PyTuple_SetItem steals a reference; pass owned values.
        fb.ins().call(ctx.incref_ref, &[value]);
        let item_index = fb.ins().iconst(i64_ty, index as i64);
        let set_inst = fb
            .ins()
            .call(ctx.tuple_set_item_ref, &[tuple_obj, item_index, value]);
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
            &[ir::BlockArg::Value(tuple_obj)],
            set_ok_block,
            &[],
        );
        fb.switch_to_block(set_fail_block);
        let failed_tuple = fb.block_params(set_fail_block)[0];
        fb.ins().call(ctx.decref_ref, &[failed_tuple]);
        fb.ins()
            .jump(ctx.step_null_block, &[ir::BlockArg::Value(ctx.exec_args)]);
        fb.switch_to_block(set_ok_block);
    }
    Some(tuple_obj)
}

fn emit_truthy_from_owned(
    fb: &mut FunctionBuilder<'_>,
    owned_value: ir::Value,
    is_true_ref: ir::FuncRef,
    decref_ref: ir::FuncRef,
    step_null_block: ir::Block,
    exec_args: ir::Value,
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
        &[ir::BlockArg::Value(exec_args)],
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
    emit_ctx: &DirectSimpleEmitCtx,
    literal_pool: &mut Vec<Box<[u8]>>,
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
                );
                if let Some(existing_index) = local_names
                    .iter()
                    .position(|candidate| candidate == &assign.target)
                {
                    let previous = local_values[existing_index];
                    fb.ins().call(emit_ctx.decref_ref, &[previous]);
                    local_values[existing_index] = value;
                } else {
                    local_names.push(assign.target.clone());
                    local_values.push(value);
                }
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
                    let null_ptr = fb.ins().iconst(emit_ctx.ptr_ty, 0);
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
                    );
                    let key_value = emit_direct_simple_expr(
                        fb,
                        key_expr,
                        local_names,
                        local_values,
                        emit_ctx,
                        literal_pool,
                        key_borrowed,
                    );
                    let value_value = emit_direct_simple_expr(
                        fb,
                        value_expr,
                        local_names,
                        local_values,
                        emit_ctx,
                        literal_pool,
                        value_borrowed,
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
                    fb.append_block_param(set_item_ok, emit_ctx.ptr_ty);
                    fb.ins().brif(
                        set_item_failed,
                        emit_ctx.step_null_block,
                        &[ir::BlockArg::Value(emit_ctx.exec_args)],
                        set_item_ok,
                        &[ir::BlockArg::Value(set_item_value)],
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
                    fb.append_block_param(synced_ok, emit_ctx.ptr_ty);
                    fb.ins().brif(
                        synced_failed,
                        emit_ctx.step_null_block,
                        &[ir::BlockArg::Value(emit_ctx.exec_args)],
                        synced_ok,
                        &[ir::BlockArg::Value(synced_value)],
                    );
                    fb.switch_to_block(synced_ok);
                    let synced_value = fb.block_params(synced_ok)[0];
                    if let Some(existing_index) = local_names
                        .iter()
                        .position(|candidate| candidate == &key_name)
                    {
                        let previous = local_values[existing_index];
                        fb.ins().call(emit_ctx.decref_ref, &[previous]);
                        local_values[existing_index] = synced_value;
                    } else {
                        local_names.push(key_name);
                        local_values.push(synced_value);
                    }
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
                );
                fb.ins().call(emit_ctx.decref_ref, &[value]);
            }
            DirectSimpleOpPlan::Delete(delete_plan) => {
                for target in &delete_plan.targets {
                    let DirectSimpleDeleteTargetPlan::LocalName(name) = target;
                    let Some(index) = local_names.iter().position(|candidate| candidate == name)
                    else {
                        return Err(format!("missing local binding for delete target: {name}"));
                    };
                    let previous = local_values.remove(index);
                    local_names.remove(index);
                    frame_locals_aliases.remove(name.as_str());
                    fb.ins().call(emit_ctx.decref_ref, &[previous]);
                }
            }
        }
    }
    Ok(())
}

fn build_entry_plan(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
) -> Result<EntryBlockPlan, String> {
    if !matches!(
        function.kind,
        dp_transform::basic_block::bb_ir::BbFunctionKind::Function
            | dp_transform::basic_block::bb_ir::BbFunctionKind::Generator { .. }
            | dp_transform::basic_block::bb_ir::BbFunctionKind::AsyncGenerator { .. }
    ) {
        return Err(format!(
            "unsupported JIT function kind in {}: {:?}; only plain/generator/async-generator functions are currently supported",
            function.qualname, function.kind
        ));
    }
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
    let mut block_terms = Vec::with_capacity(function.blocks.len());
    let mut block_exc_targets = Vec::with_capacity(function.blocks.len());
    let mut block_exc_dispatches = Vec::with_capacity(function.blocks.len());
    let mut block_param_names = Vec::with_capacity(function.blocks.len());
    let mut block_fast_paths = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        let exc_target = match block.exc_target_label.as_ref() {
            Some(label) => Some(label_to_index.get(label.as_str()).copied().ok_or_else(|| {
                format!(
                    "unknown exception target {label} in {}:{}",
                    function.qualname, block.label
                )
            })?),
            None => None,
        };
        let exc_dispatch = if let Some(target_index) = exc_target {
            let target_block = &function.blocks[target_index];
            let owner_param_index = block
                .params
                .iter()
                .position(|name| name == "_dp_self")
                .or_else(|| block.params.iter().position(|name| name == "_dp_state"));
            let mut arg_sources = Vec::with_capacity(target_block.params.len());
            for target_param in &target_block.params {
                if block.exc_name.as_deref() == Some(target_param.as_str()) {
                    arg_sources.push(BlockExcArgSource::Exception);
                    continue;
                }
                if target_param == "_dp_resume_exc" {
                    arg_sources.push(BlockExcArgSource::NoneValue);
                    continue;
                }
                if let Some(source_index) = block
                    .params
                    .iter()
                    .position(|source_name| source_name == target_param)
                {
                    arg_sources.push(BlockExcArgSource::SourceParam {
                        index: source_index,
                    });
                    continue;
                }
                if target_param.starts_with("_dp_try_exc_")
                    || target_param.starts_with("_dp_try_reason_")
                    || target_param.starts_with("_dp_try_value_")
                {
                    // Exception-edge temporary slots are control-flow bookkeeping;
                    // when they are not in source params for this edge, seed with
                    // a neutral value instead of requiring frame-local fallback.
                    arg_sources.push(BlockExcArgSource::NoneValue);
                    continue;
                }
                if owner_param_index.is_none() {
                    // Plain function blocks do not carry a frame owner parameter.
                    // For missing edge locals, seed a neutral value on exception
                    // edges instead of requiring frame-local recovery.
                    arg_sources.push(BlockExcArgSource::NoneValue);
                } else {
                    arg_sources.push(BlockExcArgSource::FrameLocal {
                        name: target_param.clone(),
                    });
                }
            }
            if owner_param_index.is_none()
                && arg_sources
                    .iter()
                    .any(|src| matches!(src, BlockExcArgSource::FrameLocal { .. }))
            {
                return Err(format!(
                    "exception dispatch from {}:{} requires frame-local fallback but has no _dp_self/_dp_state parameter",
                    function.qualname, block.label
                ));
            }
            Some(BlockExcDispatchPlan {
                target_index,
                owner_param_index,
                arg_sources,
            })
        } else {
            None
        };
        let term = match &block.term {
            BbTerm::Jump(target) => {
                let target_index =
                    label_to_index
                        .get(target.as_str())
                        .copied()
                        .ok_or_else(|| {
                            format!(
                                "unknown jump target {target} in {}:{}",
                                function.qualname, block.label
                            )
                        })?;
                BlockTermPlan::Jump { target_index }
            }
            BbTerm::BrIf {
                then_label,
                else_label,
                ..
            } => {
                let then_index = label_to_index
                    .get(then_label.as_str())
                    .copied()
                    .ok_or_else(|| {
                        format!(
                            "unknown then target {then_label} in {}:{}",
                            function.qualname, block.label
                        )
                    })?;
                let else_index = label_to_index
                    .get(else_label.as_str())
                    .copied()
                    .ok_or_else(|| {
                        format!(
                            "unknown else target {else_label} in {}:{}",
                            function.qualname, block.label
                        )
                    })?;
                BlockTermPlan::BrIf {
                    then_index,
                    else_index,
                }
            }
            BbTerm::BrTable {
                targets,
                default_label,
                ..
            } => {
                let default_index = label_to_index
                    .get(default_label.as_str())
                    .copied()
                    .ok_or_else(|| {
                        format!(
                            "unknown br_table default target {default_label} in {}:{}",
                            function.qualname, block.label
                        )
                    })?;
                let mut target_indices = Vec::with_capacity(targets.len());
                for target in targets {
                    let target_index =
                        label_to_index
                            .get(target.as_str())
                            .copied()
                            .ok_or_else(|| {
                                format!(
                                    "unknown br_table target {target} in {}:{}",
                                    function.qualname, block.label
                                )
                            })?;
                    target_indices.push(target_index);
                }
                BlockTermPlan::BrTable {
                    targets: target_indices,
                    default_index,
                }
            }
            BbTerm::Raise { .. } => BlockTermPlan::Raise,
            BbTerm::Ret(_) => BlockTermPlan::Ret,
            BbTerm::TryJump { .. } => {
                return Err(format!(
                    "unsupported try_jump in JIT entry plan for {}:{}",
                    function.qualname, block.label
                ));
            }
        };
        let fast_path = {
            if block.ops.is_empty() {
                match &block.term {
                    BbTerm::Jump(target_label) => {
                        let target_index = label_to_index
                            .get(target_label.as_str())
                            .copied()
                            .ok_or_else(|| {
                                format!(
                                    "unknown jump target {target_label} in {}:{}",
                                    function.qualname, block.label
                                )
                            })?;
                        let source_params = block.params.as_slice();
                        let target_params = function.blocks[target_index].params.as_slice();
                        if source_params == target_params {
                            BlockFastPath::JumpPassThrough { target_index }
                        } else if let Some(plan) = direct_simple_plan_from_block(block) {
                            BlockFastPath::DirectSimpleRet { plan }
                        } else if let Some(plan) =
                            direct_simple_block_plan_from_block(function, block, &label_to_index)
                        {
                            BlockFastPath::DirectSimpleBlock { plan }
                        } else {
                            BlockFastPath::None
                        }
                    }
                    BbTerm::Ret(None) => BlockFastPath::ReturnNone,
                    BbTerm::BrIf { .. } => {
                        if let Some(plan) =
                            direct_simple_brif_plan_from_block(function, block, &label_to_index)
                        {
                            BlockFastPath::DirectSimpleBrIf { plan }
                        } else if let Some(plan) =
                            direct_simple_block_plan_from_block(function, block, &label_to_index)
                        {
                            BlockFastPath::DirectSimpleBlock { plan }
                        } else {
                            BlockFastPath::None
                        }
                    }
                    _ => {
                        if let Some(plan) = direct_simple_plan_from_block(block) {
                            BlockFastPath::DirectSimpleRet { plan }
                        } else if let Some(plan) =
                            direct_simple_block_plan_from_block(function, block, &label_to_index)
                        {
                            BlockFastPath::DirectSimpleBlock { plan }
                        } else {
                            BlockFastPath::None
                        }
                    }
                }
            } else if let Some(plan) = direct_simple_plan_from_block(block) {
                BlockFastPath::DirectSimpleRet { plan }
            } else if let Some(plan) = direct_simple_expr_ret_none_plan_from_block(block) {
                BlockFastPath::DirectSimpleExprRetNone { plan }
            } else if let Some(plan) =
                direct_simple_block_plan_from_block(function, block, &label_to_index)
            {
                BlockFastPath::DirectSimpleBlock { plan }
            } else {
                BlockFastPath::None
            }
        };
        if matches!(fast_path, BlockFastPath::None) {
            return Err(unsupported_fastpath_block_message(function, block));
        }
        block_terms.push(term);
        block_exc_targets.push(exc_target);
        block_exc_dispatches.push(exc_dispatch);
        block_param_names.push(block.params.clone());
        block_fast_paths.push(fast_path);
    }
    Ok(EntryBlockPlan {
        entry_index,
        block_labels: function
            .blocks
            .iter()
            .map(|block| block.label.clone())
            .collect(),
        block_param_names,
        block_terms,
        block_exc_targets,
        block_exc_dispatches,
        block_fast_paths,
    })
}

pub fn register_bb_module_plans(module_name: &str, module: &BbModule) -> Result<(), String> {
    let lowered = lower_try_jump_exception_flow(module)?;
    let debug_skips = std::env::var_os("DIET_PYTHON_DEBUG_JIT_PLAN_SKIPS").is_some();
    let mut plans = HashMap::new();
    let mut skipped_errors: HashMap<String, String> = HashMap::new();
    for function in &lowered.functions {
        let plan_qualname = format!("{}::{}", function.qualname, function.entry);
        match build_entry_plan(function) {
            Ok(plan) => {
                plans.insert(
                    PlanKey {
                        module: module_name.to_string(),
                        qualname: plan_qualname.clone(),
                    },
                    plan,
                );
            }
            Err(err) => {
                if debug_skips {
                    eprintln!(
                        "[diet-python:jitskip] module={} qualname={} entry={} reason={}",
                        module_name, function.qualname, function.entry, err
                    );
                }
                skipped_errors.insert(plan_qualname, err);
            }
        }
    }

    if !skipped_errors.is_empty() {
        let mut skipped = skipped_errors.into_iter().collect::<Vec<_>>();
        skipped.sort_by(|(left, _), (right, _)| left.cmp(right));
        let mut details = String::new();
        for (idx, (qualname, reason)) in skipped.iter().enumerate() {
            if idx > 0 {
                details.push_str("; ");
            }
            details.push_str(qualname.as_str());
            details.push_str(": ");
            details.push_str(reason.as_str());
        }
        return Err(format!(
            "module {module_name} has unsupported JIT plans ({count}): {details}",
            count = skipped.len()
        ));
    }

    let mut registry = bb_plan_registry()
        .lock()
        .map_err(|_| "failed to lock bb plan registry".to_string())?;
    registry.retain(|key, _| key.module != module_name);
    registry.extend(plans);
    Ok(())
}

pub fn lower_try_jump_exception_flow(module: &BbModule) -> Result<BbModule, String> {
    exception_pass::lower_try_jump_exception_flow(module)
}

pub fn lookup_bb_plan(module_name: &str, qualname: &str) -> Option<EntryBlockPlan> {
    let registry = bb_plan_registry().lock().ok()?;
    registry
        .get(&PlanKey {
            module: module_name.to_string(),
            qualname: qualname.to_string(),
        })
        .cloned()
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

unsafe extern "C" fn dp_jit_raise_from_exc(exc: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_RAISE_FROM_EXC_FN {
        return func(exc);
    }
    -1
}

unsafe extern "C" fn dp_jit_py_call_three(
    callable: ObjPtr,
    arg1: ObjPtr,
    arg2: ObjPtr,
    arg3: ObjPtr,
    _sentinel: ObjPtr,
) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_VAR_ARGS_FN {
        return func(callable, arg1, arg2, arg3);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_py_call_object(callable: ObjPtr, args: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_OBJECT_FN {
        return func(callable, args);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_py_call_with_kw(
    callable: ObjPtr,
    args: ObjPtr,
    kwargs: ObjPtr,
) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_WITH_KW_FN {
        return func(callable, args, kwargs);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_get_raised_exception() -> ObjPtr {
    if let Some(func) = DP_JIT_GET_RAISED_EXCEPTION_FN {
        return func();
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_get_arg_item(args: ObjPtr, index: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_GET_ARG_ITEM_FN {
        return func(args, index);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_make_int(value: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_MAKE_INT_FN {
        return func(value);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_make_float(value: f64) -> ObjPtr {
    if let Some(func) = DP_JIT_MAKE_FLOAT_FN {
        return func(value);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_make_bytes(data_ptr: *const u8, data_len: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_MAKE_BYTES_FN {
        return func(data_ptr, data_len);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_load_name(block: ObjPtr, name_ptr: *const u8, name_len: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_LOAD_NAME_FN {
        return func(block, name_ptr, name_len);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_load_local_raw_by_name(
    owner: ObjPtr,
    name_ptr: *const u8,
    name_len: i64,
) -> ObjPtr {
    if let Some(func) = DP_JIT_LOAD_LOCAL_RAW_BY_NAME_FN {
        return func(owner, name_ptr, name_len);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_pyobject_getattr(obj: ObjPtr, attr: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_PYOBJECT_GETATTR_FN {
        return func(obj, attr);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_pyobject_setattr(obj: ObjPtr, attr: ObjPtr, value: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_PYOBJECT_SETATTR_FN {
        return func(obj, attr, value);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_pyobject_getitem(obj: ObjPtr, key: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_PYOBJECT_GETITEM_FN {
        return func(obj, key);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_pyobject_setitem(obj: ObjPtr, key: ObjPtr, value: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_PYOBJECT_SETITEM_FN {
        return func(obj, key, value);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_pyobject_to_i64(value: ObjPtr) -> i64 {
    if let Some(func) = DP_JIT_PYOBJECT_TO_I64_FN {
        return func(value);
    }
    i64::MIN
}

unsafe extern "C" fn dp_jit_decode_literal_bytes(data_ptr: *const u8, data_len: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_DECODE_LITERAL_BYTES_FN {
        return func(data_ptr, data_len);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_tuple_new(size: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_TUPLE_NEW_FN {
        return func(size);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_tuple_set_item(tuple_obj: ObjPtr, index: i64, item: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_TUPLE_SET_ITEM_FN {
        return func(tuple_obj, index, item);
    }
    -1
}

unsafe extern "C" fn dp_jit_is_true(value: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_IS_TRUE_FN {
        return func(value);
    }
    -1
}

unsafe extern "C" fn dp_jit_compare_eq_obj(lhs: ObjPtr, rhs: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_COMPARE_EQ_OBJ_FN {
        return func(lhs, rhs);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_compare_lt_obj(lhs: ObjPtr, rhs: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_COMPARE_LT_OBJ_FN {
        return func(lhs, rhs);
    }
    ptr::null_mut()
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

pub fn run_cranelift_smoke(module: &BbModule) -> Result<(), String> {
    let function_count = module.functions.len() as i64;
    let block_count = module
        .functions
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

    let incref_id = declare_import_fn(&mut jit_module, "dp_jit_incref", &incref_sig)?;
    let decref_id = declare_import_fn(&mut jit_module, "dp_jit_decref", &decref_sig)?;
    let call_id = declare_import_fn(&mut jit_module, "dp_jit_call_one_arg", &call_sig)?;
    let main_id = declare_local_fn(&mut jit_module, "dp_jit_call_smoke", &main_sig)?;

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

    define_function_with_incremental_cache(
        &mut jit_module,
        main_id,
        &mut ctx,
        "failed to define jit call smoke function",
    )?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize jit call smoke function: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(main_id);
    let compiled: extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr = std::mem::transmute(code_ptr);
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

    let incref_id = declare_import_fn(&mut jit_module, "dp_jit_incref", &incref_sig)?;
    let decref_id = declare_import_fn(&mut jit_module, "dp_jit_decref", &decref_sig)?;
    let call_id = declare_import_fn(&mut jit_module, "dp_jit_call_two_args", &call_sig)?;
    let main_id = declare_local_fn(&mut jit_module, "dp_jit_call2", &main_sig)?;

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

    define_function_with_incremental_cache(
        &mut jit_module,
        main_id,
        &mut ctx,
        "failed to define jit two-arg call function",
    )?;
    jit_module.clear_context(&mut ctx);
    jit_module
        .finalize_definitions()
        .map_err(|err| format!("failed to finalize jit two-arg call function: {err}"))?;

    let code_ptr = jit_module.get_finalized_function(main_id);
    let compiled: extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr = std::mem::transmute(code_ptr);
    let result = compiled(callable, arg1, arg2);
    Ok(result)
}

fn build_cranelift_run_bb_specialized_function(
    jit_module: &mut JITModule,
    blocks: &[ObjPtr],
    plan: &EntryBlockPlan,
    globals_obj: ObjPtr,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
    none_obj: ObjPtr,
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
    let block_count = plan.block_labels.len();
    if block_count != plan.block_param_names.len()
        || block_count != plan.block_terms.len()
        || block_count != plan.block_exc_targets.len()
        || block_count != plan.block_exc_dispatches.len()
        || block_count != plan.block_fast_paths.len()
    {
        return Err(format!(
            "specialized JIT plan size mismatch: labels={}, params={}, terms={}, exc_targets={}, exc_dispatches={}, fast_paths={}",
            plan.block_labels.len(),
            plan.block_param_names.len(),
            plan.block_terms.len(),
            plan.block_exc_targets.len(),
            plan.block_exc_dispatches.len(),
            plan.block_fast_paths.len(),
        ));
    }
    if plan.entry_index >= block_count {
        return Err(format!(
            "specialized JIT run_bb entry index out of range: {} >= {}",
            plan.entry_index, block_count
        ));
    }
    let has_generic_blocks = plan
        .block_fast_paths
        .iter()
        .any(|path| matches!(path, BlockFastPath::None));
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

    let mut compare_obj_sig = jit_module.make_signature();
    compare_obj_sig.params.push(ir::AbiParam::new(ptr_ty));
    compare_obj_sig.params.push(ir::AbiParam::new(ptr_ty));
    compare_obj_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut raise_exc_sig = jit_module.make_signature();
    raise_exc_sig.params.push(ir::AbiParam::new(ptr_ty));
    raise_exc_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut main_sig = jit_module.make_signature();
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
    let tuple_new_id = declare_import_fn(jit_module, "dp_jit_tuple_new", &tuple_new_sig)?;
    let tuple_set_item_id =
        declare_import_fn(jit_module, "dp_jit_tuple_set_item", &tuple_set_item_sig)?;
    let is_true_id = declare_import_fn(jit_module, "dp_jit_is_true", &is_true_sig)?;
    let compare_eq_obj_id =
        declare_import_fn(jit_module, "dp_jit_compare_eq_obj", &compare_obj_sig)?;
    let compare_lt_obj_id =
        declare_import_fn(jit_module, "dp_jit_compare_lt_obj", &compare_obj_sig)?;
    let raise_exc_id = declare_import_fn(jit_module, "dp_jit_raise_from_exc", &raise_exc_sig)?;
    let main_id = declare_local_fn(jit_module, "dp_jit_run_bb_specialized", &main_sig)?;
    let mut import_id_to_symbol: HashMap<u32, &'static str> = HashMap::new();
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
    import_id_to_symbol.insert(tuple_new_id.as_u32(), "dp_jit_tuple_new");
    import_id_to_symbol.insert(tuple_set_item_id.as_u32(), "dp_jit_tuple_set_item");
    import_id_to_symbol.insert(is_true_id.as_u32(), "dp_jit_is_true");
    import_id_to_symbol.insert(compare_eq_obj_id.as_u32(), "dp_jit_compare_eq_obj");
    import_id_to_symbol.insert(compare_lt_obj_id.as_u32(), "dp_jit_compare_lt_obj");
    import_id_to_symbol.insert(raise_exc_id.as_u32(), "dp_jit_raise_from_exc");

    let mut ctx = jit_module.make_context();
    let mut literal_pool: Vec<Box<[u8]>> = Vec::new();
    ctx.func.signature = main_sig;
    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry_block = fb.create_block();
        let mut exec_blocks = Vec::with_capacity(block_count);
        for _ in 0..block_count {
            exec_blocks.push(fb.create_block());
        }
        let step_null_block = fb.create_block();
        let raise_exc_direct_block = fb.create_block();

        fb.append_block_params_for_function_params(entry_block);
        for block in &exec_blocks {
            fb.append_block_param(*block, ptr_ty); // args
        }
        fb.append_block_param(step_null_block, ptr_ty); // args
        fb.append_block_param(raise_exc_direct_block, ptr_ty); // args
        fb.append_block_param(raise_exc_direct_block, ptr_ty); // exc

        fb.switch_to_block(entry_block);
        let entry_args = fb.block_params(entry_block)[0];
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
        let make_bytes_ref = jit_module.declare_func_in_func(make_bytes_id, &mut fb.func);
        let tuple_new_ref = jit_module.declare_func_in_func(tuple_new_id, &mut fb.func);
        let tuple_set_item_ref = jit_module.declare_func_in_func(tuple_set_item_id, &mut fb.func);
        let compare_eq_obj_ref = jit_module.declare_func_in_func(compare_eq_obj_id, &mut fb.func);
        let compare_lt_obj_ref = jit_module.declare_func_in_func(compare_lt_obj_id, &mut fb.func);

        fb.ins().call(incref_ref, &[entry_args]);
        let entry_jump_args = [ir::BlockArg::Value(entry_args)];
        fb.ins()
            .jump(exec_blocks[plan.entry_index], &entry_jump_args);

        let mut exception_dispatch_blocks: Vec<Option<ir::Block>> = vec![None; exec_blocks.len()];
        for (index, exc_dispatch_plan) in plan.block_exc_dispatches.iter().enumerate() {
            if exc_dispatch_plan.is_some() {
                let dispatch_block = fb.create_block();
                fb.append_block_param(dispatch_block, ptr_ty); // args
                exception_dispatch_blocks[index] = Some(dispatch_block);
            }
        }

        for (index, block) in exec_blocks.iter().enumerate() {
            fb.switch_to_block(*block);
            let exec_args = fb.block_params(*block)[0];
            let block_const = fb.ins().iconst(ptr_ty, globals_obj as i64);
            let none_const = fb.ins().iconst(ptr_ty, none_obj as i64);
            let true_const = fb.ins().iconst(ptr_ty, true_obj as i64);
            let false_const = fb.ins().iconst(ptr_ty, false_obj as i64);
            let empty_tuple_const = fb.ins().iconst(ptr_ty, empty_tuple_obj as i64);
            let fast_step_null_block = exception_dispatch_blocks[index].unwrap_or(step_null_block);
            let emit_ctx = DirectSimpleEmitCtx {
                incref_ref,
                decref_ref,
                py_call_ref,
                make_int_ref,
                step_null_block: fast_step_null_block,
                exec_args,
                ptr_ty,
                i64_ty,
                none_const,
                true_const,
                false_const,
                empty_tuple_const,
                block_const,
                load_name_ref,
                load_local_raw_by_name_ref,
                pyobject_getattr_ref,
                pyobject_setattr_ref,
                pyobject_getitem_ref,
                pyobject_setitem_ref,
                pyobject_to_i64_ref,
                decode_literal_bytes_ref,
                make_bytes_ref,
                make_float_ref,
                py_call_object_ref,
                py_call_with_kw_ref,
                tuple_new_ref,
                tuple_set_item_ref,
                compare_eq_obj_ref,
                compare_lt_obj_ref,
            };
            match &plan.block_fast_paths[index] {
                BlockFastPath::JumpPassThrough { target_index } => {
                    let jump_args = [ir::BlockArg::Value(exec_args)];
                    fb.ins().jump(exec_blocks[*target_index], &jump_args);
                    continue;
                }
                BlockFastPath::ReturnNone => {
                    fb.ins().call(incref_ref, &[none_const]);
                    fb.ins().call(decref_ref, &[exec_args]);
                    fb.ins().return_(&[none_const]);
                    continue;
                }
                BlockFastPath::DirectSimpleExprRetNone { plan } => {
                    let null_ptr = fb.ins().iconst(ptr_ty, 0);
                    let local_names = plan.params.clone();
                    let mut local_values = Vec::with_capacity(local_names.len());

                    for (param_index, _) in local_names.iter().enumerate() {
                        let index_val = fb.ins().iconst(i64_ty, param_index as i64);
                        let item_inst = fb.ins().call(get_arg_item_ref, &[exec_args, index_val]);
                        let item_val = fb.inst_results(item_inst)[0];
                        let is_null =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, item_val, null_ptr);
                        let ok_block = fb.create_block();
                        fb.append_block_param(ok_block, ptr_ty);
                        fb.ins().brif(
                            is_null,
                            step_null_block,
                            &[ir::BlockArg::Value(exec_args)],
                            ok_block,
                            &[ir::BlockArg::Value(item_val)],
                        );
                        fb.switch_to_block(ok_block);
                        local_values.push(fb.block_params(ok_block)[0]);
                    }

                    for expr in &plan.exprs {
                        let value = emit_direct_simple_expr(
                            &mut fb,
                            expr,
                            &local_names,
                            &local_values,
                            &emit_ctx,
                            &mut literal_pool,
                            false,
                        );
                        fb.ins().call(decref_ref, &[value]);
                    }
                    for value in local_values {
                        fb.ins().call(decref_ref, &[value]);
                    }
                    fb.ins().call(incref_ref, &[none_const]);
                    fb.ins().call(decref_ref, &[exec_args]);
                    fb.ins().return_(&[none_const]);
                    continue;
                }
                BlockFastPath::DirectSimpleBrIf { plan } => {
                    let null_ptr = fb.ins().iconst(ptr_ty, 0);
                    let local_names = plan.params.clone();
                    let mut local_values = Vec::with_capacity(local_names.len());

                    for (param_index, _) in local_names.iter().enumerate() {
                        let index_val = fb.ins().iconst(i64_ty, param_index as i64);
                        let item_inst = fb.ins().call(get_arg_item_ref, &[exec_args, index_val]);
                        let item_val = fb.inst_results(item_inst)[0];
                        let is_null =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, item_val, null_ptr);
                        let ok_block = fb.create_block();
                        fb.append_block_param(ok_block, ptr_ty);
                        fb.ins().brif(
                            is_null,
                            step_null_block,
                            &[ir::BlockArg::Value(exec_args)],
                            ok_block,
                            &[ir::BlockArg::Value(item_val)],
                        );
                        fb.switch_to_block(ok_block);
                        local_values.push(fb.block_params(ok_block)[0]);
                    }

                    let test_value = emit_direct_simple_expr(
                        &mut fb,
                        &plan.test,
                        &local_names,
                        &local_values,
                        &emit_ctx,
                        &mut literal_pool,
                        false,
                    );
                    let truth_inst = fb.ins().call(is_true_ref, &[test_value]);
                    let truth_value = fb.inst_results(truth_inst)[0];
                    fb.ins().call(decref_ref, &[test_value]);
                    for value in local_values {
                        fb.ins().call(decref_ref, &[value]);
                    }
                    let truth_error = fb.ins().iconst(i32_ty, -1);
                    let is_error =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, truth_value, truth_error);
                    let truth_ok_block = fb.create_block();
                    fb.append_block_param(truth_ok_block, i32_ty);
                    fb.ins().brif(
                        is_error,
                        step_null_block,
                        &[ir::BlockArg::Value(exec_args)],
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
                    let pass_args = [ir::BlockArg::Value(exec_args)];
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
                    let null_ptr = fb.ins().iconst(ptr_ty, 0);
                    let mut local_names = plan.params.clone();
                    let mut local_values =
                        Vec::with_capacity(local_names.len() + plan.assigns.len());
                    let mut frame_locals_aliases: HashSet<String> = HashSet::new();

                    for (param_index, _) in local_names.iter().enumerate() {
                        let index_val = fb.ins().iconst(i64_ty, param_index as i64);
                        let item_inst = fb.ins().call(get_arg_item_ref, &[exec_args, index_val]);
                        let item_val = fb.inst_results(item_inst)[0];
                        let is_null =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, item_val, null_ptr);
                        let ok_block = fb.create_block();
                        fb.append_block_param(ok_block, ptr_ty);
                        fb.ins().brif(
                            is_null,
                            step_null_block,
                            &[ir::BlockArg::Value(exec_args)],
                            ok_block,
                            &[ir::BlockArg::Value(item_val)],
                        );
                        fb.switch_to_block(ok_block);
                        let ok_value = fb.block_params(ok_block)[0];
                        local_values.push(ok_value);
                    }

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
                            );
                            let key_value = emit_direct_simple_expr(
                                &mut fb,
                                key_expr,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                key_borrowed,
                            );
                            let value_value = emit_direct_simple_expr(
                                &mut fb,
                                value_expr,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                value_borrowed,
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
                            fb.append_block_param(set_item_ok, ptr_ty);
                            fb.ins().brif(
                                set_item_failed,
                                step_null_block,
                                &[ir::BlockArg::Value(exec_args)],
                                set_item_ok,
                                &[ir::BlockArg::Value(set_item_value)],
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
                            fb.append_block_param(synced_ok, ptr_ty);
                            fb.ins().brif(
                                synced_failed,
                                step_null_block,
                                &[ir::BlockArg::Value(exec_args)],
                                synced_ok,
                                &[ir::BlockArg::Value(synced_value)],
                            );
                            fb.switch_to_block(synced_ok);
                            let synced_value = fb.block_params(synced_ok)[0];
                            if let Some(existing_index) = local_names
                                .iter()
                                .position(|candidate| candidate == &key_name)
                            {
                                let previous = local_values[existing_index];
                                fb.ins().call(decref_ref, &[previous]);
                                local_values[existing_index] = synced_value;
                            } else {
                                local_names.push(key_name);
                                local_values.push(synced_value);
                            }
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
                            )
                        };

                        if let Some(existing_index) = local_names
                            .iter()
                            .position(|candidate| candidate == &assign.target)
                        {
                            let previous = local_values[existing_index];
                            fb.ins().call(decref_ref, &[previous]);
                            local_values[existing_index] = value;
                        } else {
                            local_names.push(assign.target.clone());
                            local_values.push(value);
                        }
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
                    );

                    for value in local_values {
                        fb.ins().call(decref_ref, &[value]);
                    }
                    fb.ins().call(decref_ref, &[exec_args]);
                    fb.ins().return_(&[ret_value]);
                    continue;
                }
                BlockFastPath::DirectSimpleBlock { plan: block_plan } => {
                    let null_ptr = fb.ins().iconst(ptr_ty, 0);
                    let mut local_names = block_plan.params.clone();
                    let mut local_values =
                        Vec::with_capacity(local_names.len() + block_plan.ops.len());
                    for (param_index, _) in local_names.iter().enumerate() {
                        let index_val = fb.ins().iconst(i64_ty, param_index as i64);
                        let item_inst = fb.ins().call(get_arg_item_ref, &[exec_args, index_val]);
                        let item_val = fb.inst_results(item_inst)[0];
                        let is_null =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, item_val, null_ptr);
                        let ok_block = fb.create_block();
                        fb.append_block_param(ok_block, ptr_ty);
                        fb.ins().brif(
                            is_null,
                            step_null_block,
                            &[ir::BlockArg::Value(exec_args)],
                            ok_block,
                            &[ir::BlockArg::Value(item_val)],
                        );
                        fb.switch_to_block(ok_block);
                        local_values.push(fb.block_params(ok_block)[0]);
                    }

                    emit_direct_simple_ops(
                        &mut fb,
                        &block_plan.ops,
                        &mut local_names,
                        &mut local_values,
                        &emit_ctx,
                        &mut literal_pool,
                    )?;

                    match &block_plan.term {
                        DirectSimpleTermPlan::Jump {
                            target_index,
                            target_params,
                        } => {
                            let next_args = emit_pack_target_args(
                                &mut fb,
                                target_params,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                            )
                            .ok_or_else(|| {
                                format!(
                                    "missing local mapping for jump args in block {}",
                                    plan.block_labels[index]
                                )
                            })?;
                            for value in &local_values {
                                fb.ins().call(decref_ref, &[*value]);
                            }
                            fb.ins().call(decref_ref, &[exec_args]);
                            fb.ins().jump(
                                exec_blocks[*target_index],
                                &[ir::BlockArg::Value(next_args)],
                            );
                        }
                        DirectSimpleTermPlan::BrIf {
                            test,
                            then_index,
                            then_params,
                            else_index,
                            else_params,
                        } => {
                            let test_value = emit_direct_simple_expr(
                                &mut fb,
                                test,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                false,
                            );
                            let is_true = emit_truthy_from_owned(
                                &mut fb,
                                test_value,
                                is_true_ref,
                                decref_ref,
                                step_null_block,
                                exec_args,
                                i32_ty,
                            );

                            let then_branch = fb.create_block();
                            let else_branch = fb.create_block();
                            fb.ins().brif(is_true, then_branch, &[], else_branch, &[]);

                            fb.switch_to_block(then_branch);
                            let then_args = emit_pack_target_args(
                                &mut fb,
                                then_params,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                            )
                            .ok_or_else(|| {
                                format!(
                                    "missing local mapping for then-branch args in block {}",
                                    plan.block_labels[index]
                                )
                            })?;
                            for value in &local_values {
                                fb.ins().call(decref_ref, &[*value]);
                            }
                            fb.ins().call(decref_ref, &[exec_args]);
                            fb.ins()
                                .jump(exec_blocks[*then_index], &[ir::BlockArg::Value(then_args)]);

                            fb.switch_to_block(else_branch);
                            let else_args = emit_pack_target_args(
                                &mut fb,
                                else_params,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                            )
                            .ok_or_else(|| {
                                format!(
                                    "missing local mapping for else-branch args in block {}",
                                    plan.block_labels[index]
                                )
                            })?;
                            for value in &local_values {
                                fb.ins().call(decref_ref, &[*value]);
                            }
                            fb.ins().call(decref_ref, &[exec_args]);
                            fb.ins()
                                .jump(exec_blocks[*else_index], &[ir::BlockArg::Value(else_args)]);
                        }
                        DirectSimpleTermPlan::BrTable {
                            index: table_index_expr,
                            targets,
                            default_index,
                            default_params,
                        } => {
                            let index_obj = emit_direct_simple_expr(
                                &mut fb,
                                table_index_expr,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                                false,
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
                                step_null_block,
                                &[ir::BlockArg::Value(exec_args)],
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

                            for ((target_index, target_params), case_block) in
                                targets.iter().zip(case_blocks.iter())
                            {
                                fb.switch_to_block(*case_block);
                                let next_args = emit_pack_target_args(
                                    &mut fb,
                                    target_params,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                )
                                .ok_or_else(|| {
                                    format!(
                                        "missing local mapping for br_table case args in block {}",
                                        plan.block_labels[index]
                                    )
                                })?;
                                for value in &local_values {
                                    fb.ins().call(decref_ref, &[*value]);
                                }
                                fb.ins().call(decref_ref, &[exec_args]);
                                fb.ins().jump(
                                    exec_blocks[*target_index],
                                    &[ir::BlockArg::Value(next_args)],
                                );
                            }

                            fb.switch_to_block(default_block);
                            let default_args = emit_pack_target_args(
                                &mut fb,
                                default_params,
                                &local_names,
                                &local_values,
                                &emit_ctx,
                                &mut literal_pool,
                            )
                            .ok_or_else(|| {
                                format!(
                                    "missing local mapping for br_table default args in block {}",
                                    plan.block_labels[index]
                                )
                            })?;
                            for value in &local_values {
                                fb.ins().call(decref_ref, &[*value]);
                            }
                            fb.ins().call(decref_ref, &[exec_args]);
                            fb.ins().jump(
                                exec_blocks[*default_index],
                                &[ir::BlockArg::Value(default_args)],
                            );
                        }
                        DirectSimpleTermPlan::Ret { value } => {
                            let ret_value = if let Some(ret_expr) = value.as_ref() {
                                emit_direct_simple_expr(
                                    &mut fb,
                                    ret_expr,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    false,
                                )
                            } else {
                                fb.ins().call(incref_ref, &[none_const]);
                                none_const
                            };
                            for value in &local_values {
                                fb.ins().call(decref_ref, &[*value]);
                            }
                            fb.ins().call(decref_ref, &[exec_args]);
                            fb.ins().return_(&[ret_value]);
                        }
                        DirectSimpleTermPlan::Raise { exc, cause } => {
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
                            let raise_fn_fail = fb.create_block();
                            let raise_fn_ok = fb.create_block();
                            fb.append_block_param(raise_fn_fail, ptr_ty);
                            fb.append_block_param(raise_fn_ok, ptr_ty);
                            fb.append_block_param(raise_fn_ok, ptr_ty);
                            fb.ins().brif(
                                raise_fn_null,
                                raise_fn_fail,
                                &[ir::BlockArg::Value(exec_args)],
                                raise_fn_ok,
                                &[
                                    ir::BlockArg::Value(exec_args),
                                    ir::BlockArg::Value(raise_fn),
                                ],
                            );

                            fb.switch_to_block(raise_fn_fail);
                            let rff_args = fb.block_params(raise_fn_fail)[0];
                            fb.ins()
                                .jump(emit_ctx.step_null_block, &[ir::BlockArg::Value(rff_args)]);

                            fb.switch_to_block(raise_fn_ok);
                            let rfo_args = fb.block_params(raise_fn_ok)[0];
                            let rfo_raise_fn = fb.block_params(raise_fn_ok)[1];
                            let exc_value = if let Some(exc_expr) = exc.as_ref() {
                                emit_direct_simple_expr(
                                    &mut fb,
                                    exc_expr,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    false,
                                )
                            } else {
                                fb.ins().call(incref_ref, &[none_const]);
                                none_const
                            };
                            let cause_value = if let Some(cause_expr) = cause.as_ref() {
                                emit_direct_simple_expr(
                                    &mut fb,
                                    cause_expr,
                                    &local_names,
                                    &local_values,
                                    &emit_ctx,
                                    &mut literal_pool,
                                    false,
                                )
                            } else {
                                fb.ins().call(incref_ref, &[none_const]);
                                none_const
                            };
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
                            let raise_exc_fail = fb.create_block();
                            let raise_exc_ok = fb.create_block();
                            fb.append_block_param(raise_exc_fail, ptr_ty);
                            fb.append_block_param(raise_exc_ok, ptr_ty);
                            fb.append_block_param(raise_exc_ok, ptr_ty);
                            fb.ins().brif(
                                raise_exc_null,
                                raise_exc_fail,
                                &[ir::BlockArg::Value(rfo_args)],
                                raise_exc_ok,
                                &[
                                    ir::BlockArg::Value(rfo_args),
                                    ir::BlockArg::Value(raise_exc_obj),
                                ],
                            );

                            fb.switch_to_block(raise_exc_fail);
                            let ref_args = fb.block_params(raise_exc_fail)[0];
                            fb.ins()
                                .jump(emit_ctx.step_null_block, &[ir::BlockArg::Value(ref_args)]);

                            fb.switch_to_block(raise_exc_ok);
                            let reo_args = fb.block_params(raise_exc_ok)[0];
                            let reo_exc_obj = fb.block_params(raise_exc_ok)[1];
                            let raise_inst = fb.ins().call(raise_exc_ref, &[reo_exc_obj]);
                            let raise_rc = fb.inst_results(raise_inst)[0];
                            fb.ins().call(decref_ref, &[reo_exc_obj]);
                            let raise_rc_fail = fb.create_block();
                            let raise_rc_ok = fb.create_block();
                            fb.append_block_param(raise_rc_fail, ptr_ty);
                            fb.append_block_param(raise_rc_ok, ptr_ty);
                            let raise_ok =
                                fb.ins().icmp_imm(ir::condcodes::IntCC::Equal, raise_rc, 0);
                            fb.ins().brif(
                                raise_ok,
                                raise_rc_ok,
                                &[ir::BlockArg::Value(reo_args)],
                                raise_rc_fail,
                                &[ir::BlockArg::Value(reo_args)],
                            );

                            fb.switch_to_block(raise_rc_fail);
                            let rcf_args = fb.block_params(raise_rc_fail)[0];
                            fb.ins()
                                .jump(emit_ctx.step_null_block, &[ir::BlockArg::Value(rcf_args)]);

                            fb.switch_to_block(raise_rc_ok);
                            let rco_args = fb.block_params(raise_rc_ok)[0];
                            for value in &local_values {
                                fb.ins().call(decref_ref, &[*value]);
                            }
                            fb.ins()
                                .jump(emit_ctx.step_null_block, &[ir::BlockArg::Value(rco_args)]);
                        }
                    }
                    continue;
                }
                BlockFastPath::None => {
                    return Err(format!(
                        "specialized JIT encountered unexpected slow-path block {}",
                        plan.block_labels[index]
                    ));
                }
            }
        }

        for (index, maybe_dispatch_block) in exception_dispatch_blocks.iter().enumerate() {
            let Some(dispatch_block) = *maybe_dispatch_block else {
                continue;
            };
            let Some(dispatch_plan) = plan.block_exc_dispatches[index].as_ref() else {
                continue;
            };

            fb.switch_to_block(dispatch_block);
            let d_args = fb.block_params(dispatch_block)[0];
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let none_const = fb.ins().iconst(ptr_ty, none_obj as i64);
            let raised_exc_inst = fb.ins().call(py_get_raised_exc_ref, &[]);
            let raised_exc = fb.inst_results(raised_exc_inst)[0];
            let raised_exc_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, raised_exc, null_ptr);
            let raised_exc_ok = fb.create_block();
            fb.append_block_param(raised_exc_ok, ptr_ty);
            fb.append_block_param(raised_exc_ok, ptr_ty);
            fb.ins().brif(
                raised_exc_null,
                step_null_block,
                &[ir::BlockArg::Value(d_args)],
                raised_exc_ok,
                &[ir::BlockArg::Value(d_args), ir::BlockArg::Value(raised_exc)],
            );

            fb.switch_to_block(raised_exc_ok);
            let reo_args = fb.block_params(raised_exc_ok)[0];
            let reo_exc = fb.block_params(raised_exc_ok)[1];
            let target_arity = fb
                .ins()
                .iconst(i64_ty, dispatch_plan.arg_sources.len() as i64);
            let target_args_inst = fb.ins().call(tuple_new_ref, &[target_arity]);
            let target_args = fb.inst_results(target_args_inst)[0];
            let target_args_null =
                fb.ins()
                    .icmp(ir::condcodes::IntCC::Equal, target_args, null_ptr);
            let dispatch_alloc_fail = fb.create_block();
            fb.append_block_param(dispatch_alloc_fail, ptr_ty);
            fb.append_block_param(dispatch_alloc_fail, ptr_ty);
            let dispatch_build_start = fb.create_block();
            fb.append_block_param(dispatch_build_start, ptr_ty);
            fb.append_block_param(dispatch_build_start, ptr_ty);
            fb.append_block_param(dispatch_build_start, ptr_ty);
            fb.ins().brif(
                target_args_null,
                dispatch_alloc_fail,
                &[ir::BlockArg::Value(reo_args), ir::BlockArg::Value(reo_exc)],
                dispatch_build_start,
                &[
                    ir::BlockArg::Value(reo_args),
                    ir::BlockArg::Value(reo_exc),
                    ir::BlockArg::Value(target_args),
                ],
            );

            fb.switch_to_block(dispatch_alloc_fail);
            let daf_args = fb.block_params(dispatch_alloc_fail)[0];
            let daf_exc = fb.block_params(dispatch_alloc_fail)[1];
            fb.ins().call(decref_ref, &[daf_exc]);
            fb.ins()
                .jump(step_null_block, &[ir::BlockArg::Value(daf_args)]);

            let mut build_block = dispatch_build_start;
            for (slot, source) in dispatch_plan.arg_sources.iter().enumerate() {
                fb.switch_to_block(build_block);
                let b_args = fb.block_params(build_block)[0];
                let b_exc = fb.block_params(build_block)[1];
                let b_target_args = fb.block_params(build_block)[2];
                let value = match source {
                    BlockExcArgSource::SourceParam { index } => {
                        let idx_const = fb.ins().iconst(i64_ty, *index as i64);
                        let value_inst = fb.ins().call(get_arg_item_ref, &[b_args, idx_const]);
                        fb.inst_results(value_inst)[0]
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
                        let owner_index = dispatch_plan
                            .owner_param_index
                            .expect("missing owner param index for frame-local exception dispatch");
                        let owner_idx_const = fb.ins().iconst(i64_ty, owner_index as i64);
                        let owner_inst =
                            fb.ins().call(get_arg_item_ref, &[b_args, owner_idx_const]);
                        let owner = fb.inst_results(owner_inst)[0];
                        let (name_ptr, name_len) =
                            intern_bytes_literal(&mut literal_pool, name.as_bytes());
                        let name_ptr_val = fb.ins().iconst(ptr_ty, name_ptr as i64);
                        let name_len_val = fb.ins().iconst(i64_ty, name_len);
                        let local_inst = fb.ins().call(
                            load_local_raw_by_name_ref,
                            &[owner, name_ptr_val, name_len_val],
                        );
                        fb.ins().call(decref_ref, &[owner]);
                        fb.inst_results(local_inst)[0]
                    }
                };
                let value_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, value, null_ptr);
                let value_fail = fb.create_block();
                fb.append_block_param(value_fail, ptr_ty);
                fb.append_block_param(value_fail, ptr_ty);
                fb.append_block_param(value_fail, ptr_ty);
                let value_ok = fb.create_block();
                fb.append_block_param(value_ok, ptr_ty);
                fb.append_block_param(value_ok, ptr_ty);
                fb.append_block_param(value_ok, ptr_ty);
                fb.append_block_param(value_ok, ptr_ty);
                fb.ins().brif(
                    value_null,
                    value_fail,
                    &[
                        ir::BlockArg::Value(b_args),
                        ir::BlockArg::Value(b_exc),
                        ir::BlockArg::Value(b_target_args),
                    ],
                    value_ok,
                    &[
                        ir::BlockArg::Value(b_args),
                        ir::BlockArg::Value(b_exc),
                        ir::BlockArg::Value(b_target_args),
                        ir::BlockArg::Value(value),
                    ],
                );

                fb.switch_to_block(value_fail);
                let vf_args = fb.block_params(value_fail)[0];
                let vf_exc = fb.block_params(value_fail)[1];
                let vf_target_args = fb.block_params(value_fail)[2];
                fb.ins().call(decref_ref, &[vf_target_args]);
                fb.ins().call(decref_ref, &[vf_exc]);
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(vf_args)]);

                fb.switch_to_block(value_ok);
                let vo_args = fb.block_params(value_ok)[0];
                let vo_exc = fb.block_params(value_ok)[1];
                let vo_target_args = fb.block_params(value_ok)[2];
                let vo_value = fb.block_params(value_ok)[3];
                let slot_const = fb.ins().iconst(i64_ty, slot as i64);
                let set_item_inst = fb
                    .ins()
                    .call(tuple_set_item_ref, &[vo_target_args, slot_const, vo_value]);
                let set_item_status = fb.inst_results(set_item_inst)[0];
                let set_item_failed =
                    fb.ins()
                        .icmp_imm(ir::condcodes::IntCC::NotEqual, set_item_status, 0);
                let set_item_fail = fb.create_block();
                fb.append_block_param(set_item_fail, ptr_ty);
                fb.append_block_param(set_item_fail, ptr_ty);
                fb.append_block_param(set_item_fail, ptr_ty);
                let next_build_block = fb.create_block();
                fb.append_block_param(next_build_block, ptr_ty);
                fb.append_block_param(next_build_block, ptr_ty);
                fb.append_block_param(next_build_block, ptr_ty);
                fb.ins().brif(
                    set_item_failed,
                    set_item_fail,
                    &[
                        ir::BlockArg::Value(vo_args),
                        ir::BlockArg::Value(vo_exc),
                        ir::BlockArg::Value(vo_target_args),
                    ],
                    next_build_block,
                    &[
                        ir::BlockArg::Value(vo_args),
                        ir::BlockArg::Value(vo_exc),
                        ir::BlockArg::Value(vo_target_args),
                    ],
                );

                fb.switch_to_block(set_item_fail);
                let sf_args = fb.block_params(set_item_fail)[0];
                let sf_exc = fb.block_params(set_item_fail)[1];
                let sf_target_args = fb.block_params(set_item_fail)[2];
                fb.ins().call(decref_ref, &[sf_target_args]);
                fb.ins().call(decref_ref, &[sf_exc]);
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(sf_args)]);

                build_block = next_build_block;
            }

            fb.switch_to_block(build_block);
            let bd_args = fb.block_params(build_block)[0];
            let bd_exc = fb.block_params(build_block)[1];
            let bd_target_args = fb.block_params(build_block)[2];
            fb.ins().call(decref_ref, &[bd_exc]);
            fb.ins().call(decref_ref, &[bd_args]);
            fb.ins().jump(
                exec_blocks[dispatch_plan.target_index],
                &[ir::BlockArg::Value(bd_target_args)],
            );
        }

        fb.switch_to_block(step_null_block);
        let step_null_args = fb.block_params(step_null_block)[0];
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
        fb.ins().return_(&[red_null]);

        fb.seal_all_blocks();
        fb.finalize();
    }

    Ok((ctx, main_id, literal_pool, import_id_to_symbol))
}

fn register_specialized_jit_symbols(builder: &mut JITBuilder) {
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol(
        "PyObject_CallFunctionObjArgs",
        dp_jit_py_call_three as *const u8,
    );
    builder.symbol("PyObject_CallObject", dp_jit_py_call_object as *const u8);
    builder.symbol(
        "dp_jit_py_call_with_kw",
        dp_jit_py_call_with_kw as *const u8,
    );
    builder.symbol(
        "PyErr_GetRaisedException",
        dp_jit_get_raised_exception as *const u8,
    );
    builder.symbol("dp_jit_get_arg_item", dp_jit_get_arg_item as *const u8);
    builder.symbol("dp_jit_make_int", dp_jit_make_int as *const u8);
    builder.symbol("dp_jit_make_float", dp_jit_make_float as *const u8);
    builder.symbol("dp_jit_make_bytes", dp_jit_make_bytes as *const u8);
    builder.symbol("dp_jit_load_name", dp_jit_load_name as *const u8);
    builder.symbol(
        "dp_jit_load_local_raw_by_name",
        dp_jit_load_local_raw_by_name as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_getattr",
        dp_jit_pyobject_getattr as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_setattr",
        dp_jit_pyobject_setattr as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_getitem",
        dp_jit_pyobject_getitem as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_setitem",
        dp_jit_pyobject_setitem as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_to_i64",
        dp_jit_pyobject_to_i64 as *const u8,
    );
    builder.symbol(
        "dp_jit_decode_literal_bytes",
        dp_jit_decode_literal_bytes as *const u8,
    );
    builder.symbol("dp_jit_tuple_new", dp_jit_tuple_new as *const u8);
    builder.symbol("dp_jit_tuple_set_item", dp_jit_tuple_set_item as *const u8);
    builder.symbol("dp_jit_is_true", dp_jit_is_true as *const u8);
    builder.symbol("dp_jit_compare_eq_obj", dp_jit_compare_eq_obj as *const u8);
    builder.symbol("dp_jit_compare_lt_obj", dp_jit_compare_lt_obj as *const u8);
    builder.symbol("dp_jit_raise_from_exc", dp_jit_raise_from_exc as *const u8);
}

pub unsafe fn render_cranelift_run_bb_specialized(
    blocks: &[ObjPtr],
    plan: &EntryBlockPlan,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
    empty_tuple_obj: ObjPtr,
) -> Result<String, String> {
    render_cranelift_run_bb_specialized_with_cfg(blocks, plan, true_obj, false_obj, empty_tuple_obj)
        .map(|rendered| rendered.clif)
}

pub unsafe fn render_cranelift_run_bb_specialized_with_cfg(
    blocks: &[ObjPtr],
    plan: &EntryBlockPlan,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
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
        empty_tuple_obj,
    )?;
    let mut out = String::new();
    out.push_str("; import fn aliases (Cranelift display id -> symbol)\n");
    out.push_str("; dp_jit_incref\n");
    out.push_str("; dp_jit_decref\n");
    out.push_str("; PyObject_CallFunctionObjArgs\n");
    out.push_str("; PyObject_CallObject\n");
    out.push_str("; dp_jit_py_call_with_kw\n");
    out.push_str("; PyErr_GetRaisedException\n");
    out.push_str("; dp_jit_get_arg_item\n");
    out.push_str("; dp_jit_make_int\n");
    out.push_str("; dp_jit_make_float\n");
    out.push_str("; dp_jit_make_bytes\n");
    out.push_str("; dp_jit_load_name\n");
    out.push_str("; dp_jit_load_local_raw_by_name\n");
    out.push_str("; dp_jit_pyobject_getattr\n");
    out.push_str("; dp_jit_pyobject_setattr\n");
    out.push_str("; dp_jit_pyobject_getitem\n");
    out.push_str("; dp_jit_pyobject_setitem\n");
    out.push_str("; dp_jit_pyobject_to_i64\n");
    out.push_str("; dp_jit_decode_literal_bytes\n");
    out.push_str("; dp_jit_tuple_new\n");
    out.push_str("; dp_jit_tuple_set_item\n");
    out.push_str("; dp_jit_is_true\n");
    out.push_str("; dp_jit_compare_eq_obj\n");
    out.push_str("; dp_jit_compare_lt_obj\n");
    out.push_str("; dp_jit_raise_from_exc\n");
    out.push('\n');
    let rendered_clif = ctx.func.display().to_string();
    out.push_str(&rewrite_import_fn_aliases(
        rendered_clif.as_str(),
        &import_id_to_symbol,
    ));
    let cfg_dot = CFGPrinter::new(&ctx.func).to_string();
    Ok(RenderedSpecializedClif { clif: out, cfg_dot })
}

unsafe fn configure_specialized_jit_hooks(
    incref_fn: IncrefFn,
    decref_fn: DecrefFn,
    py_call_three_fn: CallVarArgsFn,
    py_call_object_fn: CallObjectFn,
    py_call_with_kw_fn: CallWithKwFn,
    py_get_raised_exception_fn: GetRaisedExceptionFn,
    get_arg_item_fn: GetArgItemFn,
    make_int_fn: MakeIntFn,
    make_float_fn: MakeFloatFn,
    make_bytes_fn: MakeBytesFn,
    load_name_fn: LoadNameFn,
    load_local_raw_by_name_fn: LoadLocalRawByNameFn,
    pyobject_getattr_fn: PyObjectGetAttrFn,
    pyobject_setattr_fn: PyObjectSetAttrFn,
    pyobject_getitem_fn: PyObjectGetItemFn,
    pyobject_setitem_fn: PyObjectSetItemFn,
    pyobject_to_i64_fn: PyObjectToI64Fn,
    decode_literal_bytes_fn: DecodeLiteralBytesFn,
    tuple_new_fn: TupleNewFn,
    tuple_set_item_fn: TupleSetItemFn,
    is_true_fn: IsTrueFn,
    compare_eq_obj_fn: CompareObjFn,
    compare_lt_obj_fn: CompareObjFn,
    raise_from_exc_fn: RaiseFromExcFn,
) {
    DP_JIT_INCREF_FN = Some(incref_fn);
    DP_JIT_DECREF_FN = Some(decref_fn);
    DP_JIT_CALL_VAR_ARGS_FN = Some(py_call_three_fn);
    DP_JIT_CALL_OBJECT_FN = Some(py_call_object_fn);
    DP_JIT_CALL_WITH_KW_FN = Some(py_call_with_kw_fn);
    DP_JIT_GET_RAISED_EXCEPTION_FN = Some(py_get_raised_exception_fn);
    DP_JIT_GET_ARG_ITEM_FN = Some(get_arg_item_fn);
    DP_JIT_MAKE_INT_FN = Some(make_int_fn);
    DP_JIT_MAKE_FLOAT_FN = Some(make_float_fn);
    DP_JIT_MAKE_BYTES_FN = Some(make_bytes_fn);
    DP_JIT_LOAD_NAME_FN = Some(load_name_fn);
    DP_JIT_LOAD_LOCAL_RAW_BY_NAME_FN = Some(load_local_raw_by_name_fn);
    DP_JIT_PYOBJECT_GETATTR_FN = Some(pyobject_getattr_fn);
    DP_JIT_PYOBJECT_SETATTR_FN = Some(pyobject_setattr_fn);
    DP_JIT_PYOBJECT_GETITEM_FN = Some(pyobject_getitem_fn);
    DP_JIT_PYOBJECT_SETITEM_FN = Some(pyobject_setitem_fn);
    DP_JIT_PYOBJECT_TO_I64_FN = Some(pyobject_to_i64_fn);
    DP_JIT_DECODE_LITERAL_BYTES_FN = Some(decode_literal_bytes_fn);
    DP_JIT_TUPLE_NEW_FN = Some(tuple_new_fn);
    DP_JIT_TUPLE_SET_ITEM_FN = Some(tuple_set_item_fn);
    DP_JIT_IS_TRUE_FN = Some(is_true_fn);
    DP_JIT_COMPARE_EQ_OBJ_FN = Some(compare_eq_obj_fn);
    DP_JIT_COMPARE_LT_OBJ_FN = Some(compare_lt_obj_fn);
    DP_JIT_RAISE_FROM_EXC_FN = Some(raise_from_exc_fn);
}

pub unsafe fn compile_cranelift_run_bb_specialized_cached(
    blocks: &[ObjPtr],
    plan: &EntryBlockPlan,
    globals_obj: ObjPtr,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
    none_obj: ObjPtr,
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

pub unsafe fn run_cranelift_run_bb_specialized_cached(
    compiled_handle: ObjPtr,
    args: ObjPtr,
    incref_fn: IncrefFn,
    decref_fn: DecrefFn,
    py_call_three_fn: CallVarArgsFn,
    py_call_object_fn: CallObjectFn,
    py_call_with_kw_fn: CallWithKwFn,
    py_get_raised_exception_fn: GetRaisedExceptionFn,
    get_arg_item_fn: GetArgItemFn,
    make_int_fn: MakeIntFn,
    make_float_fn: MakeFloatFn,
    make_bytes_fn: MakeBytesFn,
    load_name_fn: LoadNameFn,
    load_local_raw_by_name_fn: LoadLocalRawByNameFn,
    pyobject_getattr_fn: PyObjectGetAttrFn,
    pyobject_setattr_fn: PyObjectSetAttrFn,
    pyobject_getitem_fn: PyObjectGetItemFn,
    pyobject_setitem_fn: PyObjectSetItemFn,
    pyobject_to_i64_fn: PyObjectToI64Fn,
    decode_literal_bytes_fn: DecodeLiteralBytesFn,
    tuple_new_fn: TupleNewFn,
    tuple_set_item_fn: TupleSetItemFn,
    is_true_fn: IsTrueFn,
    compare_eq_obj_fn: CompareObjFn,
    compare_lt_obj_fn: CompareObjFn,
    raise_from_exc_fn: RaiseFromExcFn,
) -> Result<ObjPtr, String> {
    if compiled_handle.is_null() {
        return Err("invalid null compiled handle passed to specialized JIT run_bb".to_string());
    }
    if args.is_null() {
        return Err("invalid null args passed to specialized JIT run_bb".to_string());
    }
    configure_specialized_jit_hooks(
        incref_fn,
        decref_fn,
        py_call_three_fn,
        py_call_object_fn,
        py_call_with_kw_fn,
        py_get_raised_exception_fn,
        get_arg_item_fn,
        make_int_fn,
        make_float_fn,
        make_bytes_fn,
        load_name_fn,
        load_local_raw_by_name_fn,
        pyobject_getattr_fn,
        pyobject_setattr_fn,
        pyobject_getitem_fn,
        pyobject_setitem_fn,
        pyobject_to_i64_fn,
        decode_literal_bytes_fn,
        tuple_new_fn,
        tuple_set_item_fn,
        is_true_fn,
        compare_eq_obj_fn,
        compare_lt_obj_fn,
        raise_from_exc_fn,
    );
    let compiled = &*(compiled_handle as *const CompiledSpecializedRunner);
    let Some(entry) = compiled.entry else {
        return Err("invalid compiled handle without entrypoint".to_string());
    };
    Ok(entry(args))
}

pub unsafe fn free_cranelift_run_bb_specialized_cached(compiled_handle: ObjPtr) {
    if compiled_handle.is_null() {
        return;
    }
    let _ = Box::from_raw(compiled_handle as *mut CompiledSpecializedRunner);
}

pub unsafe fn run_cranelift_run_bb_specialized(
    blocks: &[ObjPtr],
    plan: &EntryBlockPlan,
    globals_obj: ObjPtr,
    true_obj: ObjPtr,
    false_obj: ObjPtr,
    args: ObjPtr,
    incref_fn: IncrefFn,
    decref_fn: DecrefFn,
    py_call_three_fn: CallVarArgsFn,
    py_call_object_fn: CallObjectFn,
    py_call_with_kw_fn: CallWithKwFn,
    py_get_raised_exception_fn: GetRaisedExceptionFn,
    get_arg_item_fn: GetArgItemFn,
    make_int_fn: MakeIntFn,
    make_float_fn: MakeFloatFn,
    make_bytes_fn: MakeBytesFn,
    load_name_fn: LoadNameFn,
    load_local_raw_by_name_fn: LoadLocalRawByNameFn,
    pyobject_getattr_fn: PyObjectGetAttrFn,
    pyobject_setattr_fn: PyObjectSetAttrFn,
    pyobject_getitem_fn: PyObjectGetItemFn,
    pyobject_setitem_fn: PyObjectSetItemFn,
    pyobject_to_i64_fn: PyObjectToI64Fn,
    decode_literal_bytes_fn: DecodeLiteralBytesFn,
    tuple_new_fn: TupleNewFn,
    tuple_set_item_fn: TupleSetItemFn,
    is_true_fn: IsTrueFn,
    compare_eq_obj_fn: CompareObjFn,
    compare_lt_obj_fn: CompareObjFn,
    raise_from_exc_fn: RaiseFromExcFn,
    none_obj: ObjPtr,
    empty_tuple_obj: ObjPtr,
) -> Result<ObjPtr, String> {
    if args.is_null() {
        return Err("invalid null args passed to specialized JIT run_bb".to_string());
    }
    if globals_obj.is_null() {
        return Err("invalid null globals object passed to specialized JIT run_bb".to_string());
    }
    configure_specialized_jit_hooks(
        incref_fn,
        decref_fn,
        py_call_three_fn,
        py_call_object_fn,
        py_call_with_kw_fn,
        py_get_raised_exception_fn,
        get_arg_item_fn,
        make_int_fn,
        make_float_fn,
        make_bytes_fn,
        load_name_fn,
        load_local_raw_by_name_fn,
        pyobject_getattr_fn,
        pyobject_setattr_fn,
        pyobject_getitem_fn,
        pyobject_setitem_fn,
        pyobject_to_i64_fn,
        decode_literal_bytes_fn,
        tuple_new_fn,
        tuple_set_item_fn,
        is_true_fn,
        compare_eq_obj_fn,
        compare_lt_obj_fn,
        raise_from_exc_fn,
    );
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
    let compiled: extern "C" fn(ObjPtr) -> ObjPtr = std::mem::transmute(code_ptr);
    Ok(compiled(args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_specialized_jit_clif_smoke() {
        let blocks = [1usize as ObjPtr, 2usize as ObjPtr, 3usize as ObjPtr];
        let plan = EntryBlockPlan {
            entry_index: 1,
            block_labels: vec!["b0".into(), "b1".into(), "b2".into()],
            block_param_names: vec![vec![], vec![], vec![]],
            block_terms: vec![
                BlockTermPlan::Ret,
                BlockTermPlan::BrIf {
                    then_index: 2,
                    else_index: 0,
                },
                BlockTermPlan::Ret,
            ],
            block_exc_targets: vec![None, None, None],
            block_exc_dispatches: vec![None, None, None],
            block_fast_paths: vec![
                BlockFastPath::None,
                BlockFastPath::None,
                BlockFastPath::None,
            ],
        };
        let err = unsafe {
            render_cranelift_run_bb_specialized(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
            )
        }
        .expect_err("specialized JIT CLIF render should reject slow-path blocks");
        assert!(
            err.contains("fully lowered fastpath blocks"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn render_specialized_jit_fastpath_ret_none_avoids_block_call() {
        let blocks = [1usize as ObjPtr];
        let plan = EntryBlockPlan {
            entry_index: 0,
            block_labels: vec!["b0".into()],
            block_param_names: vec![vec![]],
            block_terms: vec![BlockTermPlan::Ret],
            block_exc_targets: vec![None],
            block_exc_dispatches: vec![None],
            block_fast_paths: vec![BlockFastPath::ReturnNone],
        };
        let clif = unsafe {
            render_cranelift_run_bb_specialized(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed");
        assert!(
            !clif.contains("call PyObject_CallObject"),
            "fast-path ret-none should avoid block function calls:\n{clif}"
        );
        assert!(
            !clif.contains("call PyObject_CallFunctionObjArgs"),
            "fast-path ret-none should avoid helper Python calls:\n{clif}"
        );
    }
}

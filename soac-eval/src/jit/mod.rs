use cranelift_codegen::cfg_printer::CFGPrinter;
use cranelift_codegen::ir;
use cranelift_codegen::ir::InstBuilder;
use cranelift_codegen::settings;
use cranelift_codegen::settings::Configurable;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Switch};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use dp_transform::basic_block::bb_ir::{BbExpr, BbModule, BbOp, BbTerm};
use ruff_python_ast::Number;
use std::borrow::Cow;
use std::collections::HashMap;
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
type TermKindFn = unsafe extern "C" fn(ObjPtr) -> i64;
type TermJumpTargetFn = unsafe extern "C" fn(ObjPtr) -> ObjPtr;
type TermJumpArgsFn = unsafe extern "C" fn(ObjPtr) -> ObjPtr;
type TermRetValueFn = unsafe extern "C" fn(ObjPtr) -> ObjPtr;
type TermRaiseExcFn = unsafe extern "C" fn(ObjPtr) -> ObjPtr;
type TermInvalidFn = unsafe extern "C" fn(ObjPtr) -> i32;
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
static mut DP_JIT_TERM_KIND_FN: Option<TermKindFn> = None;
static mut DP_JIT_TERM_JUMP_TARGET_FN: Option<TermJumpTargetFn> = None;
static mut DP_JIT_TERM_JUMP_ARGS_FN: Option<TermJumpArgsFn> = None;
static mut DP_JIT_TERM_RET_VALUE_FN: Option<TermRetValueFn> = None;
static mut DP_JIT_TERM_RAISE_EXC_FN: Option<TermRaiseExcFn> = None;
static mut DP_JIT_TERM_INVALID_FN: Option<TermInvalidFn> = None;
static mut DP_JIT_RAISE_FROM_EXC_FN: Option<RaiseFromExcFn> = None;

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
        args: Vec<DirectSimpleExprPlan>,
        keywords: Vec<(String, DirectSimpleExprPlan)>,
    },
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

static BB_PLAN_REGISTRY: OnceLock<Mutex<PlanRegistry>> = OnceLock::new();

fn bb_plan_registry() -> &'static Mutex<PlanRegistry> {
    BB_PLAN_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn direct_simple_expr_from(expr: &BbExpr) -> Option<DirectSimpleExprPlan> {
    fn helper_name_expr(name: &str) -> DirectSimpleExprPlan {
        DirectSimpleExprPlan::Name(name.to_string())
    }

    fn call_helper_with_tuple_arg(
        helper_name: &str,
        items: Vec<DirectSimpleExprPlan>,
    ) -> DirectSimpleExprPlan {
        DirectSimpleExprPlan::Call {
            func: Box::new(helper_name_expr(helper_name)),
            args: vec![DirectSimpleExprPlan::Tuple(items)],
            keywords: Vec::new(),
        }
    }

    match expr {
        BbExpr::Await(_) => None,
        BbExpr::Name(name) => Some(DirectSimpleExprPlan::Name(name.id.to_string())),
        BbExpr::NumberLiteral(number) => match &number.value {
            Number::Int(value) => value.as_i64().map(DirectSimpleExprPlan::Int),
            Number::Float(value) => Some(DirectSimpleExprPlan::Float(*value)),
            Number::Complex { .. } => None,
        },
        BbExpr::BytesLiteral(bytes) => {
            let value: Cow<[u8]> = (&bytes.value).into();
            Some(DirectSimpleExprPlan::Bytes(value.into_owned()))
        }
        BbExpr::Starred(_) => None,
        BbExpr::TupleLiteral(tuple) => {
            let mut items = Vec::with_capacity(tuple.elts.len());
            for elt in &tuple.elts {
                items.push(direct_simple_expr_from(&BbExpr::from_expr(elt.clone()))?);
            }
            Some(DirectSimpleExprPlan::Tuple(items))
        }
        BbExpr::ListLiteral(list) => {
            let mut items = Vec::with_capacity(list.elts.len());
            for elt in &list.elts {
                items.push(direct_simple_expr_from(&BbExpr::from_expr(elt.clone()))?);
            }
            Some(call_helper_with_tuple_arg("__dp_list", items))
        }
        BbExpr::SetLiteral(set) => {
            let mut items = Vec::with_capacity(set.elts.len());
            for elt in &set.elts {
                items.push(direct_simple_expr_from(&BbExpr::from_expr(elt.clone()))?);
            }
            Some(call_helper_with_tuple_arg("__dp_set", items))
        }
        BbExpr::DictLiteral(dict) => {
            let mut kv_items = Vec::with_capacity(dict.items.len());
            for item in &dict.items {
                let key = item.key.as_ref()?;
                let key_expr = direct_simple_expr_from(&BbExpr::from_expr(key.clone()))?;
                let value_expr = direct_simple_expr_from(&BbExpr::from_expr(item.value.clone()))?;
                kv_items.push(DirectSimpleExprPlan::Tuple(vec![key_expr, value_expr]));
            }
            Some(call_helper_with_tuple_arg("__dp_dict", kv_items))
        }
        BbExpr::Call(call) => {
            let func = direct_simple_expr_from(call.func.as_ref())?;
            let mut args = Vec::with_capacity(call.args.len());
            for arg in &call.args {
                args.push(direct_simple_expr_from(arg)?);
            }
            let mut keywords = Vec::with_capacity(call.keywords.len());
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
                let name = keyword.arg.as_ref()?.to_string();
                keywords.push((name, direct_simple_expr_from(value)?));
            }
            Some(DirectSimpleExprPlan::Call {
                func: Box::new(func),
                args,
                keywords,
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
    known_names: &[String],
) -> Option<Vec<String>> {
    let params = function.blocks[target_index].params.clone();
    if params
        .iter()
        .all(|name| known_names.iter().any(|known| known == name))
    {
        Some(params)
    } else {
        None
    }
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
            let target_params = target_params_from_index(function, target_index, &known_names)?;
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
            let then_params = target_params_from_index(function, then_index, &known_names)?;
            let else_index = *label_to_index.get(else_label.as_str())?;
            let else_params = target_params_from_index(function, else_index, &known_names)?;
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
                let target_params = target_params_from_index(function, target_index, &known_names)?;
                target_plans.push((target_index, target_params));
            }
            let default_index = *label_to_index.get(default_label.as_str())?;
            let default_params = target_params_from_index(function, default_index, &known_names)?;
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
        DirectSimpleExprPlan::Call {
            func,
            args,
            keywords,
        } => {
            assert!(
                !borrowed,
                "direct simple plan must not use borrowed call expression"
            );
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
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
    for (index, name) in target_params.iter().enumerate() {
        let value_index = local_names.iter().position(|candidate| candidate == name)?;
        let value = local_values[value_index];
        // PyTuple_SetItem steals a reference; increment first when using local slot values.
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
    // Generator/async-generator block wrappers currently inject additional
    // resume/send checks during Python rendering that are not yet represented
    // in BbOp/BbTerm. Restrict direct JIT fast-path elision to plain functions.
    let allow_fast_path = matches!(
        function.kind,
        dp_transform::basic_block::bb_ir::BbFunctionKind::Function
    );
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
                arg_sources.push(BlockExcArgSource::FrameLocal {
                    name: target_param.clone(),
                });
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
        let fast_path = if allow_fast_path && exc_target.is_none() {
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
        } else {
            BlockFastPath::None
        };
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
    let mut plans = HashMap::new();
    let mut skipped_errors: HashMap<String, String> = HashMap::new();
    for function in &lowered.functions {
        match build_entry_plan(function) {
            Ok(plan) => {
                plans.insert(
                    PlanKey {
                        module: module_name.to_string(),
                        qualname: function.qualname.clone(),
                    },
                    plan,
                );
            }
            Err(err) => {
                skipped_errors.insert(function.qualname.clone(), err);
            }
        }
    }

    if let Some(module_init_qualname) = lowered.module_init.as_ref() {
        if !plans.keys().any(|k| &k.qualname == module_init_qualname) {
            let detail = skipped_errors
                .remove(module_init_qualname)
                .unwrap_or_else(|| "unknown planning error".to_string());
            return Err(format!(
                "failed to build JIT plan for module init {module_name}.{module_init_qualname}: {detail}"
            ));
        }
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

unsafe extern "C" fn dp_jit_term_raise_exc(term: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_TERM_RAISE_EXC_FN {
        return func(term);
    }
    ptr::null_mut()
}

unsafe extern "C" fn dp_jit_term_invalid(term: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_TERM_INVALID_FN {
        return func(term);
    }
    -1
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

    jit_module
        .define_function(main_id, &mut ctx)
        .map_err(|err| format!("failed to define jit call smoke function: {err}"))?;
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

    let mut term_raise_exc_sig = jit_module.make_signature();
    term_raise_exc_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_raise_exc_sig.returns.push(ir::AbiParam::new(ptr_ty));

    let mut raise_exc_sig = jit_module.make_signature();
    raise_exc_sig.params.push(ir::AbiParam::new(ptr_ty));
    raise_exc_sig.returns.push(ir::AbiParam::new(i32_ty));

    let mut term_invalid_sig = jit_module.make_signature();
    term_invalid_sig.params.push(ir::AbiParam::new(ptr_ty));
    term_invalid_sig.returns.push(ir::AbiParam::new(i32_ty));

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
    let term_kind_id = declare_import_fn(jit_module, "dp_jit_term_kind", &term_kind_sig)?;
    let term_jump_target_id =
        declare_import_fn(jit_module, "dp_jit_term_jump_target", &term_jump_target_sig)?;
    let term_jump_args_id =
        declare_import_fn(jit_module, "dp_jit_term_jump_args", &term_jump_args_sig)?;
    let term_ret_value_id =
        declare_import_fn(jit_module, "dp_jit_term_ret_value", &term_ret_value_sig)?;
    let term_raise_exc_id =
        declare_import_fn(jit_module, "dp_jit_term_raise_exc", &term_raise_exc_sig)?;
    let raise_exc_id = declare_import_fn(jit_module, "dp_jit_raise_from_exc", &raise_exc_sig)?;
    let term_invalid_id = declare_import_fn(jit_module, "dp_jit_term_invalid", &term_invalid_sig)?;
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
    import_id_to_symbol.insert(term_kind_id.as_u32(), "dp_jit_term_kind");
    import_id_to_symbol.insert(term_jump_target_id.as_u32(), "dp_jit_term_jump_target");
    import_id_to_symbol.insert(term_jump_args_id.as_u32(), "dp_jit_term_jump_args");
    import_id_to_symbol.insert(term_ret_value_id.as_u32(), "dp_jit_term_ret_value");
    import_id_to_symbol.insert(term_raise_exc_id.as_u32(), "dp_jit_term_raise_exc");
    import_id_to_symbol.insert(raise_exc_id.as_u32(), "dp_jit_raise_from_exc");
    import_id_to_symbol.insert(term_invalid_id.as_u32(), "dp_jit_term_invalid");

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
        let ret_block = fb.create_block();
        let raise_block = fb.create_block();
        let raise_exc_direct_block = fb.create_block();
        let invalid_term_block = fb.create_block();
        let invalid_jump_null_block = fb.create_block();
        let jump_invalid_target_block = fb.create_block();

        fb.append_block_params_for_function_params(entry_block);
        for block in &exec_blocks {
            fb.append_block_param(*block, ptr_ty); // args
        }
        fb.append_block_param(step_null_block, ptr_ty); // args
        fb.append_block_param(ret_block, ptr_ty); // args
        fb.append_block_param(ret_block, ptr_ty); // term
        fb.append_block_param(raise_block, ptr_ty); // args
        fb.append_block_param(raise_block, ptr_ty); // term
        fb.append_block_param(raise_exc_direct_block, ptr_ty); // args
        fb.append_block_param(raise_exc_direct_block, ptr_ty); // exc
        fb.append_block_param(invalid_term_block, ptr_ty); // args
        fb.append_block_param(invalid_term_block, ptr_ty); // term
        fb.append_block_param(invalid_jump_null_block, ptr_ty); // args
        fb.append_block_param(invalid_jump_null_block, ptr_ty); // term
        fb.append_block_param(invalid_jump_null_block, ptr_ty); // target
        fb.append_block_param(jump_invalid_target_block, ptr_ty); // args
        fb.append_block_param(jump_invalid_target_block, ptr_ty); // term
        fb.append_block_param(jump_invalid_target_block, ptr_ty); // next_args

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
        let term_kind_ref = jit_module.declare_func_in_func(term_kind_id, &mut fb.func);
        let term_jump_target_ref =
            jit_module.declare_func_in_func(term_jump_target_id, &mut fb.func);
        let term_jump_args_ref = jit_module.declare_func_in_func(term_jump_args_id, &mut fb.func);
        let term_ret_value_ref = jit_module.declare_func_in_func(term_ret_value_id, &mut fb.func);
        let term_raise_exc_ref = jit_module.declare_func_in_func(term_raise_exc_id, &mut fb.func);
        let raise_exc_ref = jit_module.declare_func_in_func(raise_exc_id, &mut fb.func);
        let term_invalid_ref = jit_module.declare_func_in_func(term_invalid_id, &mut fb.func);
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

        for (index, block) in exec_blocks.iter().enumerate() {
            fb.switch_to_block(*block);
            let exec_args = fb.block_params(*block)[0];
            let block_const = fb.ins().iconst(ptr_ty, globals_obj as i64);
            let none_const = fb.ins().iconst(ptr_ty, none_obj as i64);
            let true_const = fb.ins().iconst(ptr_ty, true_obj as i64);
            let false_const = fb.ins().iconst(ptr_ty, false_obj as i64);
            let empty_tuple_const = fb.ins().iconst(ptr_ty, empty_tuple_obj as i64);
            let emit_ctx = DirectSimpleEmitCtx {
                incref_ref,
                decref_ref,
                py_call_ref,
                make_int_ref,
                step_null_block,
                exec_args,
                ptr_ty,
                i64_ty,
                none_const,
                true_const,
                false_const,
                empty_tuple_const,
                block_const,
                load_name_ref,
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
                        let value = emit_direct_simple_expr(
                            &mut fb,
                            &assign.value,
                            &local_names,
                            &local_values,
                            &emit_ctx,
                            &mut literal_pool,
                            false,
                        );

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
                    }
                    continue;
                }
                BlockFastPath::None => {}
            }
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let exc_dispatch_plan = plan.block_exc_dispatches[index].as_ref();
            let exc_target_index = exc_dispatch_plan.map(|dispatch| dispatch.target_index);
            let kind_dispatch_start = fb.create_block();
            fb.append_block_param(kind_dispatch_start, ptr_ty);
            fb.append_block_param(kind_dispatch_start, ptr_ty);
            let block_params_ready_block = fb.create_block();
            fb.append_block_param(block_params_ready_block, ptr_ty);
            let param_names = &plan.block_param_names[index];
            if param_names.is_empty() {
                fb.ins().call(incref_ref, &[empty_tuple_const]);
                fb.ins().jump(
                    block_params_ready_block,
                    &[ir::BlockArg::Value(empty_tuple_const)],
                );
            } else {
                let (list_name_ptr, list_name_len) =
                    intern_bytes_literal(&mut literal_pool, b"list");
                let list_name_ptr_val = fb.ins().iconst(ptr_ty, list_name_ptr as i64);
                let list_name_len_val = fb.ins().iconst(i64_ty, list_name_len);
                let list_callable_inst = fb.ins().call(
                    load_name_ref,
                    &[block_const, list_name_ptr_val, list_name_len_val],
                );
                let list_callable = fb.inst_results(list_callable_inst)[0];
                let list_callable_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, list_callable, null_ptr);
                let list_callable_fail = fb.create_block();
                fb.append_block_param(list_callable_fail, ptr_ty);
                let list_callable_ok = fb.create_block();
                fb.append_block_param(list_callable_ok, ptr_ty);
                fb.append_block_param(list_callable_ok, ptr_ty);
                fb.ins().brif(
                    list_callable_null,
                    list_callable_fail,
                    &[ir::BlockArg::Value(exec_args)],
                    list_callable_ok,
                    &[
                        ir::BlockArg::Value(exec_args),
                        ir::BlockArg::Value(list_callable),
                    ],
                );

                fb.switch_to_block(list_callable_fail);
                let lcf_args = fb.block_params(list_callable_fail)[0];
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(lcf_args)]);

                fb.switch_to_block(list_callable_ok);
                let lco_args = fb.block_params(list_callable_ok)[0];
                let lco_list_callable = fb.block_params(list_callable_ok)[1];
                let args_list_inst = fb.ins().call(
                    py_call_ref,
                    &[lco_list_callable, lco_args, null_ptr, null_ptr, null_ptr],
                );
                fb.ins().call(decref_ref, &[lco_list_callable]);
                let args_list = fb.inst_results(args_list_inst)[0];
                let args_list_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, args_list, null_ptr);
                let args_list_fail = fb.create_block();
                fb.append_block_param(args_list_fail, ptr_ty);
                let args_list_ok = fb.create_block();
                fb.append_block_param(args_list_ok, ptr_ty);
                fb.append_block_param(args_list_ok, ptr_ty);
                fb.ins().brif(
                    args_list_null,
                    args_list_fail,
                    &[ir::BlockArg::Value(lco_args)],
                    args_list_ok,
                    &[
                        ir::BlockArg::Value(lco_args),
                        ir::BlockArg::Value(args_list),
                    ],
                );

                fb.switch_to_block(args_list_fail);
                let alf_args = fb.block_params(args_list_fail)[0];
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(alf_args)]);

                fb.switch_to_block(args_list_ok);
                let alo_args = fb.block_params(args_list_ok)[0];
                let alo_args_list = fb.block_params(args_list_ok)[1];
                let (block_param_name_ptr, block_param_name_len) =
                    intern_bytes_literal(&mut literal_pool, b"__dp_BlockParam");
                let block_param_name_ptr_val = fb.ins().iconst(ptr_ty, block_param_name_ptr as i64);
                let block_param_name_len_val = fb.ins().iconst(i64_ty, block_param_name_len);
                let block_param_callable_inst = fb.ins().call(
                    load_name_ref,
                    &[
                        block_const,
                        block_param_name_ptr_val,
                        block_param_name_len_val,
                    ],
                );
                let block_param_callable = fb.inst_results(block_param_callable_inst)[0];
                let block_param_callable_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, block_param_callable, null_ptr);
                let block_param_callable_fail = fb.create_block();
                fb.append_block_param(block_param_callable_fail, ptr_ty);
                fb.append_block_param(block_param_callable_fail, ptr_ty);
                let block_param_callable_ok = fb.create_block();
                fb.append_block_param(block_param_callable_ok, ptr_ty);
                fb.append_block_param(block_param_callable_ok, ptr_ty);
                fb.append_block_param(block_param_callable_ok, ptr_ty);
                fb.ins().brif(
                    block_param_callable_null,
                    block_param_callable_fail,
                    &[
                        ir::BlockArg::Value(alo_args),
                        ir::BlockArg::Value(alo_args_list),
                    ],
                    block_param_callable_ok,
                    &[
                        ir::BlockArg::Value(alo_args),
                        ir::BlockArg::Value(alo_args_list),
                        ir::BlockArg::Value(block_param_callable),
                    ],
                );

                fb.switch_to_block(block_param_callable_fail);
                let bpcf_args = fb.block_params(block_param_callable_fail)[0];
                let bpcf_args_list = fb.block_params(block_param_callable_fail)[1];
                fb.ins().call(decref_ref, &[bpcf_args_list]);
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(bpcf_args)]);

                fb.switch_to_block(block_param_callable_ok);
                let bpco_args = fb.block_params(block_param_callable_ok)[0];
                let bpco_args_list = fb.block_params(block_param_callable_ok)[1];
                let bpco_callable = fb.block_params(block_param_callable_ok)[2];
                let block_params_arity = fb.ins().iconst(i64_ty, param_names.len() as i64);
                let block_params_inst = fb.ins().call(tuple_new_ref, &[block_params_arity]);
                let block_params = fb.inst_results(block_params_inst)[0];
                let block_params_null =
                    fb.ins()
                        .icmp(ir::condcodes::IntCC::Equal, block_params, null_ptr);
                let block_params_fail = fb.create_block();
                fb.append_block_param(block_params_fail, ptr_ty);
                fb.append_block_param(block_params_fail, ptr_ty);
                fb.append_block_param(block_params_fail, ptr_ty);
                let block_params_build_start = fb.create_block();
                fb.append_block_param(block_params_build_start, ptr_ty);
                fb.append_block_param(block_params_build_start, ptr_ty);
                fb.append_block_param(block_params_build_start, ptr_ty);
                fb.append_block_param(block_params_build_start, ptr_ty);
                fb.ins().brif(
                    block_params_null,
                    block_params_fail,
                    &[
                        ir::BlockArg::Value(bpco_args),
                        ir::BlockArg::Value(bpco_args_list),
                        ir::BlockArg::Value(bpco_callable),
                    ],
                    block_params_build_start,
                    &[
                        ir::BlockArg::Value(bpco_args),
                        ir::BlockArg::Value(bpco_args_list),
                        ir::BlockArg::Value(bpco_callable),
                        ir::BlockArg::Value(block_params),
                    ],
                );

                fb.switch_to_block(block_params_fail);
                let bpf_args = fb.block_params(block_params_fail)[0];
                let bpf_args_list = fb.block_params(block_params_fail)[1];
                let bpf_callable = fb.block_params(block_params_fail)[2];
                fb.ins().call(decref_ref, &[bpf_callable]);
                fb.ins().call(decref_ref, &[bpf_args_list]);
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(bpf_args)]);

                let mut build_block = block_params_build_start;
                for slot in 0..param_names.len() {
                    fb.switch_to_block(build_block);
                    let b_args = fb.block_params(build_block)[0];
                    let b_args_list = fb.block_params(build_block)[1];
                    let b_callable = fb.block_params(build_block)[2];
                    let b_block_params = fb.block_params(build_block)[3];
                    let idx_const = fb.ins().iconst(i64_ty, slot as i64);
                    let idx_obj_inst = fb.ins().call(make_int_ref, &[idx_const]);
                    let idx_obj = fb.inst_results(idx_obj_inst)[0];
                    let idx_obj_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, idx_obj, null_ptr);
                    let idx_obj_fail = fb.create_block();
                    fb.append_block_param(idx_obj_fail, ptr_ty);
                    fb.append_block_param(idx_obj_fail, ptr_ty);
                    fb.append_block_param(idx_obj_fail, ptr_ty);
                    fb.append_block_param(idx_obj_fail, ptr_ty);
                    let idx_obj_ok = fb.create_block();
                    fb.append_block_param(idx_obj_ok, ptr_ty);
                    fb.append_block_param(idx_obj_ok, ptr_ty);
                    fb.append_block_param(idx_obj_ok, ptr_ty);
                    fb.append_block_param(idx_obj_ok, ptr_ty);
                    fb.append_block_param(idx_obj_ok, ptr_ty);
                    fb.ins().brif(
                        idx_obj_null,
                        idx_obj_fail,
                        &[
                            ir::BlockArg::Value(b_args),
                            ir::BlockArg::Value(b_args_list),
                            ir::BlockArg::Value(b_callable),
                            ir::BlockArg::Value(b_block_params),
                        ],
                        idx_obj_ok,
                        &[
                            ir::BlockArg::Value(b_args),
                            ir::BlockArg::Value(b_args_list),
                            ir::BlockArg::Value(b_callable),
                            ir::BlockArg::Value(b_block_params),
                            ir::BlockArg::Value(idx_obj),
                        ],
                    );

                    fb.switch_to_block(idx_obj_fail);
                    let iof_args = fb.block_params(idx_obj_fail)[0];
                    let iof_args_list = fb.block_params(idx_obj_fail)[1];
                    let iof_callable = fb.block_params(idx_obj_fail)[2];
                    let iof_block_params = fb.block_params(idx_obj_fail)[3];
                    fb.ins().call(decref_ref, &[iof_block_params]);
                    fb.ins().call(decref_ref, &[iof_callable]);
                    fb.ins().call(decref_ref, &[iof_args_list]);
                    fb.ins()
                        .jump(step_null_block, &[ir::BlockArg::Value(iof_args)]);

                    fb.switch_to_block(idx_obj_ok);
                    let ioo_args = fb.block_params(idx_obj_ok)[0];
                    let ioo_args_list = fb.block_params(idx_obj_ok)[1];
                    let ioo_callable = fb.block_params(idx_obj_ok)[2];
                    let ioo_block_params = fb.block_params(idx_obj_ok)[3];
                    let ioo_idx_obj = fb.block_params(idx_obj_ok)[4];
                    let param_obj_inst = fb.ins().call(
                        py_call_ref,
                        &[ioo_callable, ioo_args_list, ioo_idx_obj, null_ptr, null_ptr],
                    );
                    fb.ins().call(decref_ref, &[ioo_idx_obj]);
                    let param_obj = fb.inst_results(param_obj_inst)[0];
                    let param_obj_null =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, param_obj, null_ptr);
                    let param_obj_fail = fb.create_block();
                    fb.append_block_param(param_obj_fail, ptr_ty);
                    fb.append_block_param(param_obj_fail, ptr_ty);
                    fb.append_block_param(param_obj_fail, ptr_ty);
                    fb.append_block_param(param_obj_fail, ptr_ty);
                    let param_obj_ok = fb.create_block();
                    fb.append_block_param(param_obj_ok, ptr_ty);
                    fb.append_block_param(param_obj_ok, ptr_ty);
                    fb.append_block_param(param_obj_ok, ptr_ty);
                    fb.append_block_param(param_obj_ok, ptr_ty);
                    fb.append_block_param(param_obj_ok, ptr_ty);
                    fb.ins().brif(
                        param_obj_null,
                        param_obj_fail,
                        &[
                            ir::BlockArg::Value(ioo_args),
                            ir::BlockArg::Value(ioo_args_list),
                            ir::BlockArg::Value(ioo_callable),
                            ir::BlockArg::Value(ioo_block_params),
                        ],
                        param_obj_ok,
                        &[
                            ir::BlockArg::Value(ioo_args),
                            ir::BlockArg::Value(ioo_args_list),
                            ir::BlockArg::Value(ioo_callable),
                            ir::BlockArg::Value(ioo_block_params),
                            ir::BlockArg::Value(param_obj),
                        ],
                    );

                    fb.switch_to_block(param_obj_fail);
                    let pof_args = fb.block_params(param_obj_fail)[0];
                    let pof_args_list = fb.block_params(param_obj_fail)[1];
                    let pof_callable = fb.block_params(param_obj_fail)[2];
                    let pof_block_params = fb.block_params(param_obj_fail)[3];
                    fb.ins().call(decref_ref, &[pof_block_params]);
                    fb.ins().call(decref_ref, &[pof_callable]);
                    fb.ins().call(decref_ref, &[pof_args_list]);
                    fb.ins()
                        .jump(step_null_block, &[ir::BlockArg::Value(pof_args)]);

                    fb.switch_to_block(param_obj_ok);
                    let poo_args = fb.block_params(param_obj_ok)[0];
                    let poo_args_list = fb.block_params(param_obj_ok)[1];
                    let poo_callable = fb.block_params(param_obj_ok)[2];
                    let poo_block_params = fb.block_params(param_obj_ok)[3];
                    let poo_param_obj = fb.block_params(param_obj_ok)[4];
                    let slot_const = fb.ins().iconst(i64_ty, slot as i64);
                    let set_item_inst = fb.ins().call(
                        tuple_set_item_ref,
                        &[poo_block_params, slot_const, poo_param_obj],
                    );
                    let set_item_status = fb.inst_results(set_item_inst)[0];
                    let set_item_failed =
                        fb.ins()
                            .icmp_imm(ir::condcodes::IntCC::NotEqual, set_item_status, 0);
                    let set_item_fail = fb.create_block();
                    fb.append_block_param(set_item_fail, ptr_ty);
                    fb.append_block_param(set_item_fail, ptr_ty);
                    fb.append_block_param(set_item_fail, ptr_ty);
                    fb.append_block_param(set_item_fail, ptr_ty);
                    let next_build_block = fb.create_block();
                    fb.append_block_param(next_build_block, ptr_ty);
                    fb.append_block_param(next_build_block, ptr_ty);
                    fb.append_block_param(next_build_block, ptr_ty);
                    fb.append_block_param(next_build_block, ptr_ty);
                    fb.ins().brif(
                        set_item_failed,
                        set_item_fail,
                        &[
                            ir::BlockArg::Value(poo_args),
                            ir::BlockArg::Value(poo_args_list),
                            ir::BlockArg::Value(poo_callable),
                            ir::BlockArg::Value(poo_block_params),
                        ],
                        next_build_block,
                        &[
                            ir::BlockArg::Value(poo_args),
                            ir::BlockArg::Value(poo_args_list),
                            ir::BlockArg::Value(poo_callable),
                            ir::BlockArg::Value(poo_block_params),
                        ],
                    );

                    fb.switch_to_block(set_item_fail);
                    let sif_args = fb.block_params(set_item_fail)[0];
                    let sif_args_list = fb.block_params(set_item_fail)[1];
                    let sif_callable = fb.block_params(set_item_fail)[2];
                    let sif_block_params = fb.block_params(set_item_fail)[3];
                    fb.ins().call(decref_ref, &[sif_block_params]);
                    fb.ins().call(decref_ref, &[sif_callable]);
                    fb.ins().call(decref_ref, &[sif_args_list]);
                    fb.ins()
                        .jump(step_null_block, &[ir::BlockArg::Value(sif_args)]);

                    build_block = next_build_block;
                }

                fb.switch_to_block(build_block);
                let _bpd_args = fb.block_params(build_block)[0];
                let bpd_args_list = fb.block_params(build_block)[1];
                let bpd_callable = fb.block_params(build_block)[2];
                let bpd_block_params = fb.block_params(build_block)[3];
                fb.ins().call(decref_ref, &[bpd_callable]);
                fb.ins().call(decref_ref, &[bpd_args_list]);
                fb.ins().jump(
                    block_params_ready_block,
                    &[ir::BlockArg::Value(bpd_block_params)],
                );
            }

            fb.switch_to_block(block_params_ready_block);
            let bpo_block_params = fb.block_params(block_params_ready_block)[0];
            let term_inst = fb
                .ins()
                .call(py_call_object_ref, &[block_const, bpo_block_params]);
            let term = fb.inst_results(term_inst)[0];
            fb.ins().call(decref_ref, &[bpo_block_params]);
            let term_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, term, null_ptr);
            let term_ok_block = fb.create_block();
            fb.append_block_param(term_ok_block, ptr_ty);
            fb.append_block_param(term_ok_block, ptr_ty);
            let term_ok_args = [ir::BlockArg::Value(exec_args), ir::BlockArg::Value(term)];
            let term_null_args = [ir::BlockArg::Value(exec_args)];
            let term_null_block = fb.create_block();
            fb.append_block_param(term_null_block, ptr_ty);
            fb.ins().brif(
                term_null,
                term_null_block,
                &term_null_args,
                term_ok_block,
                &term_ok_args,
            );

            fb.switch_to_block(term_null_block);
            let tn_args = fb.block_params(term_null_block)[0];
            let raised_exc_inst = fb.ins().call(py_get_raised_exc_ref, &[]);
            let raised_exc = fb.inst_results(raised_exc_inst)[0];
            let raised_exc_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, raised_exc, null_ptr);
            let raised_exc_ok_block = fb.create_block();
            fb.append_block_param(raised_exc_ok_block, ptr_ty);
            fb.append_block_param(raised_exc_ok_block, ptr_ty);
            let raised_exc_err_block = fb.create_block();
            fb.append_block_param(raised_exc_err_block, ptr_ty);
            fb.ins().brif(
                raised_exc_null,
                raised_exc_err_block,
                &[ir::BlockArg::Value(tn_args)],
                raised_exc_ok_block,
                &[
                    ir::BlockArg::Value(tn_args),
                    ir::BlockArg::Value(raised_exc),
                ],
            );

            fb.switch_to_block(raised_exc_err_block);
            let ree_args = fb.block_params(raised_exc_err_block)[0];
            fb.ins()
                .jump(step_null_block, &[ir::BlockArg::Value(ree_args)]);

            fb.switch_to_block(raised_exc_ok_block);
            let reo_args = fb.block_params(raised_exc_ok_block)[0];
            let reo_exc = fb.block_params(raised_exc_ok_block)[1];
            if let Some(dispatch_plan) = exc_dispatch_plan {
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
                            let owner_index = dispatch_plan.owner_param_index.expect(
                                "missing owner param index for frame-local exception dispatch",
                            );
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
                let dispatch_jump_args = [ir::BlockArg::Value(bd_target_args)];
                fb.ins()
                    .jump(exec_blocks[dispatch_plan.target_index], &dispatch_jump_args);
            } else {
                fb.ins().jump(
                    raise_exc_direct_block,
                    &[ir::BlockArg::Value(reo_args), ir::BlockArg::Value(reo_exc)],
                );
            }

            fb.switch_to_block(term_ok_block);
            let tk_args = fb.block_params(term_ok_block)[0];
            let tk_term = fb.block_params(term_ok_block)[1];
            let kind_dispatch_args = [ir::BlockArg::Value(tk_args), ir::BlockArg::Value(tk_term)];
            fb.ins().jump(kind_dispatch_start, &kind_dispatch_args);

            fb.switch_to_block(kind_dispatch_start);
            let kd_args = fb.block_params(kind_dispatch_start)[0];
            let kd_term = fb.block_params(kind_dispatch_start)[1];
            let kind_inst = fb.ins().call(term_kind_ref, &[kd_term]);
            let kind_val = fb.inst_results(kind_inst)[0];
            let jump_kind = fb.ins().iconst(i64_ty, 0);
            let ret_kind = fb.ins().iconst(i64_ty, 1);
            let raise_kind = fb.ins().iconst(i64_ty, 2);

            let jump_case = fb.create_block();
            let after_jump_check = fb.create_block();
            fb.append_block_param(jump_case, ptr_ty);
            fb.append_block_param(jump_case, ptr_ty);
            fb.append_block_param(after_jump_check, ptr_ty);
            fb.append_block_param(after_jump_check, ptr_ty);
            let kd_pair = [ir::BlockArg::Value(kd_args), ir::BlockArg::Value(kd_term)];
            let is_jump = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, kind_val, jump_kind);
            fb.ins()
                .brif(is_jump, jump_case, &kd_pair, after_jump_check, &kd_pair);

            fb.switch_to_block(after_jump_check);
            let aj_args = fb.block_params(after_jump_check)[0];
            let aj_term = fb.block_params(after_jump_check)[1];
            let after_ret_check = fb.create_block();
            fb.append_block_param(after_ret_check, ptr_ty);
            fb.append_block_param(after_ret_check, ptr_ty);
            let aj_pair = [ir::BlockArg::Value(aj_args), ir::BlockArg::Value(aj_term)];
            let is_ret = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, kind_val, ret_kind);
            fb.ins()
                .brif(is_ret, ret_block, &aj_pair, after_ret_check, &aj_pair);

            fb.switch_to_block(after_ret_check);
            let ar_args = fb.block_params(after_ret_check)[0];
            let ar_term = fb.block_params(after_ret_check)[1];
            let ar_pair = [ir::BlockArg::Value(ar_args), ir::BlockArg::Value(ar_term)];
            let is_raise = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, kind_val, raise_kind);
            if let Some(dispatch_plan) = exc_dispatch_plan {
                let raise_dispatch_block = fb.create_block();
                fb.append_block_param(raise_dispatch_block, ptr_ty);
                fb.append_block_param(raise_dispatch_block, ptr_ty);
                fb.ins().brif(
                    is_raise,
                    raise_dispatch_block,
                    &ar_pair,
                    invalid_term_block,
                    &ar_pair,
                );

                fb.switch_to_block(raise_dispatch_block);
                let rd_args = fb.block_params(raise_dispatch_block)[0];
                let rd_term = fb.block_params(raise_dispatch_block)[1];
                let rd_exc_inst = fb.ins().call(term_raise_exc_ref, &[rd_term]);
                let rd_exc = fb.inst_results(rd_exc_inst)[0];
                let rd_exc_null = fb.ins().icmp(ir::condcodes::IntCC::Equal, rd_exc, null_ptr);
                let raise_dispatch_have_exc = fb.create_block();
                fb.append_block_param(raise_dispatch_have_exc, ptr_ty);
                fb.append_block_param(raise_dispatch_have_exc, ptr_ty);
                fb.append_block_param(raise_dispatch_have_exc, ptr_ty);
                let raise_dispatch_attr_fail = fb.create_block();
                fb.append_block_param(raise_dispatch_attr_fail, ptr_ty);
                fb.append_block_param(raise_dispatch_attr_fail, ptr_ty);
                let rd_pair = [ir::BlockArg::Value(rd_args), ir::BlockArg::Value(rd_term)];
                let rd_triplet = [
                    ir::BlockArg::Value(rd_args),
                    ir::BlockArg::Value(rd_term),
                    ir::BlockArg::Value(rd_exc),
                ];
                fb.ins().brif(
                    rd_exc_null,
                    raise_dispatch_attr_fail,
                    &rd_pair,
                    raise_dispatch_have_exc,
                    &rd_triplet,
                );

                fb.switch_to_block(raise_dispatch_attr_fail);
                let raf_args = fb.block_params(raise_dispatch_attr_fail)[0];
                let raf_term = fb.block_params(raise_dispatch_attr_fail)[1];
                let raf_pair = [ir::BlockArg::Value(raf_args), ir::BlockArg::Value(raf_term)];
                fb.ins().jump(raise_block, &raf_pair);

                fb.switch_to_block(raise_dispatch_have_exc);
                let rhe_args = fb.block_params(raise_dispatch_have_exc)[0];
                let rhe_term = fb.block_params(raise_dispatch_have_exc)[1];
                let rhe_exc = fb.block_params(raise_dispatch_have_exc)[2];
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
                fb.append_block_param(dispatch_alloc_fail, ptr_ty);
                let dispatch_build_start = fb.create_block();
                fb.append_block_param(dispatch_build_start, ptr_ty);
                fb.append_block_param(dispatch_build_start, ptr_ty);
                fb.append_block_param(dispatch_build_start, ptr_ty);
                fb.append_block_param(dispatch_build_start, ptr_ty);
                fb.ins().brif(
                    target_args_null,
                    dispatch_alloc_fail,
                    &[
                        ir::BlockArg::Value(rhe_args),
                        ir::BlockArg::Value(rhe_term),
                        ir::BlockArg::Value(rhe_exc),
                    ],
                    dispatch_build_start,
                    &[
                        ir::BlockArg::Value(rhe_args),
                        ir::BlockArg::Value(rhe_term),
                        ir::BlockArg::Value(rhe_exc),
                        ir::BlockArg::Value(target_args),
                    ],
                );

                fb.switch_to_block(dispatch_alloc_fail);
                let daf_args = fb.block_params(dispatch_alloc_fail)[0];
                let daf_term = fb.block_params(dispatch_alloc_fail)[1];
                let daf_exc = fb.block_params(dispatch_alloc_fail)[2];
                fb.ins().call(decref_ref, &[daf_exc]);
                fb.ins().call(decref_ref, &[daf_term]);
                fb.ins()
                    .jump(step_null_block, &[ir::BlockArg::Value(daf_args)]);

                let mut build_block = dispatch_build_start;
                for (slot, source) in dispatch_plan.arg_sources.iter().enumerate() {
                    fb.switch_to_block(build_block);
                    let b_args = fb.block_params(build_block)[0];
                    let b_term = fb.block_params(build_block)[1];
                    let b_exc = fb.block_params(build_block)[2];
                    let b_target_args = fb.block_params(build_block)[3];
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
                            let owner_index = dispatch_plan.owner_param_index.expect(
                                "missing owner param index for frame-local exception dispatch",
                            );
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
                    fb.append_block_param(value_fail, ptr_ty);
                    let value_ok = fb.create_block();
                    fb.append_block_param(value_ok, ptr_ty);
                    fb.append_block_param(value_ok, ptr_ty);
                    fb.append_block_param(value_ok, ptr_ty);
                    fb.append_block_param(value_ok, ptr_ty);
                    fb.append_block_param(value_ok, ptr_ty);
                    fb.ins().brif(
                        value_null,
                        value_fail,
                        &[
                            ir::BlockArg::Value(b_args),
                            ir::BlockArg::Value(b_term),
                            ir::BlockArg::Value(b_exc),
                            ir::BlockArg::Value(b_target_args),
                        ],
                        value_ok,
                        &[
                            ir::BlockArg::Value(b_args),
                            ir::BlockArg::Value(b_term),
                            ir::BlockArg::Value(b_exc),
                            ir::BlockArg::Value(b_target_args),
                            ir::BlockArg::Value(value),
                        ],
                    );

                    fb.switch_to_block(value_fail);
                    let vf_args = fb.block_params(value_fail)[0];
                    let vf_term = fb.block_params(value_fail)[1];
                    let vf_exc = fb.block_params(value_fail)[2];
                    let vf_target_args = fb.block_params(value_fail)[3];
                    fb.ins().call(decref_ref, &[vf_target_args]);
                    fb.ins().call(decref_ref, &[vf_exc]);
                    fb.ins().call(decref_ref, &[vf_term]);
                    fb.ins()
                        .jump(step_null_block, &[ir::BlockArg::Value(vf_args)]);

                    fb.switch_to_block(value_ok);
                    let vo_args = fb.block_params(value_ok)[0];
                    let vo_term = fb.block_params(value_ok)[1];
                    let vo_exc = fb.block_params(value_ok)[2];
                    let vo_target_args = fb.block_params(value_ok)[3];
                    let vo_value = fb.block_params(value_ok)[4];
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
                    fb.append_block_param(set_item_fail, ptr_ty);
                    let next_build_block = fb.create_block();
                    fb.append_block_param(next_build_block, ptr_ty);
                    fb.append_block_param(next_build_block, ptr_ty);
                    fb.append_block_param(next_build_block, ptr_ty);
                    fb.append_block_param(next_build_block, ptr_ty);
                    fb.ins().brif(
                        set_item_failed,
                        set_item_fail,
                        &[
                            ir::BlockArg::Value(vo_args),
                            ir::BlockArg::Value(vo_term),
                            ir::BlockArg::Value(vo_exc),
                            ir::BlockArg::Value(vo_target_args),
                        ],
                        next_build_block,
                        &[
                            ir::BlockArg::Value(vo_args),
                            ir::BlockArg::Value(vo_term),
                            ir::BlockArg::Value(vo_exc),
                            ir::BlockArg::Value(vo_target_args),
                        ],
                    );

                    fb.switch_to_block(set_item_fail);
                    let sf_args = fb.block_params(set_item_fail)[0];
                    let sf_term = fb.block_params(set_item_fail)[1];
                    let sf_exc = fb.block_params(set_item_fail)[2];
                    let sf_target_args = fb.block_params(set_item_fail)[3];
                    fb.ins().call(decref_ref, &[sf_target_args]);
                    fb.ins().call(decref_ref, &[sf_exc]);
                    fb.ins().call(decref_ref, &[sf_term]);
                    fb.ins()
                        .jump(step_null_block, &[ir::BlockArg::Value(sf_args)]);

                    build_block = next_build_block;
                }

                fb.switch_to_block(build_block);
                let bd_args = fb.block_params(build_block)[0];
                let bd_term = fb.block_params(build_block)[1];
                let bd_exc = fb.block_params(build_block)[2];
                let bd_target_args = fb.block_params(build_block)[3];
                fb.ins().call(decref_ref, &[bd_exc]);
                fb.ins().call(decref_ref, &[bd_term]);
                fb.ins().call(decref_ref, &[bd_args]);
                let dispatch_jump_args = [ir::BlockArg::Value(bd_target_args)];
                fb.ins()
                    .jump(exec_blocks[dispatch_plan.target_index], &dispatch_jump_args);
            } else {
                fb.ins().brif(
                    is_raise,
                    raise_block,
                    &ar_pair,
                    invalid_term_block,
                    &ar_pair,
                );
            }

            fb.switch_to_block(jump_case);
            let jump_args = fb.block_params(jump_case)[0];
            let jump_term = fb.block_params(jump_case)[1];
            let target_inst = fb.ins().call(term_jump_target_ref, &[jump_term]);
            let target_val = fb.inst_results(target_inst)[0];
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            let target_null = fb
                .ins()
                .icmp(ir::condcodes::IntCC::Equal, target_val, null_ptr);
            let target_ok_block = fb.create_block();
            fb.append_block_param(target_ok_block, ptr_ty);
            fb.append_block_param(target_ok_block, ptr_ty);
            fb.append_block_param(target_ok_block, ptr_ty);
            let target_ok_args = [
                ir::BlockArg::Value(jump_args),
                ir::BlockArg::Value(jump_term),
                ir::BlockArg::Value(target_val),
            ];
            let term_error_args = [
                ir::BlockArg::Value(jump_args),
                ir::BlockArg::Value(jump_term),
            ];
            fb.ins().brif(
                target_null,
                invalid_term_block,
                &term_error_args,
                target_ok_block,
                &target_ok_args,
            );

            fb.switch_to_block(target_ok_block);
            let jto_args = fb.block_params(target_ok_block)[0];
            let jto_term = fb.block_params(target_ok_block)[1];
            let jto_target = fb.block_params(target_ok_block)[2];
            let next_args_inst = fb.ins().call(term_jump_args_ref, &[jto_term]);
            let next_args_val = fb.inst_results(next_args_inst)[0];
            let next_args_null =
                fb.ins()
                    .icmp(ir::condcodes::IntCC::Equal, next_args_val, null_ptr);
            let jump_invalid_null_args = [
                ir::BlockArg::Value(jto_args),
                ir::BlockArg::Value(jto_term),
                ir::BlockArg::Value(jto_target),
            ];
            let jump_valid_args = [
                ir::BlockArg::Value(jto_args),
                ir::BlockArg::Value(jto_term),
                ir::BlockArg::Value(next_args_val),
                ir::BlockArg::Value(jto_target),
            ];
            let jump_valid_dispatch = fb.create_block();
            fb.append_block_param(jump_valid_dispatch, ptr_ty);
            fb.append_block_param(jump_valid_dispatch, ptr_ty);
            fb.append_block_param(jump_valid_dispatch, ptr_ty);
            fb.append_block_param(jump_valid_dispatch, ptr_ty);
            fb.ins().brif(
                next_args_null,
                invalid_jump_null_block,
                &jump_invalid_null_args,
                jump_valid_dispatch,
                &jump_valid_args,
            );

            fb.switch_to_block(jump_valid_dispatch);
            let jv_args = fb.block_params(jump_valid_dispatch)[0];
            let jv_term = fb.block_params(jump_valid_dispatch)[1];
            let jv_next_args = fb.block_params(jump_valid_dispatch)[2];
            let jv_target = fb.block_params(jump_valid_dispatch)[3];
            // term_jump_target helper returns a new reference. We only need pointer
            // identity for dispatch comparisons, so release ownership before branching.
            fb.ins().call(decref_ref, &[jv_target]);
            match &plan.block_terms[index] {
                BlockTermPlan::Jump { target_index } => {
                    let target_const = fb.ins().iconst(ptr_ty, blocks[*target_index] as i64);
                    let target_match =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, jv_target, target_const);
                    let target_block = fb.create_block();
                    let fallback_block = fb.create_block();
                    fb.append_block_param(target_block, ptr_ty);
                    fb.append_block_param(target_block, ptr_ty);
                    fb.append_block_param(target_block, ptr_ty);
                    fb.append_block_param(fallback_block, ptr_ty);
                    fb.append_block_param(fallback_block, ptr_ty);
                    fb.append_block_param(fallback_block, ptr_ty);
                    let block_args = [
                        ir::BlockArg::Value(jv_args),
                        ir::BlockArg::Value(jv_term),
                        ir::BlockArg::Value(jv_next_args),
                    ];
                    fb.ins().brif(
                        target_match,
                        target_block,
                        &block_args,
                        fallback_block,
                        &block_args,
                    );

                    fb.switch_to_block(target_block);
                    let ok_args = fb.block_params(target_block)[0];
                    let ok_term = fb.block_params(target_block)[1];
                    let ok_next_args = fb.block_params(target_block)[2];
                    fb.ins().call(decref_ref, &[ok_term]);
                    fb.ins().call(decref_ref, &[ok_args]);
                    let next = [ir::BlockArg::Value(ok_next_args)];
                    fb.ins().jump(exec_blocks[*target_index], &next);

                    fb.switch_to_block(fallback_block);
                    let fb_args = fb.block_params(fallback_block)[0];
                    let fb_term = fb.block_params(fallback_block)[1];
                    let fb_next_args = fb.block_params(fallback_block)[2];
                    if let Some(exc_index) = exc_target_index {
                        let exc_const = fb.ins().iconst(ptr_ty, blocks[exc_index] as i64);
                        let exc_match =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, jv_target, exc_const);
                        let exc_block = fb.create_block();
                        fb.append_block_param(exc_block, ptr_ty);
                        fb.append_block_param(exc_block, ptr_ty);
                        fb.append_block_param(exc_block, ptr_ty);
                        let fallback_args = [
                            ir::BlockArg::Value(fb_args),
                            ir::BlockArg::Value(fb_term),
                            ir::BlockArg::Value(fb_next_args),
                        ];
                        fb.ins().brif(
                            exc_match,
                            exc_block,
                            &fallback_args,
                            jump_invalid_target_block,
                            &fallback_args,
                        );
                        fb.switch_to_block(exc_block);
                        let ex_args = fb.block_params(exc_block)[0];
                        let ex_term = fb.block_params(exc_block)[1];
                        let ex_next_args = fb.block_params(exc_block)[2];
                        fb.ins().call(decref_ref, &[ex_term]);
                        fb.ins().call(decref_ref, &[ex_args]);
                        let next = [ir::BlockArg::Value(ex_next_args)];
                        fb.ins().jump(exec_blocks[exc_index], &next);
                    } else {
                        let invalid_args = [
                            ir::BlockArg::Value(fb_args),
                            ir::BlockArg::Value(fb_term),
                            ir::BlockArg::Value(fb_next_args),
                        ];
                        fb.ins().jump(jump_invalid_target_block, &invalid_args);
                    }
                }
                BlockTermPlan::BrIf {
                    then_index,
                    else_index,
                } => {
                    let then_const = fb.ins().iconst(ptr_ty, blocks[*then_index] as i64);
                    let else_const = fb.ins().iconst(ptr_ty, blocks[*else_index] as i64);
                    let then_match =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, jv_target, then_const);
                    let then_block = fb.create_block();
                    let else_check = fb.create_block();
                    fb.append_block_param(then_block, ptr_ty);
                    fb.append_block_param(then_block, ptr_ty);
                    fb.append_block_param(then_block, ptr_ty);
                    fb.append_block_param(else_check, ptr_ty);
                    fb.append_block_param(else_check, ptr_ty);
                    fb.append_block_param(else_check, ptr_ty);
                    let branch_args = [
                        ir::BlockArg::Value(jv_args),
                        ir::BlockArg::Value(jv_term),
                        ir::BlockArg::Value(jv_next_args),
                    ];
                    fb.ins().brif(
                        then_match,
                        then_block,
                        &branch_args,
                        else_check,
                        &branch_args,
                    );

                    fb.switch_to_block(then_block);
                    let tb_args = fb.block_params(then_block)[0];
                    let tb_term = fb.block_params(then_block)[1];
                    let tb_next_args = fb.block_params(then_block)[2];
                    fb.ins().call(decref_ref, &[tb_term]);
                    fb.ins().call(decref_ref, &[tb_args]);
                    let then_jump = [ir::BlockArg::Value(tb_next_args)];
                    fb.ins().jump(exec_blocks[*then_index], &then_jump);

                    fb.switch_to_block(else_check);
                    let ec_args = fb.block_params(else_check)[0];
                    let ec_term = fb.block_params(else_check)[1];
                    let ec_next_args = fb.block_params(else_check)[2];
                    let else_match =
                        fb.ins()
                            .icmp(ir::condcodes::IntCC::Equal, jv_target, else_const);
                    let else_block = fb.create_block();
                    fb.append_block_param(else_block, ptr_ty);
                    fb.append_block_param(else_block, ptr_ty);
                    fb.append_block_param(else_block, ptr_ty);
                    let else_args = [
                        ir::BlockArg::Value(ec_args),
                        ir::BlockArg::Value(ec_term),
                        ir::BlockArg::Value(ec_next_args),
                    ];
                    if let Some(exc_index) = exc_target_index {
                        let exc_check = fb.create_block();
                        fb.append_block_param(exc_check, ptr_ty);
                        fb.append_block_param(exc_check, ptr_ty);
                        fb.append_block_param(exc_check, ptr_ty);
                        fb.ins()
                            .brif(else_match, else_block, &else_args, exc_check, &else_args);

                        fb.switch_to_block(exc_check);
                        let xc_args = fb.block_params(exc_check)[0];
                        let xc_term = fb.block_params(exc_check)[1];
                        let xc_next_args = fb.block_params(exc_check)[2];
                        let exc_const = fb.ins().iconst(ptr_ty, blocks[exc_index] as i64);
                        let exc_match =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, jv_target, exc_const);
                        let exc_block = fb.create_block();
                        fb.append_block_param(exc_block, ptr_ty);
                        fb.append_block_param(exc_block, ptr_ty);
                        fb.append_block_param(exc_block, ptr_ty);
                        let exc_args = [
                            ir::BlockArg::Value(xc_args),
                            ir::BlockArg::Value(xc_term),
                            ir::BlockArg::Value(xc_next_args),
                        ];
                        fb.ins().brif(
                            exc_match,
                            exc_block,
                            &exc_args,
                            jump_invalid_target_block,
                            &exc_args,
                        );

                        fb.switch_to_block(exc_block);
                        let xb_args = fb.block_params(exc_block)[0];
                        let xb_term = fb.block_params(exc_block)[1];
                        let xb_next_args = fb.block_params(exc_block)[2];
                        fb.ins().call(decref_ref, &[xb_term]);
                        fb.ins().call(decref_ref, &[xb_args]);
                        let exc_jump = [ir::BlockArg::Value(xb_next_args)];
                        fb.ins().jump(exec_blocks[exc_index], &exc_jump);
                    } else {
                        fb.ins().brif(
                            else_match,
                            else_block,
                            &else_args,
                            jump_invalid_target_block,
                            &else_args,
                        );
                    }

                    fb.switch_to_block(else_block);
                    let eb_args = fb.block_params(else_block)[0];
                    let eb_term = fb.block_params(else_block)[1];
                    let eb_next_args = fb.block_params(else_block)[2];
                    fb.ins().call(decref_ref, &[eb_term]);
                    fb.ins().call(decref_ref, &[eb_args]);
                    let else_jump = [ir::BlockArg::Value(eb_next_args)];
                    fb.ins().jump(exec_blocks[*else_index], &else_jump);
                }
                BlockTermPlan::BrTable {
                    targets,
                    default_index,
                } => {
                    let mut all_targets = targets.clone();
                    all_targets.push(*default_index);
                    if let Some(exc_index) = exc_target_index {
                        if !all_targets.contains(&exc_index) {
                            all_targets.push(exc_index);
                        }
                    }
                    let mut check_blocks = Vec::with_capacity(all_targets.len());
                    for _ in 0..all_targets.len() {
                        let check = fb.create_block();
                        fb.append_block_param(check, ptr_ty); // args
                        fb.append_block_param(check, ptr_ty); // term
                        fb.append_block_param(check, ptr_ty); // next_args
                        fb.append_block_param(check, ptr_ty); // target
                        check_blocks.push(check);
                    }
                    let first_check_args = [
                        ir::BlockArg::Value(jv_args),
                        ir::BlockArg::Value(jv_term),
                        ir::BlockArg::Value(jv_next_args),
                        ir::BlockArg::Value(jv_target),
                    ];
                    fb.ins().jump(check_blocks[0], &first_check_args);

                    for (pos, target_index) in all_targets.iter().enumerate() {
                        let check = check_blocks[pos];
                        fb.switch_to_block(check);
                        let cb_args = fb.block_params(check)[0];
                        let cb_term = fb.block_params(check)[1];
                        let cb_next_args = fb.block_params(check)[2];
                        let cb_target = fb.block_params(check)[3];
                        let target_const = fb.ins().iconst(ptr_ty, blocks[*target_index] as i64);
                        let matches =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, cb_target, target_const);
                        let match_block = fb.create_block();
                        fb.append_block_param(match_block, ptr_ty);
                        fb.append_block_param(match_block, ptr_ty);
                        fb.append_block_param(match_block, ptr_ty);
                        let branch_args = [
                            ir::BlockArg::Value(cb_args),
                            ir::BlockArg::Value(cb_term),
                            ir::BlockArg::Value(cb_next_args),
                        ];
                        if pos + 1 < check_blocks.len() {
                            let miss_check = check_blocks[pos + 1];
                            let miss_args = [
                                ir::BlockArg::Value(cb_args),
                                ir::BlockArg::Value(cb_term),
                                ir::BlockArg::Value(cb_next_args),
                                ir::BlockArg::Value(cb_target),
                            ];
                            fb.ins().brif(
                                matches,
                                match_block,
                                &branch_args,
                                miss_check,
                                &miss_args,
                            );
                        } else {
                            fb.ins().brif(
                                matches,
                                match_block,
                                &branch_args,
                                jump_invalid_target_block,
                                &branch_args,
                            );
                        }

                        fb.switch_to_block(match_block);
                        let mb_args = fb.block_params(match_block)[0];
                        let mb_term = fb.block_params(match_block)[1];
                        let mb_next_args = fb.block_params(match_block)[2];
                        fb.ins().call(decref_ref, &[mb_term]);
                        fb.ins().call(decref_ref, &[mb_args]);
                        let jump_args = [ir::BlockArg::Value(mb_next_args)];
                        fb.ins().jump(exec_blocks[*target_index], &jump_args);
                    }
                }
                BlockTermPlan::Raise | BlockTermPlan::Ret => {
                    if let Some(exc_index) = exc_target_index {
                        let exc_const = fb.ins().iconst(ptr_ty, blocks[exc_index] as i64);
                        let exc_match =
                            fb.ins()
                                .icmp(ir::condcodes::IntCC::Equal, jv_target, exc_const);
                        let exc_block = fb.create_block();
                        fb.append_block_param(exc_block, ptr_ty);
                        fb.append_block_param(exc_block, ptr_ty);
                        fb.append_block_param(exc_block, ptr_ty);
                        let branch_args = [
                            ir::BlockArg::Value(jv_args),
                            ir::BlockArg::Value(jv_term),
                            ir::BlockArg::Value(jv_next_args),
                        ];
                        fb.ins().brif(
                            exc_match,
                            exc_block,
                            &branch_args,
                            jump_invalid_target_block,
                            &branch_args,
                        );
                        fb.switch_to_block(exc_block);
                        let ex_args = fb.block_params(exc_block)[0];
                        let ex_term = fb.block_params(exc_block)[1];
                        let ex_next_args = fb.block_params(exc_block)[2];
                        fb.ins().call(decref_ref, &[ex_term]);
                        fb.ins().call(decref_ref, &[ex_args]);
                        let next = [ir::BlockArg::Value(ex_next_args)];
                        fb.ins().jump(exec_blocks[exc_index], &next);
                    } else {
                        let invalid_args =
                            [ir::BlockArg::Value(jv_args), ir::BlockArg::Value(jv_term)];
                        fb.ins().jump(invalid_term_block, &invalid_args);
                    }
                }
            }
        }

        if !has_generic_blocks {
            fb.switch_to_block(step_null_block);
            let step_null_args = fb.block_params(step_null_block)[0];
            fb.ins().call(decref_ref, &[step_null_args]);
            let null_ptr = fb.ins().iconst(ptr_ty, 0);
            fb.ins().return_(&[null_ptr]);
            fb.seal_all_blocks();
            fb.finalize();
            return Ok((ctx, main_id, literal_pool, import_id_to_symbol));
        }

        fb.switch_to_block(step_null_block);
        let step_null_args = fb.block_params(step_null_block)[0];
        fb.ins().call(decref_ref, &[step_null_args]);
        let null_ptr = fb.ins().iconst(ptr_ty, 0);
        fb.ins().return_(&[null_ptr]);

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

        fb.switch_to_block(invalid_jump_null_block);
        let jn_args = fb.block_params(invalid_jump_null_block)[0];
        let jn_term = fb.block_params(invalid_jump_null_block)[1];
        let jn_target = fb.block_params(invalid_jump_null_block)[2];
        let _ = fb.ins().call(term_invalid_ref, &[jn_term]);
        fb.ins().call(decref_ref, &[jn_target]);
        fb.ins().call(decref_ref, &[jn_term]);
        fb.ins().call(decref_ref, &[jn_args]);
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
        let raise_null = fb.ins().iconst(ptr_ty, 0);
        let exc_inst = fb.ins().call(term_raise_exc_ref, &[raise_term]);
        let exc_value = fb.inst_results(exc_inst)[0];
        let exc_null = fb
            .ins()
            .icmp(ir::condcodes::IntCC::Equal, exc_value, raise_null);
        let raise_set_block = fb.create_block();
        fb.append_block_param(raise_set_block, ptr_ty);
        let raise_exc_error_block = fb.create_block();
        fb.ins().brif(
            exc_null,
            raise_exc_error_block,
            &[],
            raise_set_block,
            &[ir::BlockArg::Value(exc_value)],
        );
        fb.switch_to_block(raise_set_block);
        let raise_exc = fb.block_params(raise_set_block)[0];
        let _ = fb.ins().call(raise_exc_ref, &[raise_exc]);
        fb.ins().call(decref_ref, &[raise_exc]);
        fb.ins().jump(raise_exc_error_block, &[]);
        fb.switch_to_block(raise_exc_error_block);
        fb.ins().call(decref_ref, &[raise_term]);
        fb.ins().call(decref_ref, &[raise_args]);
        fb.ins().return_(&[raise_null]);

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

    Ok((ctx, main_id, literal_pool, import_id_to_symbol))
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
    builder.symbol("dp_jit_term_kind", dp_jit_term_kind as *const u8);
    builder.symbol(
        "dp_jit_term_jump_target",
        dp_jit_term_jump_target as *const u8,
    );
    builder.symbol("dp_jit_term_jump_args", dp_jit_term_jump_args as *const u8);
    builder.symbol("dp_jit_term_ret_value", dp_jit_term_ret_value as *const u8);
    builder.symbol("dp_jit_term_raise_exc", dp_jit_term_raise_exc as *const u8);
    builder.symbol("dp_jit_raise_from_exc", dp_jit_raise_from_exc as *const u8);
    builder.symbol("dp_jit_term_invalid", dp_jit_term_invalid as *const u8);
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
    out.push_str("; dp_jit_term_kind\n");
    out.push_str("; dp_jit_term_jump_target\n");
    out.push_str("; dp_jit_term_jump_args\n");
    out.push_str("; dp_jit_term_ret_value\n");
    out.push_str("; dp_jit_term_raise_exc\n");
    out.push_str("; dp_jit_raise_from_exc\n");
    out.push_str("; dp_jit_term_invalid\n");
    out.push('\n');
    let rendered_clif = ctx.func.display().to_string();
    out.push_str(&rewrite_import_fn_aliases(
        rendered_clif.as_str(),
        &import_id_to_symbol,
    ));
    let cfg_dot = CFGPrinter::new(&ctx.func).to_string();
    Ok(RenderedSpecializedClif { clif: out, cfg_dot })
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
    term_kind_fn: TermKindFn,
    term_jump_target_fn: TermJumpTargetFn,
    term_jump_args_fn: TermJumpArgsFn,
    term_ret_value_fn: TermRetValueFn,
    term_raise_exc_fn: TermRaiseExcFn,
    raise_from_exc_fn: RaiseFromExcFn,
    term_invalid_fn: TermInvalidFn,
    none_obj: ObjPtr,
    empty_tuple_obj: ObjPtr,
) -> Result<ObjPtr, String> {
    if args.is_null() {
        return Err("invalid null args passed to specialized JIT run_bb".to_string());
    }
    if globals_obj.is_null() {
        return Err("invalid null globals object passed to specialized JIT run_bb".to_string());
    }

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
    DP_JIT_TERM_KIND_FN = Some(term_kind_fn);
    DP_JIT_TERM_JUMP_TARGET_FN = Some(term_jump_target_fn);
    DP_JIT_TERM_JUMP_ARGS_FN = Some(term_jump_args_fn);
    DP_JIT_TERM_RET_VALUE_FN = Some(term_ret_value_fn);
    DP_JIT_TERM_RAISE_EXC_FN = Some(term_raise_exc_fn);
    DP_JIT_RAISE_FROM_EXC_FN = Some(raise_from_exc_fn);
    DP_JIT_TERM_INVALID_FN = Some(term_invalid_fn);

    let mut builder = new_jit_builder()?;
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
    builder.symbol("dp_jit_term_kind", dp_jit_term_kind as *const u8);
    builder.symbol(
        "dp_jit_term_jump_target",
        dp_jit_term_jump_target as *const u8,
    );
    builder.symbol("dp_jit_term_jump_args", dp_jit_term_jump_args as *const u8);
    builder.symbol("dp_jit_term_ret_value", dp_jit_term_ret_value as *const u8);
    builder.symbol("dp_jit_term_raise_exc", dp_jit_term_raise_exc as *const u8);
    builder.symbol("dp_jit_raise_from_exc", dp_jit_raise_from_exc as *const u8);
    builder.symbol("dp_jit_term_invalid", dp_jit_term_invalid as *const u8);
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

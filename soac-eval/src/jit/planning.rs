use dp_transform::block_py::{
    AbruptKind, BbBlock, BbStmt, BlockArg, BlockPyFunction, BlockPyLabel, BlockPyModule,
    BlockPyTerm, CoreBlockPyCallArg, CoreBlockPyExprWithoutAwaitOrYield, CoreBlockPyKeywordArg,
    CoreBlockPyLiteral, CoreNumberLiteralValue,
};
use dp_transform::passes::BbBlockPyPass;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct ClifPlan {
    pub entry_index: usize,
    pub block_labels: Vec<String>,
    pub ambient_param_names: Vec<String>,
    pub block_param_names: Vec<Vec<String>>,
    pub block_terms: Vec<BlockTermPlan>,
    pub block_exc_targets: Vec<Option<usize>>,
    pub block_exc_dispatches: Vec<Option<BlockExcDispatchPlan>>,
    pub block_fast_paths: Vec<BlockFastPath>,
}

#[derive(Clone, Debug)]
pub struct BlockExcDispatchPlan {
    pub target_index: usize,
    pub owner_param_name: Option<String>,
    pub arg_sources: Vec<BlockExcArgSource>,
}

#[derive(Clone, Debug)]
pub enum BlockExcArgSource {
    SourceParam { name: String },
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
pub enum DirectSimpleBlockArgPlan {
    Name(String),
    Expr(DirectSimpleExprPlan),
    None,
    CurrentException,
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
        target_args: Vec<DirectSimpleBlockArgPlan>,
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

type PlanRegistry = HashMap<PlanKey, ClifPlan>;
type FunctionRegistry = HashMap<PlanKey, BlockPyFunction<BbBlockPyPass>>;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PlanKey {
    pub module: String,
    pub function_id: usize,
}

static CLIF_PLAN_REGISTRY: OnceLock<Mutex<PlanRegistry>> = OnceLock::new();
static BB_FUNCTION_REGISTRY: OnceLock<Mutex<FunctionRegistry>> = OnceLock::new();

fn clif_plan_registry() -> &'static Mutex<PlanRegistry> {
    CLIF_PLAN_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bb_function_registry() -> &'static Mutex<FunctionRegistry> {
    BB_FUNCTION_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn direct_simple_expr_from(
    expr: &CoreBlockPyExprWithoutAwaitOrYield,
) -> Option<DirectSimpleExprPlan> {
    match expr {
        CoreBlockPyExprWithoutAwaitOrYield::Name(name) => {
            Some(DirectSimpleExprPlan::Name(name.id.to_string()))
        }
        CoreBlockPyExprWithoutAwaitOrYield::Literal(literal) => match literal {
            CoreBlockPyLiteral::NumberLiteral(number) => match &number.value {
                CoreNumberLiteralValue::Int(value) => value.as_i64().map(DirectSimpleExprPlan::Int),
                CoreNumberLiteralValue::Float(value) => Some(DirectSimpleExprPlan::Float(*value)),
            },
            CoreBlockPyLiteral::BytesLiteral(bytes) => {
                Some(DirectSimpleExprPlan::Bytes(bytes.value.clone()))
            }
            CoreBlockPyLiteral::StringLiteral(_) => None,
        },
        CoreBlockPyExprWithoutAwaitOrYield::Call(call) => {
            let func = direct_simple_expr_from(call.func.as_ref())?;
            let mut parts = Vec::with_capacity(call.args.len() + call.keywords.len());
            for arg in &call.args {
                match arg {
                    CoreBlockPyCallArg::Positional(arg) => {
                        parts.push(DirectSimpleCallPart::Pos(direct_simple_expr_from(arg)?));
                    }
                    CoreBlockPyCallArg::Starred(arg) => {
                        parts.push(DirectSimpleCallPart::Star(direct_simple_expr_from(arg)?));
                    }
                }
            }
            for keyword in &call.keywords {
                match keyword {
                    CoreBlockPyKeywordArg::Named { arg: name, value } => {
                        parts.push(DirectSimpleCallPart::Kw {
                            name: name.to_string(),
                            value: direct_simple_expr_from(value)?,
                        });
                    }
                    CoreBlockPyKeywordArg::Starred(value) => {
                        parts.push(DirectSimpleCallPart::KwStar(direct_simple_expr_from(
                            value,
                        )?));
                    }
                }
            }
            Some(DirectSimpleExprPlan::Call {
                func: Box::new(func),
                parts,
            })
        }
    }
}

fn abrupt_kind_tag(kind: AbruptKind) -> i64 {
    match kind {
        AbruptKind::Fallthrough => 0,
        AbruptKind::Return => 1,
        AbruptKind::Exception => 2,
        AbruptKind::Break => 3,
        AbruptKind::Continue => 4,
    }
}

fn direct_simple_block_arg_from(
    arg: &BlockArg<CoreBlockPyExprWithoutAwaitOrYield>,
) -> Option<DirectSimpleBlockArgPlan> {
    match arg {
        BlockArg::Name(name) => Some(DirectSimpleBlockArgPlan::Name(name.clone())),
        BlockArg::Expr(expr) => Some(DirectSimpleBlockArgPlan::Expr(direct_simple_expr_from(
            expr,
        )?)),
        BlockArg::None => Some(DirectSimpleBlockArgPlan::None),
        BlockArg::CurrentException => Some(DirectSimpleBlockArgPlan::CurrentException),
        BlockArg::AbruptKind(kind) => Some(DirectSimpleBlockArgPlan::Expr(
            DirectSimpleExprPlan::Int(abrupt_kind_tag(*kind)),
        )),
    }
}

fn direct_simple_plan_from_block(block: &BbBlock) -> Option<DirectSimpleRetPlan> {
    let mut known_names = block.param_name_vec();
    let mut assigns = Vec::new();
    for op in &block.body {
        let BbStmt::Assign(assign) = op else {
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
    let BlockPyTerm::Return(ret_value) = &block.term else {
        return None;
    };
    let ret = if let Some(value) = ret_value.as_ref() {
        direct_simple_expr_from(value)?
    } else {
        DirectSimpleExprPlan::None
    };
    Some(DirectSimpleRetPlan {
        params: block.param_name_vec(),
        assigns,
        ret,
    })
}

fn ambient_param_name_set(function: &BlockPyFunction<BbBlockPyPass>) -> HashSet<String> {
    function
        .closure_layout()
        .as_ref()
        .map(|layout| layout.ambient_storage_names().into_iter().collect())
        .unwrap_or_default()
}

fn jit_param_names_for_block(
    block: &BbBlock,
    ambient_param_names: &HashSet<String>,
) -> Vec<String> {
    block
        .param_names()
        .filter(|name| !ambient_param_names.contains(*name))
        .map(ToString::to_string)
        .collect()
}

fn direct_simple_brif_plan_from_block(
    function: &BlockPyFunction<BbBlockPyPass>,
    block: &BbBlock,
    label_to_index: &HashMap<BlockPyLabel, usize>,
    ambient_param_names: &HashSet<String>,
) -> Option<DirectSimpleBrIfPlan> {
    if !block.body.is_empty() {
        return None;
    }
    let BlockPyTerm::IfTerm(if_term) = &block.term else {
        return None;
    };
    let then_index = *label_to_index.get(if_term.then_label.as_str())?;
    let else_index = *label_to_index.get(if_term.else_label.as_str())?;
    let source_params = jit_param_names_for_block(block, ambient_param_names);
    if jit_param_names_for_block(&function.blocks[then_index], ambient_param_names) != source_params
        || jit_param_names_for_block(&function.blocks[else_index], ambient_param_names)
            != source_params
    {
        return None;
    }
    let test = direct_simple_expr_from(&if_term.test)?;
    Some(DirectSimpleBrIfPlan {
        params: block.param_name_vec(),
        test,
        then_index,
        else_index,
    })
}

fn direct_simple_expr_ret_none_plan_from_block(
    block: &BbBlock,
) -> Option<DirectSimpleExprRetNonePlan> {
    if !matches!(block.term, BlockPyTerm::Return(None)) {
        return None;
    }
    let mut exprs = Vec::with_capacity(block.body.len());
    for op in &block.body {
        let BbStmt::Expr(expr) = op else {
            return None;
        };
        let expr = direct_simple_expr_from(expr)?;
        exprs.push(expr);
    }
    Some(DirectSimpleExprRetNonePlan {
        params: block.param_name_vec(),
        exprs,
    })
}

fn target_params_from_index(
    function: &BlockPyFunction<BbBlockPyPass>,
    target_index: usize,
    ambient_param_names: &HashSet<String>,
) -> Option<Vec<String>> {
    Some(jit_param_names_for_block(
        function.blocks.get(target_index)?,
        ambient_param_names,
    ))
}

fn direct_simple_delete_plan_from_targets(
    targets: &[ruff_python_ast::ExprName],
    known_names: &mut Vec<String>,
) -> Option<DirectSimpleDeletePlan> {
    let mut plan_targets = Vec::with_capacity(targets.len());
    for target in targets {
        let target_name = target.id.to_string();
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

fn direct_simple_op_from_bb_stmt(
    op: &BbStmt,
    known_names: &mut Vec<String>,
) -> Option<DirectSimpleOpPlan> {
    match op {
        BbStmt::Expr(expr_stmt) => {
            let value = direct_simple_expr_from(expr_stmt)?;
            Some(DirectSimpleOpPlan::Expr(value))
        }
        BbStmt::Assign(assign) => {
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
        BbStmt::Delete(delete_stmt) => {
            let delete_plan = direct_simple_delete_plan_from_targets(
                std::slice::from_ref(&delete_stmt.target),
                known_names,
            )?;
            Some(DirectSimpleOpPlan::Delete(delete_plan))
        }
    }
}

fn bb_stmt_kind(op: &BbStmt) -> &'static str {
    match op {
        BbStmt::Assign(_) => "Assign",
        BbStmt::Expr(_) => "Expr",
        BbStmt::Delete(_) => "Delete",
    }
}

fn bb_term_kind(term: &BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>) -> &'static str {
    match term {
        BlockPyTerm::Jump(_) => "Jump",
        BlockPyTerm::IfTerm(_) => "BrIf",
        BlockPyTerm::BranchTable(_) => "BrTable",
        BlockPyTerm::Raise(_) => "Raise",
        BlockPyTerm::Return(_) => "Ret",
        BlockPyTerm::TryJump(_) => "TryJump",
    }
}

fn unsupported_fastpath_block_message(
    function: &BlockPyFunction<BbBlockPyPass>,
    block: &BbBlock,
) -> String {
    let op_kinds = block
        .body
        .iter()
        .map(bb_stmt_kind)
        .collect::<Vec<_>>()
        .join(", ");
    let op_debug = block
        .body
        .iter()
        .map(|op| format!("{op:?}"))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "unsupported JIT block shape in {}:{}: term={}, ops=[{}], params={:?}, exc_target={:?}; op_debug=[{}]; expected direct-simple lowered block",
        function.names.qualname,
        block.label,
        bb_term_kind(&block.term),
        op_kinds,
        block.param_name_vec(),
        block.meta.exc_edge.as_ref().map(|edge| &edge.target),
        op_debug,
    )
}

fn direct_simple_block_plan_from_block(
    function: &BlockPyFunction<BbBlockPyPass>,
    block: &BbBlock,
    label_to_index: &HashMap<BlockPyLabel, usize>,
    ambient_param_names: &HashSet<String>,
) -> Option<DirectSimpleBlockPlan> {
    let mut known_names = block.param_name_vec();
    let mut ops = Vec::new();
    for op in &block.body {
        let stmt_op = direct_simple_op_from_bb_stmt(op, &mut known_names)?;
        ops.push(stmt_op);
    }
    let term = match &block.term {
        BlockPyTerm::Jump(target_label) => {
            let target_index = *label_to_index.get(target_label.as_str())?;
            let target_params =
                target_params_from_index(function, target_index, ambient_param_names)?;
            let target_args = target_label
                .args
                .iter()
                .map(direct_simple_block_arg_from)
                .collect::<Option<Vec<_>>>()?;
            DirectSimpleTermPlan::Jump {
                target_index,
                target_params,
                target_args,
            }
        }
        BlockPyTerm::IfTerm(if_term) => {
            let test_expr = direct_simple_expr_from(&if_term.test)?;
            let then_index = *label_to_index.get(if_term.then_label.as_str())?;
            let then_params = target_params_from_index(function, then_index, ambient_param_names)?;
            let else_index = *label_to_index.get(if_term.else_label.as_str())?;
            let else_params = target_params_from_index(function, else_index, ambient_param_names)?;
            DirectSimpleTermPlan::BrIf {
                test: test_expr,
                then_index,
                then_params,
                else_index,
                else_params,
            }
        }
        BlockPyTerm::BranchTable(branch) => {
            let index_expr = direct_simple_expr_from(&branch.index)?;
            let mut target_plans = Vec::with_capacity(branch.targets.len());
            for target_label in &branch.targets {
                let target_index = *label_to_index.get(target_label.as_str())?;
                let target_params =
                    target_params_from_index(function, target_index, ambient_param_names)?;
                target_plans.push((target_index, target_params));
            }
            let default_index = *label_to_index.get(branch.default_label.as_str())?;
            let default_params =
                target_params_from_index(function, default_index, ambient_param_names)?;
            DirectSimpleTermPlan::BrTable {
                index: index_expr,
                targets: target_plans,
                default_index,
                default_params,
            }
        }
        BlockPyTerm::Return(ret_value) => {
            let value = if let Some(expr) = ret_value.as_ref() {
                Some(direct_simple_expr_from(expr)?)
            } else {
                None
            };
            DirectSimpleTermPlan::Ret { value }
        }
        BlockPyTerm::Raise(raise_stmt) => {
            let exc = raise_stmt.exc.as_ref().and_then(direct_simple_expr_from);
            DirectSimpleTermPlan::Raise { exc }
        }
        BlockPyTerm::TryJump(_) => return None,
    };
    Some(DirectSimpleBlockPlan {
        params: block.param_name_vec(),
        ops,
        term,
    })
}

fn build_clif_plan(function: &BlockPyFunction<BbBlockPyPass>) -> Result<ClifPlan, String> {
    let ambient_param_names = function
        .closure_layout()
        .as_ref()
        .map(|layout| layout.ambient_storage_names())
        .unwrap_or_default();
    let ambient_param_name_set = ambient_param_name_set(function);
    let mut label_to_index = HashMap::new();
    for (index, block) in function.blocks.iter().enumerate() {
        label_to_index.insert(block.label.clone(), index);
    }
    let entry_index = 0;
    let mut block_terms = Vec::with_capacity(function.blocks.len());
    let mut block_exc_targets = Vec::with_capacity(function.blocks.len());
    let mut block_exc_dispatches = Vec::with_capacity(function.blocks.len());
    let mut block_param_names = Vec::with_capacity(function.blocks.len());
    let mut block_fast_paths = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        let exc_target =
            match block.meta.exc_edge.as_ref().map(|edge| &edge.target) {
                Some(label) => Some(label_to_index.get(label.as_str()).copied().unwrap_or_else(
                    || {
                        panic!(
                            "unknown exception target {label} in {}:{}",
                            function.names.qualname, block.label
                        )
                    },
                )),
                None => None,
            };
        let exc_dispatch = if let Some(target_index) = exc_target {
            let target_block = &function.blocks[target_index];
            let block_param_names = jit_param_names_for_block(block, &ambient_param_name_set);
            let full_target_param_names = target_block.param_name_vec();
            let owner_param_name = block_param_names
                .iter()
                .find(|param| param.as_str() == "_dp_self" || param.as_str() == "_dp_state")
                .cloned();
            let mut arg_sources = Vec::with_capacity(full_target_param_names.len());
            let exc_args = &block
                .meta
                .exc_edge
                .as_ref()
                .expect("exc_target implies exc_edge")
                .args;
            if exc_args.len() != full_target_param_names.len() {
                panic!(
                    "exception dispatch from {}:{} has {} explicit edge args for target {} with {} full params",
                    function.names.qualname,
                    block.label,
                    exc_args.len(),
                    target_block.label,
                    full_target_param_names.len()
                );
            }
            for (target_param_name, source) in full_target_param_names.iter().zip(exc_args.iter()) {
                if ambient_param_name_set.contains(target_param_name) {
                    continue;
                }
                match source {
                    BlockArg::Name(name) => {
                        if !block_param_names
                            .iter()
                            .any(|source_name| source_name == name)
                        {
                            if owner_param_name.is_none() {
                                arg_sources.push(BlockExcArgSource::NoneValue);
                            } else {
                                arg_sources
                                    .push(BlockExcArgSource::FrameLocal { name: name.clone() });
                            }
                        } else {
                            arg_sources.push(BlockExcArgSource::SourceParam { name: name.clone() });
                        }
                    }
                    BlockArg::CurrentException => {
                        arg_sources.push(BlockExcArgSource::Exception);
                    }
                    BlockArg::None => {
                        arg_sources.push(BlockExcArgSource::NoneValue);
                    }
                    BlockArg::Expr(_) => {
                        panic!(
                            "exception dispatch from {}:{} uses expr edge arg for target param {}",
                            function.names.qualname, block.label, target_param_name
                        );
                    }
                    BlockArg::AbruptKind(kind) => {
                        panic!(
                            "exception dispatch from {}:{} uses abrupt-kind edge arg {:?} for target param {}",
                            function.names.qualname, block.label, kind, target_param_name
                        );
                    }
                }
            }
            if owner_param_name.is_none()
                && arg_sources
                    .iter()
                    .any(|src| matches!(src, BlockExcArgSource::FrameLocal { .. }))
            {
                panic!(
                    "exception dispatch from {}:{} requires frame-local fallback but has no _dp_self/_dp_state parameter",
                    function.names.qualname, block.label
                );
            }
            Some(BlockExcDispatchPlan {
                target_index,
                owner_param_name,
                arg_sources,
            })
        } else {
            None
        };
        let term = match &block.term {
            BlockPyTerm::Jump(target) => {
                let target_index =
                    label_to_index
                        .get(target.as_str())
                        .copied()
                        .unwrap_or_else(|| {
                            panic!(
                                "unknown jump target {} in {}:{}",
                                target.as_str(),
                                function.names.qualname,
                                block.label
                            )
                        });
                BlockTermPlan::Jump { target_index }
            }
            BlockPyTerm::IfTerm(if_term) => {
                let then_index = label_to_index
                    .get(if_term.then_label.as_str())
                    .copied()
                    .unwrap_or_else(|| {
                        panic!(
                            "unknown then target {} in {}:{}",
                            if_term.then_label, function.names.qualname, block.label
                        )
                    });
                let else_index = label_to_index
                    .get(if_term.else_label.as_str())
                    .copied()
                    .unwrap_or_else(|| {
                        panic!(
                            "unknown else target {} in {}:{}",
                            if_term.else_label, function.names.qualname, block.label
                        )
                    });
                BlockTermPlan::BrIf {
                    then_index,
                    else_index,
                }
            }
            BlockPyTerm::BranchTable(branch) => {
                let default_index = label_to_index
                    .get(branch.default_label.as_str())
                    .copied()
                    .unwrap_or_else(|| {
                        panic!(
                            "unknown br_table default target {} in {}:{}",
                            branch.default_label, function.names.qualname, block.label
                        )
                    });
                let mut target_indices = Vec::with_capacity(branch.targets.len());
                for target in &branch.targets {
                    let target_index =
                        label_to_index
                            .get(target.as_str())
                            .copied()
                            .unwrap_or_else(|| {
                                panic!(
                                    "unknown br_table target {target} in {}:{}",
                                    function.names.qualname, block.label
                                )
                            });
                    target_indices.push(target_index);
                }
                BlockTermPlan::BrTable {
                    targets: target_indices,
                    default_index,
                }
            }
            BlockPyTerm::Raise(_) => BlockTermPlan::Raise,
            BlockPyTerm::Return(_) => BlockTermPlan::Ret,
            BlockPyTerm::TryJump(_) => {
                panic!(
                    "unexpected TryJump in BB function {}:{}",
                    function.names.qualname, block.label
                );
            }
        };
        let fast_path = {
            if block.body.is_empty() {
                match &block.term {
                    BlockPyTerm::Jump(target_label) => {
                        let target_index = label_to_index
                            .get(target_label.as_str())
                            .copied()
                            .unwrap_or_else(|| {
                                panic!(
                                    "unknown jump target {} in {}:{}",
                                    target_label.as_str(),
                                    function.names.qualname,
                                    block.label
                                )
                            });
                        let source_params =
                            jit_param_names_for_block(block, &ambient_param_name_set);
                        let target_params = jit_param_names_for_block(
                            &function.blocks[target_index],
                            &ambient_param_name_set,
                        );
                        if target_label.args.is_empty() && source_params == target_params {
                            BlockFastPath::JumpPassThrough { target_index }
                        } else if let Some(plan) = direct_simple_plan_from_block(block) {
                            BlockFastPath::DirectSimpleRet { plan }
                        } else if let Some(plan) = direct_simple_block_plan_from_block(
                            function,
                            block,
                            &label_to_index,
                            &ambient_param_name_set,
                        ) {
                            BlockFastPath::DirectSimpleBlock { plan }
                        } else {
                            BlockFastPath::None
                        }
                    }
                    BlockPyTerm::Return(None) => BlockFastPath::ReturnNone,
                    BlockPyTerm::IfTerm(_) => {
                        if let Some(plan) = direct_simple_brif_plan_from_block(
                            function,
                            block,
                            &label_to_index,
                            &ambient_param_name_set,
                        ) {
                            BlockFastPath::DirectSimpleBrIf { plan }
                        } else if let Some(plan) = direct_simple_block_plan_from_block(
                            function,
                            block,
                            &label_to_index,
                            &ambient_param_name_set,
                        ) {
                            BlockFastPath::DirectSimpleBlock { plan }
                        } else {
                            BlockFastPath::None
                        }
                    }
                    _ => {
                        if let Some(plan) = direct_simple_plan_from_block(block) {
                            BlockFastPath::DirectSimpleRet { plan }
                        } else if let Some(plan) = direct_simple_block_plan_from_block(
                            function,
                            block,
                            &label_to_index,
                            &ambient_param_name_set,
                        ) {
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
            } else if let Some(plan) = direct_simple_block_plan_from_block(
                function,
                block,
                &label_to_index,
                &ambient_param_name_set,
            ) {
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
        block_param_names.push(block.param_name_vec());
        block_fast_paths.push(fast_path);
    }
    Ok(ClifPlan {
        entry_index,
        block_labels: function
            .blocks
            .iter()
            .map(|block| block.label.to_string())
            .collect(),
        ambient_param_names,
        block_param_names,
        block_terms,
        block_exc_targets,
        block_exc_dispatches,
        block_fast_paths,
    })
}

pub fn register_clif_module_plans(
    module_name: &str,
    module: &BlockPyModule<BbBlockPyPass>,
) -> Result<(), String> {
    let lowered = dp_transform::passes::lower_try_jump_exception_flow(module)?;
    let debug_skips = std::env::var_os("DIET_PYTHON_DEBUG_JIT_PLAN_SKIPS").is_some();
    let mut plans = HashMap::new();
    let mut functions = HashMap::new();
    let mut skipped_errors: HashMap<String, String> = HashMap::new();
    for function in &lowered.callable_defs {
        let key = PlanKey {
            module: module_name.to_string(),
            function_id: function.function_id.0,
        };
        let plan_name = function
            .function_id
            .plan_qualname(function.names.qualname.as_str());
        match build_clif_plan(function) {
            Ok(plan) => {
                plans.insert(key.clone(), plan);
                functions.insert(key, function.clone());
            }
            Err(err) => {
                if debug_skips {
                    eprintln!(
                        "[diet-python:jitskip] module={} qualname={} entry={} reason={}",
                        module_name,
                        function.names.qualname,
                        function.entry_block().label_str(),
                        err
                    );
                }
                skipped_errors.insert(plan_name, err);
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

    let mut registry = clif_plan_registry()
        .lock()
        .map_err(|_| "failed to lock bb plan registry".to_string())?;
    registry.retain(|key, _| key.module != module_name);
    registry.extend(plans);
    drop(registry);

    let mut function_registry = bb_function_registry()
        .lock()
        .map_err(|_| "failed to lock bb function registry".to_string())?;
    function_registry.retain(|key, _| key.module != module_name);
    function_registry.extend(functions);
    Ok(())
}

pub fn lookup_clif_plan(module_name: &str, function_id: usize) -> Option<ClifPlan> {
    let registry = clif_plan_registry().lock().ok()?;
    registry
        .get(&PlanKey {
            module: module_name.to_string(),
            function_id,
        })
        .cloned()
}

pub fn lookup_blockpy_function(
    module_name: &str,
    function_id: usize,
) -> Option<BlockPyFunction<BbBlockPyPass>> {
    let registry = bb_function_registry().lock().ok()?;
    registry
        .get(&PlanKey {
            module: module_name.to_string(),
            function_id,
        })
        .cloned()
}

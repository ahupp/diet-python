use dp_transform::block_py::{
    AbruptKind, BbStmt, BbTerm, BlockArg, BlockPyFunction, BlockPyLabel, BlockPyModule,
    CoreBlockPyCallArg, CoreBlockPyExprWithoutAwaitOrYield, CoreBlockPyKeywordArg,
    CoreBlockPyLiteral, CoreNumberLiteralValue, PreparedBbBlock,
};
use dp_transform::passes::PreparedBbBlockPyPass;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct ClifPlan {
    pub ambient_param_names: Vec<String>,
    pub blocks: Vec<ClifBlockPlan>,
}

#[derive(Clone, Debug)]
pub struct ClifBlockPlan {
    pub label: String,
    pub param_names: Vec<String>,
    pub term: BbTerm,
    pub exc_target: Option<usize>,
    pub exc_dispatch: Option<BlockExcDispatchPlan>,
    pub fast_path: BlockFastPath,
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
pub enum BlockFastPath {
    None,
    JumpPassThrough { target_index: usize },
    DirectSimpleBrIf { plan: DirectSimpleBrIfPlan },
    DirectSimpleRet { plan: DirectSimpleRetPlan },
    DirectSimpleBlock { plan: DirectSimpleBlockPlan },
}

#[derive(Clone, Debug)]
pub enum DirectSimpleExprPlan {
    Name(String),
    Int(i64),
    Float(f64),
    Bytes(Vec<u8>),
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
        value: DirectSimpleExprPlan,
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
type FunctionRegistry = HashMap<PlanKey, BlockPyFunction<PreparedBbBlockPyPass>>;

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

fn direct_simple_block_arg_from(arg: &BlockArg) -> Option<DirectSimpleBlockArgPlan> {
    match arg {
        BlockArg::Name(name) => Some(DirectSimpleBlockArgPlan::Name(name.clone())),
        BlockArg::None => Some(DirectSimpleBlockArgPlan::None),
        BlockArg::CurrentException => Some(DirectSimpleBlockArgPlan::CurrentException),
        BlockArg::AbruptKind(kind) => Some(DirectSimpleBlockArgPlan::Expr(
            DirectSimpleExprPlan::Int(abrupt_kind_tag(*kind)),
        )),
    }
}

fn direct_simple_plan_from_block(block: &PreparedBbBlock) -> Option<DirectSimpleRetPlan> {
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
    let BbTerm::Return(ret_value) = &block.term else {
        return None;
    };
    let ret = direct_simple_expr_from(ret_value)?;
    Some(DirectSimpleRetPlan {
        params: block.param_name_vec(),
        assigns,
        ret,
    })
}

fn ambient_param_name_set(function: &BlockPyFunction<PreparedBbBlockPyPass>) -> HashSet<String> {
    function
        .closure_layout()
        .as_ref()
        .map(|layout| layout.ambient_storage_names().into_iter().collect())
        .unwrap_or_default()
}

fn jit_param_names_for_block(
    block: &PreparedBbBlock,
    ambient_param_names: &HashSet<String>,
) -> Vec<String> {
    block
        .param_names()
        .filter(|name| !ambient_param_names.contains(*name))
        .map(ToString::to_string)
        .collect()
}

fn direct_simple_brif_plan_from_block(
    function: &BlockPyFunction<PreparedBbBlockPyPass>,
    block: &PreparedBbBlock,
    label_to_index: &HashMap<BlockPyLabel, usize>,
    ambient_param_names: &HashSet<String>,
) -> Option<DirectSimpleBrIfPlan> {
    if !block.body.is_empty() {
        return None;
    }
    let BbTerm::IfTerm(if_term) = &block.term else {
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

fn target_params_from_index(
    function: &BlockPyFunction<PreparedBbBlockPyPass>,
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

fn direct_simple_block_plan_from_block(
    function: &BlockPyFunction<PreparedBbBlockPyPass>,
    block: &PreparedBbBlock,
    label_to_index: &HashMap<BlockPyLabel, usize>,
    ambient_param_names: &HashSet<String>,
) -> DirectSimpleBlockPlan {
    let mut known_names = block.param_name_vec();
    let mut ops = Vec::new();
    for op in &block.body {
        let stmt_op = direct_simple_op_from_bb_stmt(op, &mut known_names).unwrap_or_else(|| {
            panic!(
                "unexpected non-direct-simple BB stmt in {}:{}: kind={} stmt={op:?}",
                function.names.qualname,
                block.label,
                bb_stmt_kind(op),
            )
        });
        ops.push(stmt_op);
    }
    let term = match &block.term {
        BbTerm::Jump(target_label) => {
            let target_index = *label_to_index
                .get(target_label.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "unknown jump target {} in {}:{}",
                        target_label.as_str(),
                        function.names.qualname,
                        block.label
                    )
                });
            let target_params =
                target_params_from_index(function, target_index, ambient_param_names)
                    .unwrap_or_else(|| {
                        panic!(
                            "missing target params for jump target {} in {}:{}",
                            target_label.as_str(),
                            function.names.qualname,
                            block.label
                        )
                    });
            let target_args = target_label
                .args
                .iter()
                .map(direct_simple_block_arg_from)
                .collect::<Option<Vec<_>>>()
                .unwrap_or_else(|| {
                    panic!(
                        "unexpected non-direct-simple jump arg in {}:{}: {:?}",
                        function.names.qualname, block.label, target_label.args
                    )
                });
            DirectSimpleTermPlan::Jump {
                target_index,
                target_params,
                target_args,
            }
        }
        BbTerm::IfTerm(if_term) => {
            let test_expr = direct_simple_expr_from(&if_term.test).unwrap_or_else(|| {
                panic!(
                    "unexpected non-direct-simple if test in {}:{}: {:?}",
                    function.names.qualname, block.label, if_term.test
                )
            });
            let then_index = *label_to_index
                .get(if_term.then_label.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "unknown then target {} in {}:{}",
                        if_term.then_label, function.names.qualname, block.label
                    )
                });
            let then_params = target_params_from_index(function, then_index, ambient_param_names)
                .unwrap_or_else(|| {
                    panic!(
                        "missing then params for target {} in {}:{}",
                        if_term.then_label, function.names.qualname, block.label
                    )
                });
            let else_index = *label_to_index
                .get(if_term.else_label.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "unknown else target {} in {}:{}",
                        if_term.else_label, function.names.qualname, block.label
                    )
                });
            let else_params = target_params_from_index(function, else_index, ambient_param_names)
                .unwrap_or_else(|| {
                    panic!(
                        "missing else params for target {} in {}:{}",
                        if_term.else_label, function.names.qualname, block.label
                    )
                });
            DirectSimpleTermPlan::BrIf {
                test: test_expr,
                then_index,
                then_params,
                else_index,
                else_params,
            }
        }
        BbTerm::BranchTable(branch) => {
            let index_expr = direct_simple_expr_from(&branch.index).unwrap_or_else(|| {
                panic!(
                    "unexpected non-direct-simple br_table index in {}:{}: {:?}",
                    function.names.qualname, block.label, branch.index
                )
            });
            let mut target_plans = Vec::with_capacity(branch.targets.len());
            for target_label in &branch.targets {
                let target_index =
                    *label_to_index
                        .get(target_label.as_str())
                        .unwrap_or_else(|| {
                            panic!(
                                "unknown br_table target {} in {}:{}",
                                target_label, function.names.qualname, block.label
                            )
                        });
                let target_params =
                    target_params_from_index(function, target_index, ambient_param_names)
                        .unwrap_or_else(|| {
                            panic!(
                                "missing br_table params for target {} in {}:{}",
                                target_label, function.names.qualname, block.label
                            )
                        });
                target_plans.push((target_index, target_params));
            }
            let default_index = *label_to_index
                .get(branch.default_label.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "unknown br_table default target {} in {}:{}",
                        branch.default_label, function.names.qualname, block.label
                    )
                });
            let default_params =
                target_params_from_index(function, default_index, ambient_param_names)
                    .unwrap_or_else(|| {
                        panic!(
                            "missing br_table params for default target {} in {}:{}",
                            branch.default_label, function.names.qualname, block.label
                        )
                    });
            DirectSimpleTermPlan::BrTable {
                index: index_expr,
                targets: target_plans,
                default_index,
                default_params,
            }
        }
        BbTerm::Return(ret_value) => {
            let value = direct_simple_expr_from(ret_value).unwrap_or_else(|| {
                panic!(
                    "unexpected non-direct-simple return value in {}:{}: {:?}",
                    function.names.qualname, block.label, ret_value
                )
            });
            DirectSimpleTermPlan::Ret { value }
        }
        BbTerm::Raise(raise_stmt) => {
            let exc = raise_stmt.exc.as_ref().map(|expr| {
                direct_simple_expr_from(expr).unwrap_or_else(|| {
                    panic!(
                        "unexpected non-direct-simple raise value in {}:{}: {:?}",
                        function.names.qualname, block.label, expr
                    )
                })
            });
            DirectSimpleTermPlan::Raise { exc }
        }
    };
    DirectSimpleBlockPlan {
        params: block.param_name_vec(),
        ops,
        term,
    }
}

fn build_clif_plan(function: &BlockPyFunction<PreparedBbBlockPyPass>) -> ClifPlan {
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
    let mut blocks = Vec::with_capacity(function.blocks.len());
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
        let term = block.term.clone();
        let fast_path = {
            if block.body.is_empty() {
                match &block.term {
                    BbTerm::Jump(target_label) => {
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
                        } else {
                            BlockFastPath::DirectSimpleBlock {
                                plan: direct_simple_block_plan_from_block(
                                    function,
                                    block,
                                    &label_to_index,
                                    &ambient_param_name_set,
                                ),
                            }
                        }
                    }
                    BbTerm::IfTerm(_) => {
                        if let Some(plan) = direct_simple_brif_plan_from_block(
                            function,
                            block,
                            &label_to_index,
                            &ambient_param_name_set,
                        ) {
                            BlockFastPath::DirectSimpleBrIf { plan }
                        } else {
                            BlockFastPath::DirectSimpleBlock {
                                plan: direct_simple_block_plan_from_block(
                                    function,
                                    block,
                                    &label_to_index,
                                    &ambient_param_name_set,
                                ),
                            }
                        }
                    }
                    _ => {
                        if let Some(plan) = direct_simple_plan_from_block(block) {
                            BlockFastPath::DirectSimpleRet { plan }
                        } else {
                            BlockFastPath::DirectSimpleBlock {
                                plan: direct_simple_block_plan_from_block(
                                    function,
                                    block,
                                    &label_to_index,
                                    &ambient_param_name_set,
                                ),
                            }
                        }
                    }
                }
            } else if let Some(plan) = direct_simple_plan_from_block(block) {
                BlockFastPath::DirectSimpleRet { plan }
            } else {
                BlockFastPath::DirectSimpleBlock {
                    plan: direct_simple_block_plan_from_block(
                        function,
                        block,
                        &label_to_index,
                        &ambient_param_name_set,
                    ),
                }
            }
        };
        blocks.push(ClifBlockPlan {
            label: block.label.to_string(),
            param_names: block.param_name_vec(),
            term,
            exc_target,
            exc_dispatch,
            fast_path,
        });
    }
    ClifPlan {
        ambient_param_names,
        blocks,
    }
}

pub fn register_clif_module_plans(
    module_name: &str,
    module: &BlockPyModule<PreparedBbBlockPyPass>,
) -> Result<(), String> {
    let mut plans = HashMap::new();
    let mut functions = HashMap::new();
    for function in &module.callable_defs {
        let key = PlanKey {
            module: module_name.to_string(),
            function_id: function.function_id.0,
        };
        let plan = build_clif_plan(function);
        plans.insert(key.clone(), plan);
        functions.insert(key, function.clone());
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
) -> Option<BlockPyFunction<PreparedBbBlockPyPass>> {
    let registry = bb_function_registry().lock().ok()?;
    registry
        .get(&PlanKey {
            module: module_name.to_string(),
            function_id,
        })
        .cloned()
}

use dp_transform::basic_block::bb_ir::{BbBlock, BbExpr, BbModule, BbOp, BbTerm};
use ruff_python_ast::Number;
use std::borrow::Cow;
use std::collections::HashMap;
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

type PlanRegistry = HashMap<PlanKey, ClifPlan>;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PlanKey {
    pub module: String,
    pub function_id: usize,
}

static CLIF_PLAN_REGISTRY: OnceLock<Mutex<PlanRegistry>> = OnceLock::new();

fn clif_plan_registry() -> &'static Mutex<PlanRegistry> {
    CLIF_PLAN_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
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

fn direct_simple_plan_from_block(block: &BbBlock) -> Option<DirectSimpleRetPlan> {
    let mut known_names: Vec<String> = block.meta.params.clone();
    let mut assigns = Vec::new();
    for op in &block.body {
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
        params: block.meta.params.clone(),
        assigns,
        ret,
    })
}

fn direct_simple_brif_plan_from_block(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
    block: &BbBlock,
    label_to_index: &HashMap<String, usize>,
) -> Option<DirectSimpleBrIfPlan> {
    if !block.body.is_empty() {
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
    let source_params = block.meta.params.as_slice();
    if function.blocks[then_index].meta.params.as_slice() != source_params
        || function.blocks[else_index].meta.params.as_slice() != source_params
    {
        return None;
    }
    let test = direct_simple_expr_from(test)?;
    Some(DirectSimpleBrIfPlan {
        params: block.meta.params.clone(),
        test,
        then_index,
        else_index,
    })
}

fn direct_simple_expr_ret_none_plan_from_block(
    block: &BbBlock,
) -> Option<DirectSimpleExprRetNonePlan> {
    if !matches!(block.term, BbTerm::Ret(None)) {
        return None;
    }
    let mut exprs = Vec::with_capacity(block.body.len());
    for op in &block.body {
        let BbOp::Expr(expr_op) = op else {
            return None;
        };
        let expr = direct_simple_expr_from(&expr_op.value)?;
        exprs.push(expr);
    }
    Some(DirectSimpleExprRetNonePlan {
        params: block.meta.params.clone(),
        exprs,
    })
}

fn target_params_from_index(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
    target_index: usize,
) -> Option<Vec<String>> {
    Some(function.blocks.get(target_index)?.meta.params.clone())
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
        BbTerm::Raise { .. } => "Raise",
        BbTerm::Ret(_) => "Ret",
    }
}

fn unsupported_fastpath_block_message(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
    block: &BbBlock,
) -> String {
    let op_kinds = block
        .body
        .iter()
        .map(bb_op_kind)
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
        function.qualname,
        block.label,
        bb_term_kind(&block.term),
        op_kinds,
        block.meta.params,
        block.meta.exc_target_label,
        op_debug,
    )
}

fn direct_simple_block_plan_from_block(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
    block: &BbBlock,
    label_to_index: &HashMap<String, usize>,
) -> Option<DirectSimpleBlockPlan> {
    let mut known_names: Vec<String> = block.meta.params.clone();
    let mut ops = Vec::new();
    for op in &block.body {
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
                block
                    .meta
                    .params
                    .iter()
                    .find(|name| {
                        name.as_str() == "_dp_resume_exc"
                            || name.starts_with("_dp_try_exc_")
                            || name.starts_with("_dp_uncaught_exc_")
                    })
                    .map(|name| DirectSimpleExprPlan::Name(name.clone()))
            };
            let cause = if let Some(expr) = cause.as_ref() {
                Some(direct_simple_expr_from(expr)?)
            } else {
                None
            };
            DirectSimpleTermPlan::Raise { exc, cause }
        }
    };
    Some(DirectSimpleBlockPlan {
        params: block.meta.params.clone(),
        ops,
        term,
    })
}

fn build_clif_plan(
    function: &dp_transform::basic_block::bb_ir::BbFunction,
) -> Result<ClifPlan, String> {
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
    let ambient_param_names = function
        .closure_layout
        .as_ref()
        .map(|layout| {
            layout
                .freevars
                .iter()
                .chain(layout.cellvars.iter())
                .chain(layout.runtime_cells.iter())
                .map(|slot| slot.storage_name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut label_to_index = HashMap::new();
    for (index, block) in function.blocks.iter().enumerate() {
        label_to_index.insert(block.label.clone(), index);
    }
    let entry_label = function.entry_label();
    let Some(entry_index) = label_to_index.get(entry_label).copied() else {
        return Err(format!(
            "missing entry label {} in function {}",
            entry_label, function.qualname
        ));
    };
    let mut block_terms = Vec::with_capacity(function.blocks.len());
    let mut block_exc_targets = Vec::with_capacity(function.blocks.len());
    let mut block_exc_dispatches = Vec::with_capacity(function.blocks.len());
    let mut block_param_names = Vec::with_capacity(function.blocks.len());
    let mut block_fast_paths = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        let exc_target = match block.meta.exc_target_label.as_ref() {
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
                .meta
                .params
                .iter()
                .position(|name| name == "_dp_self")
                .or_else(|| {
                    block
                        .meta
                        .params
                        .iter()
                        .position(|name| name == "_dp_state")
                });
            let mut arg_sources = Vec::with_capacity(target_block.meta.params.len());
            for target_param in &target_block.meta.params {
                if block.meta.exc_name.as_deref() == Some(target_param.as_str()) {
                    arg_sources.push(BlockExcArgSource::Exception);
                    continue;
                }
                if let Some(source_index) = block
                    .meta
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
                    || target_param.starts_with("_dp_uncaught_exc_")
                {
                    arg_sources.push(BlockExcArgSource::Exception);
                    continue;
                }
                if target_param == "_dp_resume_exc" {
                    arg_sources.push(BlockExcArgSource::NoneValue);
                    continue;
                }
                if target_param.starts_with("_dp_try_reason_")
                    || target_param.starts_with("_dp_try_value_")
                {
                    arg_sources.push(BlockExcArgSource::NoneValue);
                    continue;
                }
                if owner_param_index.is_none() {
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
        };
        let fast_path = {
            if block.body.is_empty() {
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
                        let source_params = block.meta.params.as_slice();
                        let target_params = function.blocks[target_index].meta.params.as_slice();
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
        block_param_names.push(block.meta.params.clone());
        block_fast_paths.push(fast_path);
    }
    Ok(ClifPlan {
        entry_index,
        block_labels: function
            .blocks
            .iter()
            .map(|block| block.label.clone())
            .collect(),
        ambient_param_names,
        block_param_names,
        block_terms,
        block_exc_targets,
        block_exc_dispatches,
        block_fast_paths,
    })
}

pub fn register_clif_module_plans(module_name: &str, module: &BbModule) -> Result<(), String> {
    let lowered = dp_transform::basic_block::lower_try_jump_exception_flow(module)?;
    let debug_skips = std::env::var_os("DIET_PYTHON_DEBUG_JIT_PLAN_SKIPS").is_some();
    let mut plans = HashMap::new();
    let mut skipped_errors: HashMap<String, String> = HashMap::new();
    for function in &lowered.callable_defs {
        let plan_name = function
            .function_id
            .plan_qualname(function.qualname.as_str());
        match build_clif_plan(function) {
            Ok(plan) => {
                plans.insert(
                    PlanKey {
                        module: module_name.to_string(),
                        function_id: function.function_id.0,
                    },
                    plan,
                );
            }
            Err(err) => {
                if debug_skips {
                    eprintln!(
                        "[diet-python:jitskip] module={} qualname={} entry={} reason={}",
                        module_name,
                        function.qualname,
                        function.entry_label(),
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

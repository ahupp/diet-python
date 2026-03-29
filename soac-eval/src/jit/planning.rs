use dp_transform::block_py::{
    AbruptKind, BlockArg, BlockPyFunction, BlockPyFunctionKind, BlockPyLabel, BlockPyModule,
    BlockPyStmt, BlockPyTerm, CodegenBlock, CodegenBlockPyLiteral, CoreBlockPyCallArg,
    CoreBlockPyKeywordArg, CoreNumberLiteralValue, LocatedCodegenBlockPyExpr, LocatedName,
    Operation, ParamKind,
};
use dp_transform::passes::CodegenBlockPyPass;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct ClifPlan {
    pub entry_params: Vec<ClifBindingParam>,
    pub entry_param_names: Vec<String>,
    pub entry_param_default_sources: Vec<Option<ClifEntryParamDefaultSource>>,
    pub ambient_param_names: Vec<String>,
    pub owned_cell_slot_names: Vec<String>,
    pub slot_names: Vec<String>,
    pub blocks: Vec<ClifBlockPlan>,
}

#[derive(Clone, Debug)]
pub struct JitFunctionInfo {
    pub entry_params: Vec<ClifBindingParam>,
    pub entry_param_names: Vec<String>,
    pub entry_param_default_sources: Vec<Option<ClifEntryParamDefaultSource>>,
    pub ambient_param_names: Vec<String>,
    pub owned_cell_slot_names: Vec<String>,
    pub slot_names: Vec<String>,
    pub blocks: Vec<JitBlockInfo>,
}

#[derive(Clone, Debug)]
pub struct ClifBindingParam {
    pub name: String,
    pub kind: ClifBindingParamKind,
    pub has_default: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClifBindingParamKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarArgs,
    KeywordOnly,
    VarKeyword,
}

#[derive(Clone, Debug)]
pub enum ClifEntryParamDefaultSource {
    Positional(usize),
    KeywordOnly(String),
}

#[derive(Clone, Debug)]
pub struct ClifBlockPlan {
    pub label: String,
    pub param_names: Vec<String>,
    pub runtime_param_names: Vec<String>,
    pub exc_target: Option<usize>,
    pub exc_dispatch: Option<BlockExcDispatchPlan>,
    pub plan: DirectSimpleBlockPlan,
}

#[derive(Clone, Debug)]
pub struct JitBlockInfo {
    pub runtime_param_names: Vec<String>,
    pub exc_target: Option<usize>,
    pub exc_dispatch: Option<BlockExcDispatchPlan>,
}

#[derive(Clone, Debug)]
pub struct BlockExcDispatchPlan {
    pub target_index: usize,
    pub slot_writes: Vec<(String, BlockExcArgSource)>,
}

#[derive(Clone, Debug)]
pub enum BlockExcArgSource {
    Name(String),
    Exception,
    NoneValue,
}

#[derive(Clone, Debug)]
pub enum DirectSimpleExprPlan {
    Name(LocatedName),
    Int(i64),
    Float(f64),
    Bytes(Vec<u8>),
    Op(Box<Operation<DirectSimpleExprPlan>>),
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
    pub target: LocatedName,
    pub value: DirectSimpleExprPlan,
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
        full_target_params: Vec<String>,
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
    LocalName(LocatedName),
}

#[derive(Clone, Debug)]
pub struct DirectSimpleBlockPlan {
    pub ops: Vec<DirectSimpleOpPlan>,
    pub term: DirectSimpleTermPlan,
}

#[derive(Clone)]
struct RegisteredJitFunction {
    function: BlockPyFunction<CodegenBlockPyPass>,
    info: JitFunctionInfo,
}

type FunctionRegistry = HashMap<PlanKey, RegisteredJitFunction>;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PlanKey {
    pub module: String,
    pub function_id: usize,
}

static BB_FUNCTION_REGISTRY: OnceLock<Mutex<FunctionRegistry>> = OnceLock::new();

struct ValidatedPreparedBbFunction<'a> {
    function: &'a BlockPyFunction<CodegenBlockPyPass>,
    ambient_param_names: Vec<String>,
    ambient_param_name_set: HashSet<String>,
    label_to_index: HashMap<BlockPyLabel, usize>,
}

impl<'a> ValidatedPreparedBbFunction<'a> {
    fn new(function: &'a BlockPyFunction<CodegenBlockPyPass>) -> Self {
        let ambient_param_names = function
            .closure_layout()
            .as_ref()
            .map(|layout| layout.ambient_storage_names())
            .unwrap_or_default();
        let ambient_param_name_set = ambient_param_names.iter().cloned().collect();
        let mut label_to_index = HashMap::with_capacity(function.blocks.len());
        for (index, block) in function.blocks.iter().enumerate() {
            if label_to_index.insert(block.label.clone(), index).is_some() {
                panic!(
                    "duplicate BB block label {} in {}",
                    block.label, function.names.qualname
                );
            }
        }
        let validated = Self {
            function,
            ambient_param_names,
            ambient_param_name_set,
            label_to_index,
        };
        validated.validate_cfg_targets();
        validated
    }

    fn validate_cfg_targets(&self) {
        for block in &self.function.blocks {
            if let Some(exc_edge) = &block.exc_edge {
                self.require_known_target(block, &exc_edge.target, "exception target");
            }
            match &block.term {
                BlockPyTerm::Jump(target_label) => {
                    self.require_known_target(block, &target_label.target, "jump target");
                }
                BlockPyTerm::IfTerm(if_term) => {
                    self.require_known_target(block, &if_term.then_label, "then target");
                    self.require_known_target(block, &if_term.else_label, "else target");
                }
                BlockPyTerm::BranchTable(branch) => {
                    for target_label in &branch.targets {
                        self.require_known_target(block, target_label, "br_table target");
                    }
                    self.require_known_target(
                        block,
                        &branch.default_label,
                        "br_table default target",
                    );
                }
                BlockPyTerm::Return(_) | BlockPyTerm::Raise(_) => {}
            }
        }
    }

    fn require_known_target(
        &self,
        source_block: &CodegenBlock,
        target: &BlockPyLabel,
        edge_kind: &str,
    ) {
        if !self.label_to_index.contains_key(target) {
            panic!(
                "unknown {edge_kind} {} in {}:{}",
                target, self.function.names.qualname, source_block.label
            );
        }
    }

    fn index_of_target(&self, target: &BlockPyLabel) -> usize {
        self.label_to_index
            .get(target)
            .copied()
            .expect("validated BB label lookup")
    }

    fn jit_param_names_for_block(&self, block: &CodegenBlock) -> Vec<String> {
        block
            .exception_param()
            .into_iter()
            .map(ToString::to_string)
            .collect()
    }

    fn jit_param_names_for_index(&self, target_index: usize) -> Vec<String> {
        self.jit_param_names_for_block(&self.function.blocks[target_index])
    }

    fn exc_target_index(&self, block: &CodegenBlock) -> Option<usize> {
        block
            .exc_edge
            .as_ref()
            .map(|edge| self.index_of_target(&edge.target))
    }

    fn exc_dispatch_plan(
        &self,
        block: &CodegenBlock,
        exc_target: Option<usize>,
    ) -> Option<BlockExcDispatchPlan> {
        let target_index = exc_target?;
        let target_block = &self.function.blocks[target_index];
        let runtime_param_name_set = self
            .jit_param_names_for_index(target_index)
            .into_iter()
            .collect::<HashSet<_>>();
        let exc_edge = block
            .exc_edge
            .as_ref()
            .expect("exc_target implies exc_edge");
        let full_target_param_names = target_block.param_name_vec();
        if exc_edge.args.len() != full_target_param_names.len() {
            panic!(
                "exception dispatch from {}:{} has {} explicit edge args for target {} with {} full params",
                self.function.names.qualname,
                block.label,
                exc_edge.args.len(),
                target_block.label,
                full_target_param_names.len()
            );
        }
        let mut slot_writes = Vec::new();
        for (target_param_name, source) in full_target_param_names.iter().zip(exc_edge.args.iter())
        {
            if runtime_param_name_set.contains(target_param_name)
                || self.ambient_param_name_set.contains(target_param_name)
            {
                continue;
            }
            let source = match source {
                BlockArg::Name(name) => BlockExcArgSource::Name(name.clone()),
                BlockArg::CurrentException => BlockExcArgSource::Exception,
                BlockArg::None => BlockExcArgSource::NoneValue,
                BlockArg::AbruptKind(kind) => {
                    panic!(
                        "exception dispatch from {}:{} uses abrupt-kind edge arg {:?} for target param {}",
                        self.function.names.qualname, block.label, kind, target_param_name
                    );
                }
            };
            slot_writes.push((target_param_name.clone(), source));
        }
        Some(BlockExcDispatchPlan {
            target_index,
            slot_writes,
        })
    }

    fn slot_names(&self) -> Vec<String> {
        let mut slot_names = Vec::new();
        let mut seen = HashSet::new();

        for name in &self.ambient_param_names {
            if seen.insert(name.clone()) {
                slot_names.push(name.clone());
            }
        }

        for name in self.function.params.names() {
            if seen.insert(name.clone()) {
                slot_names.push(name);
            }
        }

        for name in self.function.local_cell_slots() {
            if seen.insert(name.clone()) {
                slot_names.push(name);
            }
        }

        for block in &self.function.blocks {
            for name in block.param_names() {
                if seen.insert(name.to_string()) {
                    slot_names.push(name.to_string());
                }
            }
        }

        slot_names
    }

    fn entry_param_default_sources(&self) -> Vec<Option<ClifEntryParamDefaultSource>> {
        let mut next_positional_default = 0usize;
        self.function
            .params
            .params
            .iter()
            .map(|param| {
                if !param.has_default {
                    return None;
                }
                match param.kind {
                    ParamKind::PosOnly | ParamKind::Any => {
                        let index = next_positional_default;
                        next_positional_default += 1;
                        Some(ClifEntryParamDefaultSource::Positional(index))
                    }
                    ParamKind::KwOnly => {
                        Some(ClifEntryParamDefaultSource::KeywordOnly(param.name.clone()))
                    }
                    ParamKind::VarArg | ParamKind::KwArg => None,
                }
            })
            .collect()
    }

    fn entry_params(&self) -> Vec<ClifBindingParam> {
        self.function
            .params
            .params
            .iter()
            .map(|param| ClifBindingParam {
                name: param.name.clone(),
                kind: match param.kind {
                    ParamKind::PosOnly => ClifBindingParamKind::PositionalOnly,
                    ParamKind::Any => ClifBindingParamKind::PositionalOrKeyword,
                    ParamKind::VarArg => ClifBindingParamKind::VarArgs,
                    ParamKind::KwOnly => ClifBindingParamKind::KeywordOnly,
                    ParamKind::KwArg => ClifBindingParamKind::VarKeyword,
                },
                has_default: param.has_default,
            })
            .collect()
    }
}

fn bb_function_registry() -> &'static Mutex<FunctionRegistry> {
    BB_FUNCTION_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn direct_simple_expr_from(expr: &LocatedCodegenBlockPyExpr) -> DirectSimpleExprPlan {
    match expr {
        dp_transform::block_py::CodegenBlockPyExpr::Name(name) => {
            DirectSimpleExprPlan::Name(name.clone())
        }
        dp_transform::block_py::CodegenBlockPyExpr::Literal(literal) => match literal {
            CodegenBlockPyLiteral::BytesLiteral(bytes) => {
                DirectSimpleExprPlan::Bytes(bytes.value.clone())
            }
            CodegenBlockPyLiteral::NumberLiteral(number) => match &number.value {
                CoreNumberLiteralValue::Int(value) => value
                    .as_i64()
                    .map(DirectSimpleExprPlan::Int)
                    .unwrap_or_else(|| {
                        panic!("integer literal does not fit in direct-simple i64 plan: {value}")
                    }),
                CoreNumberLiteralValue::Float(value) => DirectSimpleExprPlan::Float(*value),
            },
        },
        dp_transform::block_py::CodegenBlockPyExpr::Call(call) => {
            let func = direct_simple_expr_from(call.func.as_ref());
            let mut parts = Vec::with_capacity(call.args.len() + call.keywords.len());
            for arg in &call.args {
                match arg {
                    CoreBlockPyCallArg::Positional(arg) => {
                        parts.push(DirectSimpleCallPart::Pos(direct_simple_expr_from(arg)));
                    }
                    CoreBlockPyCallArg::Starred(arg) => {
                        parts.push(DirectSimpleCallPart::Star(direct_simple_expr_from(arg)));
                    }
                }
            }
            for keyword in &call.keywords {
                match keyword {
                    CoreBlockPyKeywordArg::Named { arg: name, value } => {
                        parts.push(DirectSimpleCallPart::Kw {
                            name: name.to_string(),
                            value: direct_simple_expr_from(value),
                        });
                    }
                    CoreBlockPyKeywordArg::Starred(value) => {
                        parts.push(DirectSimpleCallPart::KwStar(direct_simple_expr_from(value)));
                    }
                }
            }
            DirectSimpleExprPlan::Call {
                func: Box::new(func),
                parts,
            }
        }
        dp_transform::block_py::CodegenBlockPyExpr::Op(operation) => {
            let operation = operation
                .clone()
                .map_expr(&mut |arg| direct_simple_expr_from(&arg));
            DirectSimpleExprPlan::Op(Box::new(operation))
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

fn direct_simple_delete_plan_from_targets(targets: &[LocatedName]) -> DirectSimpleDeletePlan {
    let mut plan_targets = Vec::with_capacity(targets.len());
    for target in targets {
        plan_targets.push(DirectSimpleDeleteTargetPlan::LocalName(target.clone()));
    }
    DirectSimpleDeletePlan {
        targets: plan_targets,
    }
}

fn direct_simple_op_from_bb_stmt(
    op: &BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName>,
) -> DirectSimpleOpPlan {
    match op {
        BlockPyStmt::Expr(expr_stmt) => {
            DirectSimpleOpPlan::Expr(direct_simple_expr_from(expr_stmt))
        }
        BlockPyStmt::Assign(assign) => {
            let value = direct_simple_expr_from(&assign.value);
            DirectSimpleOpPlan::Assign(DirectSimpleAssignPlan {
                target: assign.target.clone(),
                value,
            })
        }
        BlockPyStmt::Delete(delete_stmt) => DirectSimpleOpPlan::Delete(
            direct_simple_delete_plan_from_targets(std::slice::from_ref(&delete_stmt.target)),
        ),
    }
}

fn direct_simple_block_plan_from_block(
    function: &ValidatedPreparedBbFunction<'_>,
    block: &CodegenBlock,
) -> DirectSimpleBlockPlan {
    let mut ops = Vec::new();
    for op in &block.body {
        let stmt_op = direct_simple_op_from_bb_stmt(op);
        ops.push(stmt_op);
    }
    let term = match &block.term {
        BlockPyTerm::Jump(target_label) => {
            let target_index = function.index_of_target(&target_label.target);
            let target_params = function.jit_param_names_for_index(target_index);
            let full_target_params = function.function.blocks[target_index].param_name_vec();
            let target_args = target_label
                .args
                .iter()
                .map(direct_simple_block_arg_from)
                .collect::<Option<Vec<_>>>()
                .unwrap_or_else(|| {
                    panic!(
                        "unexpected non-direct-simple jump arg in {}:{}: {:?}",
                        function.function.names.qualname, block.label, target_label.args
                    )
                });
            DirectSimpleTermPlan::Jump {
                target_index,
                target_params,
                full_target_params,
                target_args,
            }
        }
        BlockPyTerm::IfTerm(if_term) => {
            let test_expr = direct_simple_expr_from(&if_term.test);
            let then_index = function.index_of_target(&if_term.then_label);
            let then_params = function.jit_param_names_for_index(then_index);
            let else_index = function.index_of_target(&if_term.else_label);
            let else_params = function.jit_param_names_for_index(else_index);
            DirectSimpleTermPlan::BrIf {
                test: test_expr,
                then_index,
                then_params,
                else_index,
                else_params,
            }
        }
        BlockPyTerm::BranchTable(branch) => {
            let index_expr = direct_simple_expr_from(&branch.index);
            let mut target_plans = Vec::with_capacity(branch.targets.len());
            for target_label in &branch.targets {
                let target_index = function.index_of_target(target_label);
                let target_params = function.jit_param_names_for_index(target_index);
                target_plans.push((target_index, target_params));
            }
            let default_index = function.index_of_target(&branch.default_label);
            let default_params = function.jit_param_names_for_index(default_index);
            DirectSimpleTermPlan::BrTable {
                index: index_expr,
                targets: target_plans,
                default_index,
                default_params,
            }
        }
        BlockPyTerm::Return(ret_value) => {
            let value = direct_simple_expr_from(ret_value);
            DirectSimpleTermPlan::Ret { value }
        }
        BlockPyTerm::Raise(raise_stmt) => {
            let exc = raise_stmt.exc.as_ref().map(direct_simple_expr_from);
            DirectSimpleTermPlan::Raise { exc }
        }
    };
    DirectSimpleBlockPlan { ops, term }
}

fn build_jit_function_info(function: &BlockPyFunction<CodegenBlockPyPass>) -> JitFunctionInfo {
    let function = ValidatedPreparedBbFunction::new(function);
    let slot_names = function.slot_names();
    let owned_cell_slot_names = match function.function.kind {
        BlockPyFunctionKind::Function => {
            let mut names = function
                .function
                .semantic
                .owned_cell_storage_names()
                .into_iter()
                .collect::<Vec<_>>();
            names.sort();
            names
        }
        BlockPyFunctionKind::Generator
        | BlockPyFunctionKind::Coroutine
        | BlockPyFunctionKind::AsyncGenerator => function.function.local_cell_slots(),
    };
    let blocks = function
        .function
        .blocks
        .iter()
        .map(|block| {
            let exc_target = function.exc_target_index(block);
            let exc_dispatch = function.exc_dispatch_plan(block, exc_target);
            let runtime_param_names = function.jit_param_names_for_block(block);
            JitBlockInfo {
                runtime_param_names,
                exc_target,
                exc_dispatch,
            }
        })
        .collect();
    JitFunctionInfo {
        entry_params: function.entry_params(),
        entry_param_names: function.function.params.names(),
        entry_param_default_sources: function.entry_param_default_sources(),
        ambient_param_names: function.ambient_param_names.clone(),
        owned_cell_slot_names,
        slot_names,
        blocks,
    }
}

pub fn build_direct_simple_block_plans(
    function: &BlockPyFunction<CodegenBlockPyPass>,
) -> Vec<DirectSimpleBlockPlan> {
    let validated = ValidatedPreparedBbFunction::new(function);
    function
        .blocks
        .iter()
        .map(|block| direct_simple_block_plan_from_block(&validated, block))
        .collect()
}

fn build_clif_plan(
    function: &BlockPyFunction<CodegenBlockPyPass>,
    info: &JitFunctionInfo,
) -> ClifPlan {
    let block_plans = build_direct_simple_block_plans(function);
    let blocks = function
        .blocks
        .iter()
        .zip(info.blocks.iter())
        .zip(block_plans)
        .map(|((block, block_info), plan)| ClifBlockPlan {
            label: block.label.to_string(),
            param_names: block.param_name_vec(),
            runtime_param_names: block_info.runtime_param_names.clone(),
            exc_target: block_info.exc_target,
            exc_dispatch: block_info.exc_dispatch.clone(),
            plan,
        })
        .collect();
    ClifPlan {
        entry_params: info.entry_params.clone(),
        entry_param_names: info.entry_param_names.clone(),
        entry_param_default_sources: info.entry_param_default_sources.clone(),
        ambient_param_names: info.ambient_param_names.clone(),
        owned_cell_slot_names: info.owned_cell_slot_names.clone(),
        slot_names: info.slot_names.clone(),
        blocks,
    }
}

pub fn register_clif_module_plans(
    module_name: &str,
    module: &BlockPyModule<CodegenBlockPyPass>,
) -> Result<(), String> {
    let mut functions = HashMap::new();
    for function in &module.callable_defs {
        let key = PlanKey {
            module: module_name.to_string(),
            function_id: function.function_id.0,
        };
        let info = build_jit_function_info(function);
        functions.insert(
            key,
            RegisteredJitFunction {
                function: function.clone(),
                info,
            },
        );
    }

    let mut function_registry = bb_function_registry()
        .lock()
        .map_err(|_| "failed to lock bb function registry".to_string())?;
    function_registry.retain(|key, _| key.module != module_name);
    function_registry.extend(functions);
    Ok(())
}

pub fn lookup_clif_plan(module_name: &str, function_id: usize) -> Option<ClifPlan> {
    let registry = bb_function_registry().lock().ok()?;
    let registered = registry
        .get(&PlanKey {
            module: module_name.to_string(),
            function_id,
        })
        .cloned()?;
    drop(registry);
    Some(build_clif_plan(&registered.function, &registered.info))
}

pub fn lookup_blockpy_function(
    module_name: &str,
    function_id: usize,
) -> Option<BlockPyFunction<CodegenBlockPyPass>> {
    let registry = bb_function_registry().lock().ok()?;
    registry
        .get(&PlanKey {
            module: module_name.to_string(),
            function_id,
        })
        .map(|registered| registered.function.clone())
}

pub fn lookup_registered_jit_function(
    module_name: &str,
    function_id: usize,
) -> Option<(BlockPyFunction<CodegenBlockPyPass>, JitFunctionInfo)> {
    let registry = bb_function_registry().lock().ok()?;
    registry
        .get(&PlanKey {
            module: module_name.to_string(),
            function_id,
        })
        .map(|registered| (registered.function.clone(), registered.info.clone()))
}

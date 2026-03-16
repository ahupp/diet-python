mod codegen_normalize;
mod codegen_trace;
mod exception_pass;

use super::bb_ir::{BbBlock, BbBlockMeta, BbExpr, BbFunction, BbModule, BbOp, BbTerm};
use super::block_py::cfg::linearize_structured_ifs;
use super::block_py::exception::is_dp_lookup_call;
use super::block_py::state::collect_parameter_names;
use super::block_py::{
    BlockPyBlock, BlockPyIfTerm, BlockPyStmt, BlockPyTerm, CoreBlockPyCallableDef,
};
use super::blockpy_expr_simplify::simplify_blockpy_callable_def_exprs;
use super::cfg_ir::{CfgCallableDef, CfgModule};
use super::function_lowering::rewrite_deleted_name_loads;
use super::param_specs::function_param_specs_expr;
use super::ruff_to_blockpy::{
    build_lowered_blockpy_function_bundle, LoweredBlockPyFunction,
    ResolvedLoweredBlockPyFunctionBundlePlan,
};
use super::stmt_utils::{flatten_stmt_boxes, stmt_body_from_stmts};
use crate::py_expr;
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;
use std::collections::{HashMap, HashSet};

pub use codegen_normalize::normalize_bb_module_for_codegen;
pub use exception_pass::lower_try_jump_exception_flow;

pub type LoweredBlockPyModuleBundle = CfgModule<LoweredBlockPyFunction>;
pub type LoweredCoreBlockPyFunction = LoweredBlockPyFunction<CoreBlockPyCallableDef>;

pub type LoweredCoreBlockPyModuleBundle = CfgModule<LoweredCoreBlockPyFunction>;

pub(crate) struct ResolvedLoweredBlockPyModuleBundlePlanEntry {
    pub bundle_plan: ResolvedLoweredBlockPyFunctionBundlePlan,
    pub main_binding_target: super::bb_ir::BindingTarget,
}

pub(crate) struct ResolvedLoweredBlockPyModuleBundlePlan {
    pub module_init: Option<String>,
    pub callable_def_bundles: Vec<ResolvedLoweredBlockPyModuleBundlePlanEntry>,
}

fn build_lowered_blockpy_function_bundle_with_deleted_name_rewrite(
    plan: ResolvedLoweredBlockPyFunctionBundlePlan,
) -> super::ruff_to_blockpy::LoweredBlockPyFunctionBundle {
    let deleted_names = plan.deleted_names.clone();
    let unbound_local_names = plan.unbound_local_names.clone();
    build_lowered_blockpy_function_bundle(plan, &mut |callable_def| {
        if !deleted_names.is_empty() {
            rewrite_deleted_name_loads(
                &mut callable_def.blocks,
                &deleted_names,
                &unbound_local_names,
            );
        } else if !unbound_local_names.is_empty() {
            rewrite_deleted_name_loads(
                &mut callable_def.blocks,
                &HashSet::new(),
                &unbound_local_names,
            );
        }
    })
}

pub(crate) fn preview_main_lowered_blockpy_function(
    plan: ResolvedLoweredBlockPyFunctionBundlePlan,
) -> LoweredBlockPyFunction {
    build_lowered_blockpy_function_bundle_with_deleted_name_rewrite(plan).main_function
}

pub(crate) fn resolved_lowered_blockpy_module_bundle_plan_to_bundle(
    plan: ResolvedLoweredBlockPyModuleBundlePlan,
) -> LoweredBlockPyModuleBundle {
    let mut callable_defs = Vec::new();
    for entry in plan.callable_def_bundles {
        let bundle =
            build_lowered_blockpy_function_bundle_with_deleted_name_rewrite(entry.bundle_plan);
        callable_defs.extend(
            bundle
                .helper_functions
                .into_iter()
                .map(|helper| helper.with_binding_target(super::bb_ir::BindingTarget::Local)),
        );
        callable_defs.push(
            bundle
                .main_function
                .with_binding_target(entry.main_binding_target),
        );
    }
    LoweredBlockPyModuleBundle {
        module_init: plan.module_init,
        callable_defs,
    }
}

pub fn project_lowered_module_callable_defs<T, U: Clone>(
    module: &CfgModule<T>,
    project: impl Fn(&T) -> &U,
) -> CfgModule<U> {
    module.map_callable_defs(|lowered_function| project(lowered_function).clone())
}

pub(crate) fn simplify_lowered_blockpy_module_bundle_exprs(
    module: &LoweredBlockPyModuleBundle,
) -> LoweredCoreBlockPyModuleBundle {
    module.map_callable_defs(simplify_lowered_blockpy_function_exprs)
}

pub(crate) fn lower_core_blockpy_module_bundle_to_bb_module(
    module: &LoweredCoreBlockPyModuleBundle,
) -> BbModule {
    module.map_callable_defs(lower_core_blockpy_function_to_bb_function)
}

fn simplify_lowered_blockpy_function_exprs(
    lowered: &LoweredBlockPyFunction,
) -> LoweredCoreBlockPyFunction {
    let callable_def = simplify_blockpy_callable_def_exprs(&lowered.callable_def);
    LoweredCoreBlockPyFunction {
        binding_target: lowered.binding_target,
        param_specs: BbExpr::from_expr(function_param_specs_expr(&callable_def.params)),
        callable_def,
        is_coroutine: lowered.is_coroutine,
        bb_kind: lowered.bb_kind.clone(),
        block_params: lowered.block_params.clone(),
        exception_edges: lowered.exception_edges.clone(),
        closure_layout: lowered.closure_layout.clone(),
    }
}

pub(crate) fn lower_core_blockpy_function_to_bb_function(
    lowered: &LoweredCoreBlockPyFunction,
) -> BbFunction {
    let (linear_blocks, linear_block_params, linear_exception_edges) = linearize_structured_ifs(
        &lowered.callable_def.blocks,
        &lowered.block_params,
        &lowered.exception_edges,
    );
    BbFunction {
        cfg: CfgCallableDef {
            function_id: lowered.callable_def.function_id,
            bind_name: lowered.callable_def.bind_name.clone(),
            display_name: lowered.callable_def.display_name.clone(),
            qualname: lowered.callable_def.qualname.clone(),
            kind: lowered.bb_kind.clone(),
            params: collect_parameter_names(&lowered.callable_def.params),
            entry_liveins: lowered.callable_def.entry_liveins.clone(),
            blocks: lower_blockpy_blocks_to_bb_blocks(
                &linear_blocks,
                &linear_block_params,
                &linear_exception_edges,
            ),
        },
        binding_target: lowered.binding_target,
        is_coroutine: lowered.is_coroutine,
        closure_layout: lowered.closure_layout.clone(),
        param_specs: lowered.param_specs.clone(),
        local_cell_slots: lowered.callable_def.local_cell_slots.clone(),
    }
}

pub(crate) fn lower_blockpy_blocks_to_bb_blocks(
    blocks: &[BlockPyBlock<impl Clone + Into<Expr> + From<Expr>>],
    block_params: &HashMap<String, Vec<String>>,
    exception_edges: &HashMap<String, Option<String>>,
) -> Vec<BbBlock> {
    let block_exc_params = blocks
        .iter()
        .map(|block| {
            (
                block.label.as_str().to_string(),
                block.meta.exc_param.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    blocks
        .iter()
        .map(|block| {
            let current_exception_name = block.meta.exc_param.as_deref();
            let mut normalized_body_stmt = stmt_body_from_stmts(
                block
                    .body
                    .iter()
                    .filter_map(blockpy_stmt_to_stmt_for_analysis)
                    .collect::<Vec<_>>(),
            );
            if let Some(exc_name) = current_exception_name {
                rewrite_current_exception_in_stmt_body(&mut normalized_body_stmt, exc_name);
            }
            let mut normalized_body = flatten_stmt_boxes(&normalized_body_stmt.body)
                .into_iter()
                .map(|stmt| *stmt)
                .collect::<Vec<_>>();
            let mut normalized_term = block.term.clone();
            if let Some(exc_name) = current_exception_name {
                rewrite_current_exception_in_blockpy_term(&mut normalized_term, exc_name);
            }
            let exc_target_label = exception_edges.get(block.label.as_str()).cloned().flatten();
            let exc_name = exc_target_label.as_ref().and_then(|target_label| {
                block_exc_params
                    .get(target_label.as_str())
                    .cloned()
                    .flatten()
                    .or_else(|| {
                        block_params
                            .get(target_label.as_str())
                            .and_then(|params| exception_param_from_block_params(params))
                    })
            });
            let mut local_defs = Vec::new();
            let mut ops = Vec::new();
            for stmt in normalized_body {
                match stmt {
                    Stmt::FunctionDef(func_def)
                        if func_def.name.id.as_str().starts_with("_dp_bb_") =>
                    {
                        local_defs.push(func_def);
                    }
                    other => {
                        if let Some(op) = BbOp::from_stmt(other) {
                            ops.push(op);
                        }
                    }
                }
            }
            let mut params = block_params
                .get(block.label.as_str())
                .cloned()
                .unwrap_or_default();
            if let Some(exc_param) = block.meta.exc_param.as_ref() {
                if !params.iter().any(|param| param == exc_param) {
                    params.push(exc_param.clone());
                }
            }
            BbBlock {
                label: block.label.as_str().to_string(),
                body: ops,
                term: bb_term_from_blockpy_term(&normalized_term),
                meta: BbBlockMeta {
                    params,
                    local_defs,
                    exc_target_label,
                    exc_name,
                },
            }
        })
        .collect()
}

fn exception_param_from_block_params(params: &[String]) -> Option<String> {
    params.iter().find_map(|name| {
        (name.starts_with("_dp_try_exc_") || name.starts_with("_dp_uncaught_exc_"))
            .then(|| name.clone())
    })
}

fn rewrite_current_exception_in_stmt_body(body: &mut ast::StmtBody, exc_name: &str) {
    CurrentExceptionTransformer { exc_name }.visit_body(body);
}

fn rewrite_current_exception_in_blockpy_term<E>(term: &mut BlockPyTerm<E>, exc_name: &str)
where
    E: Clone + Into<Expr> + From<Expr>,
{
    match term {
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            rewrite_current_exception_in_blockpy_expr(test, exc_name);
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_current_exception_in_blockpy_expr(&mut branch.index, exc_name);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                rewrite_current_exception_in_blockpy_expr(exc, exc_name);
            } else {
                raise_stmt.exc = Some(current_exception_name_expr(exc_name).into());
            }
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value.as_mut() {
                rewrite_current_exception_in_blockpy_expr(value, exc_name);
            }
        }
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
    }
}

fn rewrite_current_exception_in_blockpy_expr<E>(expr: &mut E, exc_name: &str)
where
    E: Clone + Into<Expr> + From<Expr>,
{
    let mut raw: Expr = expr.clone().into();
    rewrite_current_exception_in_expr(&mut raw, exc_name);
    *expr = raw.into();
}

fn rewrite_current_exception_in_expr(expr: &mut Expr, exc_name: &str) {
    CurrentExceptionTransformer { exc_name }.visit_expr(expr);
}

struct CurrentExceptionTransformer<'a> {
    exc_name: &'a str,
}

impl Transformer for CurrentExceptionTransformer<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        walk_expr(self, expr);
        if is_current_exception_call(expr) {
            *expr = current_exception_name_expr(self.exc_name);
        } else if is_exc_info_call(expr) {
            *expr = current_exception_info_expr(self.exc_name);
        }
    }
}

fn is_current_exception_call(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    call.arguments.args.is_empty()
        && call.arguments.keywords.is_empty()
        && is_dp_lookup_call(call.func.as_ref(), "current_exception")
}

fn is_exc_info_call(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    call.arguments.args.is_empty()
        && call.arguments.keywords.is_empty()
        && is_dp_lookup_call(call.func.as_ref(), "exc_info")
}

fn current_exception_name_expr(exc_name: &str) -> Expr {
    Expr::Name(ast::ExprName {
        id: exc_name.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    })
}

fn current_exception_info_expr(exc_name: &str) -> Expr {
    py_expr!(
        "__dp_exc_info_from_exception({exc:expr})",
        exc = current_exception_name_expr(exc_name),
    )
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

pub(crate) fn blockpy_stmt_to_stmt_for_analysis<E>(stmt: &BlockPyStmt<E>) -> Option<Stmt>
where
    E: Clone + Into<Expr>,
{
    match stmt {
        BlockPyStmt::Assign(assign) => Some(Stmt::Assign(ast::StmtAssign {
            node_index: compat_node_index(),
            range: compat_range(),
            targets: vec![Expr::Name(assign.target.clone())],
            value: Box::new(assign.value.clone().into()),
        })),
        BlockPyStmt::Expr(expr) => Some(Stmt::Expr(ast::StmtExpr {
            node_index: compat_node_index(),
            range: compat_range(),
            value: Box::new(expr.clone().into()),
        })),
        BlockPyStmt::Delete(delete) => Some(Stmt::Delete(ast::StmtDelete {
            node_index: compat_node_index(),
            range: compat_range(),
            targets: vec![Expr::Name(delete.target.clone())],
        })),
        BlockPyStmt::If(if_stmt) => Some(Stmt::If(ast::StmtIf {
            node_index: compat_node_index(),
            range: compat_range(),
            test: Box::new(if_stmt.test.clone().into()),
            body: stmt_body_from_blockpy_fragment(&if_stmt.body),
            elif_else_clauses: if if_stmt.orelse.body.is_empty() && if_stmt.orelse.term.is_none() {
                Vec::new()
            } else {
                vec![ast::ElifElseClause {
                    node_index: compat_node_index(),
                    range: compat_range(),
                    test: None,
                    body: stmt_body_from_blockpy_fragment(&if_stmt.orelse),
                }]
            },
        })),
    }
}

fn bb_term_from_blockpy_term<E>(terminal: &BlockPyTerm<E>) -> BbTerm
where
    E: Clone + Into<Expr>,
{
    match terminal {
        BlockPyTerm::Jump(target) => BbTerm::Jump(target.as_str().to_string()),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BbTerm::BrIf {
            test: BbExpr::from_expr(test.clone().into()),
            then_label: then_label.as_str().to_string(),
            else_label: else_label.as_str().to_string(),
        },
        BlockPyTerm::BranchTable(branch) => BbTerm::BrTable {
            index: BbExpr::from_expr(branch.index.clone().into()),
            targets: branch
                .targets
                .iter()
                .map(|label| label.as_str().to_string())
                .collect(),
            default_label: branch.default_label.as_str().to_string(),
        },
        BlockPyTerm::Raise(raise_stmt) => BbTerm::Raise {
            exc: raise_stmt
                .exc
                .as_ref()
                .map(|exc| BbExpr::from_expr(exc.clone().into())),
            cause: None,
        },
        BlockPyTerm::TryJump(try_jump) => BbTerm::Jump(try_jump.body_label.as_str().to_string()),
        BlockPyTerm::Return(value) => {
            BbTerm::Ret(value.clone().map(|expr| BbExpr::from_expr(expr.into())))
        }
    }
}

fn stmt_body_from_blockpy_fragment<E>(
    fragment: &super::block_py::BlockPyCfgFragment<
        super::block_py::BlockPyStmt<E>,
        super::block_py::BlockPyTerm<E>,
    >,
) -> ast::StmtBody
where
    E: Clone + Into<Expr>,
{
    let mut stmts = fragment
        .body
        .iter()
        .filter_map(blockpy_stmt_to_stmt_for_analysis)
        .collect::<Vec<_>>();
    if let Some(term) = &fragment.term {
        if let Some(stmt) = blockpy_term_to_stmt_for_analysis(term) {
            stmts.push(stmt);
        }
    }
    stmt_body_from_stmts(stmts)
}

fn blockpy_term_to_stmt_for_analysis<E>(term: &BlockPyTerm<E>) -> Option<Stmt>
where
    E: Clone + Into<Expr>,
{
    match term {
        BlockPyTerm::Return(value) => Some(Stmt::Return(ast::StmtReturn {
            node_index: compat_node_index(),
            range: compat_range(),
            value: value.clone().map(|value| Box::new(value.into())),
        })),
        BlockPyTerm::Raise(raise_stmt) => Some(Stmt::Raise(ast::StmtRaise {
            node_index: compat_node_index(),
            range: compat_range(),
            exc: raise_stmt.exc.clone().map(|exc| Box::new(exc.into())),
            cause: None,
        })),
        BlockPyTerm::Jump(_)
        | BlockPyTerm::IfTerm(_)
        | BlockPyTerm::BranchTable(_)
        | BlockPyTerm::TryJump(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::basic_block::block_py::cfg::linearize_structured_ifs;
    use crate::basic_block::block_py::{
        BlockPyAssign, BlockPyBlock, BlockPyIf, BlockPyIfTerm, BlockPyLabel, BlockPyStmt,
        BlockPyStmtFragment, BlockPyTerm,
    };
    use crate::py_expr;
    use ruff_python_ast::Expr;
    use std::collections::HashMap;

    fn name_expr(name: &str) -> ruff_python_ast::ExprName {
        let Expr::Name(name_expr) = py_expr!("{name:id}", name = name) else {
            unreachable!();
        };
        name_expr
    }

    #[test]
    fn linearizes_structured_if_stmt_into_explicit_blocks() {
        let block: BlockPyBlock<Expr> = BlockPyBlock {
            label: BlockPyLabel::from("start"),
            body: vec![
                BlockPyStmt::Assign(BlockPyAssign {
                    target: name_expr("x"),
                    value: py_expr!("a").into(),
                }),
                BlockPyStmt::If(BlockPyIf {
                    test: py_expr!("cond").into(),
                    body: BlockPyStmtFragment::from_stmts(vec![BlockPyStmt::Assign(
                        BlockPyAssign {
                            target: name_expr("x"),
                            value: py_expr!("b").into(),
                        },
                    )]),
                    orelse: BlockPyStmtFragment::from_stmts(vec![BlockPyStmt::Assign(
                        BlockPyAssign {
                            target: name_expr("x"),
                            value: py_expr!("c").into(),
                        },
                    )]),
                }),
                BlockPyStmt::Expr(py_expr!("sink(x)").into()),
            ],
            term: BlockPyTerm::Return(None),
            meta: Default::default(),
        };

        let (blocks, _, _): (
            Vec<BlockPyBlock<Expr>>,
            HashMap<String, Vec<String>>,
            HashMap<String, Option<String>>,
        ) = linearize_structured_ifs(&[block], &HashMap::new(), &HashMap::new());

        assert_eq!(blocks.len(), 4, "{blocks:?}");
        assert!(matches!(
            blocks[0].term,
            BlockPyTerm::IfTerm(BlockPyIfTerm { .. })
        ));
        assert!(!blocks.iter().any(|block| {
            block
                .body
                .iter()
                .any(|stmt| matches!(stmt, BlockPyStmt::If(_)))
        }));
    }
}

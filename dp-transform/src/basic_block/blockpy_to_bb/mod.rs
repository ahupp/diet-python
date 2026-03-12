mod codegen_normalize;
mod codegen_trace;
mod exception_pass;

use super::bb_ir::{BbBlock, BbExpr, BbFunction, BbModule, BbOp, BbTerm, BindingTarget};
use super::block_py::exception::is_dp_lookup_call;
use super::block_py::state::collect_parameter_names;
use super::block_py::{BlockPyBlock, BlockPyIfTerm, BlockPyModule, BlockPyStmt, BlockPyTerm};
use super::ruff_to_blockpy::{LoweredBlockPyFunction, LoweredBlockPyFunctionBundle};
use super::stmt_utils::{flatten_stmt, flatten_stmt_boxes, stmt_body_from_stmts};
use crate::basic_block::ast_to_ast::ast_rewrite::rewrite_with_pass;
use crate::basic_block::ast_to_ast::ast_rewrite::ExprRewritePass;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_stmt;
use crate::driver::SimplifyExprPass;
use crate::py_expr;
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;
use std::collections::{HashMap, VecDeque};

pub(crate) use codegen_normalize::normalize_bb_module_for_codegen;
pub(crate) use exception_pass::lower_try_jump_exception_flow;

pub(crate) struct LoweredBbFunctionBundle {
    pub main_function: BbFunction,
    pub helper_functions: Vec<BbFunction>,
}

pub(crate) struct LoweredBlockPyModuleCallableDef {
    pub bundle: LoweredBlockPyFunctionBundle,
    pub main_binding_target: BindingTarget,
}

pub(crate) struct LoweredBlockPyModuleBundle {
    pub callable_defs: Vec<LoweredBlockPyModuleCallableDef>,
    pub module_init: Option<String>,
}

pub(crate) fn push_lowered_blockpy_callable_def_bundle(
    out: &mut LoweredBlockPyModuleBundle,
    bundle: LoweredBlockPyFunctionBundle,
    main_binding_target: BindingTarget,
) {
    out.callable_defs.push(LoweredBlockPyModuleCallableDef {
        bundle,
        main_binding_target,
    });
}

pub(crate) fn lowered_blockpy_module_bundle_to_blockpy_module(
    module: &LoweredBlockPyModuleBundle,
) -> BlockPyModule {
    let mut callable_defs = Vec::new();
    for lowered_function in &module.callable_defs {
        callable_defs.push(lowered_function.bundle.main_function.callable_def.clone());
        callable_defs.extend(
            lowered_function
                .bundle
                .helper_functions
                .iter()
                .map(|helper| helper.callable_def.clone()),
        );
    }
    BlockPyModule {
        callable_defs,
        module_init: module.module_init.clone(),
    }
}

pub(crate) fn lower_blockpy_module_bundle_to_bb_module(
    context: &Context,
    module: &LoweredBlockPyModuleBundle,
) -> BbModule {
    let mut out = Vec::new();
    for lowered_function in &module.callable_defs {
        let lowered = lower_blockpy_function_bundle_to_bb_functions(
            context,
            &lowered_function.bundle,
            lowered_function.main_binding_target,
        );
        out.extend(lowered.helper_functions);
        out.push(lowered.main_function);
    }
    BbModule {
        functions: out,
        module_init: module.module_init.clone(),
    }
}

pub(crate) fn lower_blockpy_function_bundle_to_bb_functions(
    context: &Context,
    bundle: &LoweredBlockPyFunctionBundle,
    main_binding_target: BindingTarget,
) -> LoweredBbFunctionBundle {
    LoweredBbFunctionBundle {
        main_function: lower_blockpy_function_to_bb_function(
            context,
            &bundle.main_function,
            Some(main_binding_target),
        ),
        helper_functions: bundle
            .helper_functions
            .iter()
            .map(|helper| lower_blockpy_function_to_bb_function(context, helper, None))
            .collect(),
    }
}

pub(crate) fn lower_blockpy_function_to_bb_function(
    context: &Context,
    lowered: &LoweredBlockPyFunction,
    binding_target_override: Option<BindingTarget>,
) -> BbFunction {
    BbFunction {
        function_id: lowered.callable_def.function_id,
        bind_name: lowered.callable_def.bind_name.clone(),
        display_name: lowered.callable_def.display_name.clone(),
        qualname: lowered.callable_def.qualname.clone(),
        binding_target: binding_target_override.unwrap_or(BindingTarget::Local),
        is_coroutine: lowered.is_coroutine,
        kind: lowered.bb_kind.clone(),
        entry: lowered.callable_def.entry_label().to_string(),
        param_names: collect_parameter_names(&lowered.callable_def.params),
        entry_liveins: lowered.callable_def.entry_liveins.clone(),
        closure_layout: lowered.closure_layout.clone(),
        param_specs: lowered.param_specs.clone(),
        local_cell_slots: lowered.callable_def.local_cell_slots.clone(),
        blocks: lower_blockpy_blocks_to_bb_blocks(
            context,
            &lowered.callable_def.blocks,
            &lowered.block_params,
            &lowered.exception_edges,
        ),
    }
}

pub(crate) fn lower_blockpy_blocks_to_bb_blocks(
    context: &Context,
    blocks: &[BlockPyBlock],
    block_params: &HashMap<String, Vec<String>>,
    exception_edges: &HashMap<String, Option<String>>,
) -> Vec<BbBlock> {
    let simplify_expr_pass = SimplifyExprPass;
    let block_exc_params = blocks
        .iter()
        .map(|block| (block.label.as_str().to_string(), block.exc_param.clone()))
        .collect::<HashMap<_, _>>();
    blocks
        .iter()
        .map(|block| {
            let current_exception_name = block.exc_param.as_deref();
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
            rewrite_with_pass(
                context,
                None,
                Some(&simplify_expr_pass),
                &mut normalized_body_stmt,
            );
            let mut normalized_body = flatten_stmt_boxes(&normalized_body_stmt.body)
                .into_iter()
                .map(|stmt| *stmt)
                .collect::<Vec<_>>();
            let mut normalized_term = block.term.clone();
            if let Some(exc_name) = current_exception_name {
                rewrite_current_exception_in_blockpy_term(&mut normalized_term, exc_name);
            }
            simplify_blockpy_terminal_exprs(
                context,
                &simplify_expr_pass,
                &mut normalized_term,
                &mut normalized_body,
            );
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
            let mut pending = VecDeque::from(normalized_body);
            while let Some(stmt) = pending.pop_front() {
                match stmt {
                    Stmt::FunctionDef(func_def)
                        if func_def.name.id.as_str().starts_with("_dp_bb_") =>
                    {
                        local_defs.push(func_def);
                    }
                    Stmt::Assign(assign)
                        if rewrite_stmt::assign_del::should_rewrite_targets(&assign.targets) =>
                    {
                        let rewritten = rewrite_stmt::assign_del::rewrite_assign(context, assign);
                        let rewritten_stmt = match rewritten {
                            crate::basic_block::ast_to_ast::ast_rewrite::Rewrite::Unmodified(
                                stmt,
                            )
                            | crate::basic_block::ast_to_ast::ast_rewrite::Rewrite::Walk(stmt) => {
                                stmt
                            }
                        };
                        let mut lowered = Vec::new();
                        flatten_stmt(&rewritten_stmt, &mut lowered);
                        for lowered_stmt in lowered.into_iter().rev() {
                            pending.push_front(*lowered_stmt);
                        }
                    }
                    other => {
                        if let Some(op) = BbOp::from_stmt(other) {
                            ops.push(op);
                        }
                    }
                }
            }
            BbBlock {
                label: block.label.as_str().to_string(),
                params: {
                    let mut params = block_params
                        .get(block.label.as_str())
                        .cloned()
                        .unwrap_or_default();
                    if let Some(exc_param) = block.exc_param.as_ref() {
                        if !params.iter().any(|param| param == exc_param) {
                            params.push(exc_param.clone());
                        }
                    }
                    params
                },
                local_defs,
                ops,
                exc_target_label,
                exc_name,
                term: bb_term_from_blockpy_term(&normalized_term),
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

fn rewrite_current_exception_in_blockpy_term(term: &mut BlockPyTerm, exc_name: &str) {
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

fn rewrite_current_exception_in_blockpy_expr(
    expr: &mut super::block_py::BlockPyExpr,
    exc_name: &str,
) {
    expr.rewrite_mut(|expr| rewrite_current_exception_in_expr(expr, exc_name));
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

fn simplify_expr_for_bb_term(
    context: &Context,
    pass: &SimplifyExprPass,
    expr: &mut Expr,
    body: &mut Vec<Stmt>,
) {
    let lowered = pass.lower_expr(context, expr.clone());
    if lowered.modified {
        let mut lowered_stmts = Vec::new();
        flatten_stmt(&lowered.stmt, &mut lowered_stmts);
        body.extend(lowered_stmts.into_iter().map(|stmt| *stmt));
    }
    *expr = lowered.expr;
}

fn simplify_blockpy_terminal_exprs(
    context: &Context,
    pass: &SimplifyExprPass,
    terminal: &mut BlockPyTerm,
    body: &mut Vec<Stmt>,
) {
    match terminal {
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            test.rewrite_mut(|expr| simplify_expr_for_bb_term(context, pass, expr, body))
        }
        BlockPyTerm::BranchTable(branch) => branch
            .index
            .rewrite_mut(|expr| simplify_expr_for_bb_term(context, pass, expr, body)),
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                exc.rewrite_mut(|expr| simplify_expr_for_bb_term(context, pass, expr, body));
            }
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value.as_mut() {
                value.rewrite_mut(|expr| simplify_expr_for_bb_term(context, pass, expr, body));
            }
        }
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
    }
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

pub(crate) fn blockpy_stmt_to_stmt_for_analysis(stmt: &BlockPyStmt) -> Option<Stmt> {
    match stmt {
        BlockPyStmt::Pass => Some(Stmt::Pass(ast::StmtPass {
            node_index: compat_node_index(),
            range: compat_range(),
        })),
        BlockPyStmt::Assign(assign) => Some(Stmt::Assign(ast::StmtAssign {
            node_index: compat_node_index(),
            range: compat_range(),
            targets: vec![Expr::Name(assign.target.clone())],
            value: Box::new(assign.value.to_expr()),
        })),
        BlockPyStmt::Expr(expr) => Some(Stmt::Expr(ast::StmtExpr {
            node_index: compat_node_index(),
            range: compat_range(),
            value: Box::new(expr.to_expr()),
        })),
        BlockPyStmt::Delete(delete) => Some(Stmt::Delete(ast::StmtDelete {
            node_index: compat_node_index(),
            range: compat_range(),
            targets: vec![Expr::Name(delete.target.clone())],
        })),
        BlockPyStmt::If(if_stmt) => Some(Stmt::If(ast::StmtIf {
            node_index: compat_node_index(),
            range: compat_range(),
            test: Box::new(if_stmt.test.to_expr()),
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

fn bb_term_from_blockpy_term(terminal: &BlockPyTerm) -> BbTerm {
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

fn stmt_body_from_blockpy_fragment(
    fragment: &super::block_py::BlockPyStmtFragment,
) -> ast::StmtBody {
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

fn blockpy_term_to_stmt_for_analysis(term: &BlockPyTerm) -> Option<Stmt> {
    match term {
        BlockPyTerm::Return(value) => Some(Stmt::Return(ast::StmtReturn {
            node_index: compat_node_index(),
            range: compat_range(),
            value: value.clone().map(|value| Box::new(value.to_expr())),
        })),
        BlockPyTerm::Raise(raise_stmt) => Some(Stmt::Raise(ast::StmtRaise {
            node_index: compat_node_index(),
            range: compat_range(),
            exc: raise_stmt.exc.clone().map(|exc| Box::new(exc.to_expr())),
            cause: None,
        })),
        BlockPyTerm::Jump(_)
        | BlockPyTerm::IfTerm(_)
        | BlockPyTerm::BranchTable(_)
        | BlockPyTerm::TryJump(_) => None,
    }
}

use super::{
    flatten_stmt, BbExpr, BbFunctionKind, BbTerm, Block, Context, Expr, LoweredKind,
    SimplifyExprPass, Stmt, Terminator,
};
use crate::py_stmt;
use crate::transform::ast_rewrite::ExprRewritePass;
use ruff_python_ast::StmtRaise;
use std::collections::HashMap;

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

pub(super) fn simplify_terminator_exprs(
    context: &Context,
    pass: &SimplifyExprPass,
    terminator: &mut Terminator,
    body: &mut Vec<Stmt>,
) {
    match terminator {
        Terminator::BrIf { test, .. } => simplify_expr_for_bb_term(context, pass, test, body),
        Terminator::BrTable { index, .. } => {
            simplify_expr_for_bb_term(context, pass, index, body);
        }
        Terminator::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                simplify_expr_for_bb_term(context, pass, exc, body);
            }
            if let Some(cause) = raise_stmt.cause.as_mut() {
                simplify_expr_for_bb_term(context, pass, cause, body);
            }
        }
        Terminator::Yield { value, .. } => {
            if let Some(value) = value.as_mut() {
                simplify_expr_for_bb_term(context, pass, value, body);
            }
        }
        Terminator::Ret(value) => {
            if let Some(value) = value.as_mut() {
                simplify_expr_for_bb_term(context, pass, value, body);
            }
        }
        Terminator::Jump(_) | Terminator::TryJump { .. } => {}
    }
}

pub(super) fn bb_term_from_terminator(terminator: &Terminator) -> BbTerm {
    match terminator {
        Terminator::Jump(target) => BbTerm::Jump(target.clone()),
        Terminator::BrIf {
            test,
            then_label,
            else_label,
        } => BbTerm::BrIf {
            test: BbExpr::from_expr(test.clone()),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        },
        Terminator::BrTable {
            index,
            targets,
            default_label,
        } => BbTerm::BrTable {
            index: BbExpr::from_expr(index.clone()),
            targets: targets.clone(),
            default_label: default_label.clone(),
        },
        Terminator::Raise(raise_stmt) => BbTerm::Raise {
            exc: raise_stmt
                .exc
                .as_ref()
                .map(|expr| BbExpr::from_expr((**expr).clone())),
            cause: raise_stmt
                .cause
                .as_ref()
                .map(|expr| BbExpr::from_expr((**expr).clone())),
        },
        Terminator::TryJump {
            body_label,
            except_label,
            except_exc_name,
            body_region_labels,
            except_region_labels,
            finally_label,
            finally_exc_name,
            finally_region_labels,
            finally_fallthrough_label,
        } => BbTerm::TryJump {
            body_label: body_label.clone(),
            except_label: except_label.clone(),
            except_exc_name: except_exc_name.clone(),
            body_region_labels: body_region_labels.clone(),
            except_region_labels: except_region_labels.clone(),
            finally_label: finally_label.clone(),
            finally_exc_name: finally_exc_name.clone(),
            finally_region_labels: finally_region_labels.clone(),
            finally_fallthrough_label: finally_fallthrough_label.clone(),
        },
        Terminator::Yield { .. } => {
            panic!("internal error: Terminator::Yield must be lowered before BB IR export")
        }
        Terminator::Ret(value) => BbTerm::Ret(value.clone().map(BbExpr::from_expr)),
    }
}

fn raise_done_stmt(is_async: bool, value: Option<Expr>) -> StmtRaise {
    if is_async {
        match py_stmt!("raise StopAsyncIteration()") {
            Stmt::Raise(stmt) => stmt,
            _ => unreachable!("expected raise statement"),
        }
    } else if let Some(value) = value {
        match py_stmt!("raise StopIteration({value:expr})", value = value) {
            Stmt::Raise(stmt) => stmt,
            _ => unreachable!("expected raise statement"),
        }
    } else {
        match py_stmt!("raise StopIteration()") {
            Stmt::Raise(stmt) => stmt,
            _ => unreachable!("expected raise statement"),
        }
    }
}

pub(super) fn lower_generator_yield_terms_to_explicit_return(
    blocks: &mut [Block],
    block_params: &HashMap<String, Vec<String>>,
    resume_pcs: &[(String, usize)],
    is_async: bool,
) {
    let resume_pc_by_label = resume_pcs
        .iter()
        .cloned()
        .collect::<HashMap<String, usize>>();

    // Existing Ret terminators in generator functions represent completion.
    // Rewrite them to explicit completion exceptions so Ret can represent
    // suspension value returns uniformly.
    for block in blocks.iter_mut() {
        if let Terminator::Ret(value) = &block.terminator {
            block.body.push(py_stmt!(
                "__dp_setattr(_dp_self, \"_pc\", __dp__._GEN_PC_DONE)"
            ));
            block.terminator = Terminator::Raise(raise_done_stmt(is_async, value.clone()));
        }
    }

    // Rewrite yield terminators to explicit state updates plus Ret(value).
    for block in blocks.iter_mut() {
        let (yield_value, resume_label) = match &block.terminator {
            Terminator::Yield {
                value,
                resume_label,
            } => (value.clone(), resume_label.clone()),
            _ => continue,
        };
        let next_pc = *resume_pc_by_label
            .get(resume_label.as_str())
            .unwrap_or_else(|| panic!("missing resume pc for label: {resume_label}"));
        block.body.push(py_stmt!(
            "__dp_setattr(_dp_self, \"_pc\", {next_pc:literal})",
            next_pc = next_pc as i64,
        ));
        let next_state_names = block_params
            .get(resume_label.as_str())
            .cloned()
            .unwrap_or_default();
        for name in next_state_names {
            if matches!(
                name.as_str(),
                "_dp_self" | "_dp_send_value" | "_dp_resume_exc"
            ) {
                continue;
            }
            block.body.push(py_stmt!(
                "__dp_store_local(_dp_self, {name:literal}, {value:id})",
                name = name.as_str(),
                value = name.as_str(),
            ));
        }
        block.terminator = Terminator::Ret(yield_value);
    }
}

pub(super) fn bb_function_kind_from(kind: &LoweredKind) -> BbFunctionKind {
    match kind {
        LoweredKind::Function => BbFunctionKind::Function,
        LoweredKind::Generator {
            resume_label,
            target_labels,
            resume_pcs,
        } => BbFunctionKind::Generator {
            resume_label: resume_label.clone(),
            target_labels: target_labels.clone(),
            resume_pcs: resume_pcs.clone(),
        },
        LoweredKind::AsyncGenerator {
            resume_label,
            target_labels,
            resume_pcs,
        } => BbFunctionKind::AsyncGenerator {
            resume_label: resume_label.clone(),
            target_labels: target_labels.clone(),
            resume_pcs: resume_pcs.clone(),
        },
    }
}

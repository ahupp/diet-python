use super::dataflow::analyze_blockpy_use_def;
use super::{
    BlockPyBlock, BlockPyBranchTable, BlockPyCfgFragment, BlockPyIf, BlockPyIfTerm, BlockPyRaise,
    BlockPyStmt, BlockPyTerm, Expr,
};
use crate::basic_block::ast_symbol_analysis::{assigned_names_in_stmt, collect_assigned_names};
use crate::basic_block::ast_to_ast::scope::cell_name;
use crate::py_stmt;
use crate::transformer::Transformer;
use ruff_python_ast::{self as ast, Stmt};
use std::collections::HashSet;

pub(crate) fn collect_parameter_names(parameters: &ast::Parameters) -> Vec<String> {
    let mut names = Vec::new();
    for param in &parameters.posonlyargs {
        names.push(param.parameter.name.id.to_string());
    }
    for param in &parameters.args {
        names.push(param.parameter.name.id.to_string());
    }
    if let Some(vararg) = &parameters.vararg {
        names.push(vararg.name.id.to_string());
    }
    for param in &parameters.kwonlyargs {
        names.push(param.parameter.name.id.to_string());
    }
    if let Some(kwarg) = &parameters.kwarg {
        names.push(kwarg.name.id.to_string());
    }
    names
}

pub(crate) fn collect_state_vars<E>(
    param_names: &[String],
    blocks: &[BlockPyBlock<E>],
) -> Vec<String>
where
    E: Clone + Into<Expr>,
{
    let mut defs_anywhere = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            defs_anywhere.extend(assigned_names_in_blockpy_stmt(stmt));
        }
        defs_anywhere.extend(assigned_names_in_blockpy_term(&block.term));
    }

    let mut state = param_names.to_vec();
    for block in blocks {
        let (uses, defs) = analyze_blockpy_use_def(block);
        let mut names = defs.into_iter().collect::<Vec<_>>();
        for name in uses {
            let is_special_runtime_state = name == "_dp_self"
                || name.starts_with("_dp_cell_")
                || name.starts_with("_dp_try_exc_")
                || name == "_dp_classcell";
            let is_known_local = defs_anywhere.contains(name.as_str())
                || param_names.iter().any(|param| param == &name);
            let include = is_special_runtime_state || is_known_local;
            if include {
                names.push(name);
            }
        }
        names.sort();
        names.dedup();
        for name in names {
            if !state.iter().any(|existing| existing == &name) {
                state.push(name);
            }
        }
    }
    state
}

pub(crate) fn collect_cell_slots(stmts: &[Stmt]) -> HashSet<String> {
    let mut slots = HashSet::new();
    for stmt in stmts {
        let mut names: HashSet<String> = assigned_names_in_stmt(stmt);
        for name in names.drain() {
            if name.starts_with("_dp_cell_") {
                slots.insert(name);
            }
        }
    }
    slots
}

pub(crate) fn sync_target_cells_stmts(target: &Expr, cell_slots: &HashSet<String>) -> Vec<Stmt> {
    let mut names: HashSet<String> = HashSet::new();
    collect_assigned_names(target, &mut names);
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();

    names
        .into_iter()
        .filter_map(|name| {
            let cell = cell_name(name.as_str());
            if !cell_slots.contains(cell.as_str()) {
                return None;
            }
            Some(py_stmt!(
                "__dp_store_cell({cell:id}, {value:id})",
                cell = cell.as_str(),
                value = name.as_str(),
            ))
        })
        .collect()
}

fn assigned_names_in_blockpy_stmt<E>(stmt: &BlockPyStmt<E>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    match stmt {
        BlockPyStmt::Delete(_) => HashSet::new(),
        BlockPyStmt::Assign(assign) => {
            let mut names = HashSet::from([assign.target.id.to_string()]);
            collect_named_expr_target_names_in_blockpy_expr(&assign.value, &mut names);
            names
        }
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(test, &mut names);
            names.extend(assigned_names_in_blockpy_stmt_fragment(body));
            names.extend(assigned_names_in_blockpy_stmt_fragment(orelse));
            names
        }
        BlockPyStmt::Expr(expr) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(expr, &mut names);
            names
        }
    }
}

fn assigned_names_in_blockpy_term<E>(term: &BlockPyTerm<E>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => HashSet::new(),
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(test, &mut names);
            names
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(index, &mut names);
            names
        }
        BlockPyTerm::Return(value) => {
            let mut names = HashSet::new();
            if let Some(value) = value {
                collect_named_expr_target_names_in_blockpy_expr(value, &mut names);
            }
            names
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            let mut names = HashSet::new();
            if let Some(exc) = exc {
                collect_named_expr_target_names_in_blockpy_expr(exc, &mut names);
            }
            names
        }
    }
}

fn assigned_names_in_blockpy_stmt_fragment<E>(
    fragment: &BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>,
) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    let mut out = HashSet::new();
    for stmt in &fragment.body {
        out.extend(assigned_names_in_blockpy_stmt(stmt));
    }
    if let Some(term) = &fragment.term {
        out.extend(assigned_names_in_blockpy_term(term));
    }
    out
}

fn collect_named_expr_targets(expr: &Expr, names: &mut HashSet<String>) {
    #[derive(Default)]
    struct NamedExprTargetCollector {
        names: HashSet<String>,
    }

    impl crate::transformer::Transformer for NamedExprTargetCollector {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if let Expr::Named(ast::ExprNamed { target, value, .. }) = expr {
                collect_assigned_names(target.as_ref(), &mut self.names);
                self.visit_expr(value.as_mut());
                return;
            }
            crate::transformer::walk_expr(self, expr);
        }
    }

    let mut expr = expr.clone();
    let mut collector = NamedExprTargetCollector::default();
    collector.visit_expr(&mut expr);
    names.extend(collector.names);
}

fn collect_named_expr_target_names_in_blockpy_expr<E>(expr: &E, names: &mut HashSet<String>)
where
    E: Clone + Into<Expr>,
{
    collect_named_expr_targets(&expr.clone().into(), names);
}

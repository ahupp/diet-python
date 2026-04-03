use super::{
    BlockPyBranchTable, BlockPyCfgFragment, BlockPyIfTerm, BlockPyRaise, BlockPySemanticExprNode,
    BlockPyTerm, Instr, StructuredIf, StructuredInstr,
};
use std::collections::HashSet;

pub(super) fn assigned_names_in_linear_blockpy_stmt<E>(stmt: &E) -> HashSet<String>
where
    E: BlockPySemanticExprNode + Instr,
{
    let mut names = HashSet::new();
    collect_named_expr_target_names_in_blockpy_expr(stmt, &mut names);
    names
}

pub(super) fn assigned_names_in_blockpy_stmt<E>(stmt: &StructuredInstr<E>) -> HashSet<String>
where
    E: BlockPySemanticExprNode + Instr,
{
    match stmt {
        StructuredInstr::Expr(expr) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(expr, &mut names);
            names
        }
        StructuredInstr::If(StructuredIf { test, body, orelse }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(test, &mut names);
            names.extend(assigned_names_in_blockpy_fragment(body));
            names.extend(assigned_names_in_blockpy_fragment(orelse));
            names
        }
    }
}

pub(super) fn assigned_names_in_blockpy_stmts<E>(stmts: &[StructuredInstr<E>]) -> HashSet<String>
where
    E: BlockPySemanticExprNode + Instr,
{
    let mut out = HashSet::new();
    for stmt in stmts {
        out.extend(assigned_names_in_blockpy_stmt(stmt));
    }
    out
}

pub(super) fn assigned_names_in_blockpy_term<E>(term: &BlockPyTerm<E>) -> HashSet<String>
where
    E: BlockPySemanticExprNode + Instr,
{
    match term {
        BlockPyTerm::Jump(_) => HashSet::new(),
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
            collect_named_expr_target_names_in_blockpy_expr(value, &mut names);
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

pub(super) fn assigned_names_in_blockpy_fragment<E>(
    fragment: &BlockPyCfgFragment<StructuredInstr<E>, BlockPyTerm<E>>,
) -> HashSet<String>
where
    E: BlockPySemanticExprNode + Instr,
{
    let mut out = assigned_names_in_blockpy_stmts(&fragment.body);
    if let Some(term) = &fragment.term {
        out.extend(assigned_names_in_blockpy_term(term));
    }
    out
}

fn collect_named_expr_target_names_in_blockpy_expr<E>(expr: &E, names: &mut HashSet<String>)
where
    E: BlockPySemanticExprNode,
{
    expr.walk_root_defined_names(&mut |name| {
        names.insert(name.to_string());
    });
    expr.walk_child_exprs(&mut |child| {
        collect_named_expr_target_names_in_blockpy_expr(child, names);
    });
}

#[cfg(test)]
mod test;

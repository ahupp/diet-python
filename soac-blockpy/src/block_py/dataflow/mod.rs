use super::{
    BlockPyAssign, BlockPyBranchTable, BlockPyCfgFragment, BlockPyIf, BlockPyIfTerm,
    BlockPyNameLike, BlockPyRaise, BlockPySemanticExprNode, BlockPyTerm, StructuredBlockPyStmt,
};
use std::collections::HashSet;

pub(super) fn assigned_names_in_blockpy_stmt<E, N>(
    stmt: &StructuredBlockPyStmt<E, N>,
) -> HashSet<String>
where
    E: BlockPySemanticExprNode,
    N: BlockPyNameLike,
{
    match stmt {
        StructuredBlockPyStmt::Assign(BlockPyAssign { target, value }) => {
            let mut names = HashSet::from([target.id_str().to_string()]);
            collect_named_expr_target_names_in_blockpy_expr(value, &mut names);
            names
        }
        StructuredBlockPyStmt::Expr(expr) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(expr, &mut names);
            names
        }
        StructuredBlockPyStmt::Delete(_) => HashSet::new(),
        StructuredBlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(test, &mut names);
            names.extend(assigned_names_in_blockpy_fragment(body));
            names.extend(assigned_names_in_blockpy_fragment(orelse));
            names
        }
    }
}

pub(super) fn assigned_names_in_blockpy_stmts<E, N>(
    stmts: &[StructuredBlockPyStmt<E, N>],
) -> HashSet<String>
where
    E: BlockPySemanticExprNode,
    N: BlockPyNameLike,
{
    let mut out = HashSet::new();
    for stmt in stmts {
        out.extend(assigned_names_in_blockpy_stmt(stmt));
    }
    out
}

pub(super) fn assigned_names_in_blockpy_term<E>(term: &BlockPyTerm<E>) -> HashSet<String>
where
    E: BlockPySemanticExprNode,
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

pub(super) fn assigned_names_in_blockpy_fragment<E, N>(
    fragment: &BlockPyCfgFragment<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>,
) -> HashSet<String>
where
    E: BlockPySemanticExprNode,
    N: BlockPyNameLike,
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

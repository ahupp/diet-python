#[cfg(test)]
use super::{
    BlockBuilder, BlockTerm, Instr, ScopeExprNode, StructuredIf, StructuredInstr, TermBranchTable,
    TermIf, TermRaise,
};
#[cfg(test)]
use std::collections::HashSet;

#[cfg(test)]
pub(super) fn assigned_names_in_blockpy_stmt<E>(stmt: &StructuredInstr<E>) -> HashSet<String>
where
    E: ScopeExprNode + Instr,
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

#[cfg(test)]
pub(super) fn assigned_names_in_blockpy_stmts<E>(stmts: &[StructuredInstr<E>]) -> HashSet<String>
where
    E: ScopeExprNode + Instr,
{
    let mut out = HashSet::new();
    for stmt in stmts {
        out.extend(assigned_names_in_blockpy_stmt(stmt));
    }
    out
}

#[cfg(test)]
pub(super) fn assigned_names_in_blockpy_term<E>(term: &BlockTerm<E>) -> HashSet<String>
where
    E: ScopeExprNode + Instr,
{
    match term {
        BlockTerm::Jump(_) => HashSet::new(),
        BlockTerm::IfTerm(TermIf { test, .. }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(test, &mut names);
            names
        }
        BlockTerm::BranchTable(TermBranchTable { index, .. }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(index, &mut names);
            names
        }
        BlockTerm::Return(value) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(value, &mut names);
            names
        }
        BlockTerm::Raise(TermRaise { exc }) => {
            let mut names = HashSet::new();
            if let Some(exc) = exc {
                collect_named_expr_target_names_in_blockpy_expr(exc, &mut names);
            }
            names
        }
    }
}

#[cfg(test)]
pub(super) fn assigned_names_in_blockpy_fragment<E>(
    fragment: &BlockBuilder<StructuredInstr<E>, BlockTerm<E>>,
) -> HashSet<String>
where
    E: ScopeExprNode + Instr,
{
    let mut out = assigned_names_in_blockpy_stmts(&fragment.body);
    if let Some(term) = &fragment.term {
        out.extend(assigned_names_in_blockpy_term(term));
    }
    out
}

#[cfg(test)]
fn collect_named_expr_target_names_in_blockpy_expr<E>(expr: &E, names: &mut HashSet<String>)
where
    E: ScopeExprNode,
{
    expr.walk_root_defined_names(&mut |name| {
        names.insert(name.to_string());
    });
    expr.walk(&mut |child| {
        collect_named_expr_target_names_in_blockpy_expr(child, names);
    });
}

#[cfg(test)]
mod test;

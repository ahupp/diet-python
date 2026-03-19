use ruff_python_ast::Stmt;

pub(crate) fn flatten_stmt_boxes(stmts: &[Stmt]) -> Vec<Stmt> {
    let mut out = Vec::new();
    for stmt in stmts {
        flatten_stmt(stmt, &mut out);
    }
    out
}

pub(crate) fn strip_nonlocal_directives(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts
        .into_iter()
        .filter(|stmt| !matches!(stmt, Stmt::Global(_) | Stmt::Nonlocal(_)))
        .collect()
}

pub(crate) fn should_strip_nonlocal_for_bb(fn_name: &str) -> bool {
    // Generated helper functions (comprehensions/lambdas/etc.) are prefixed
    // `_dp_fn__dp_...` and currently rely on their existing non-BB lowering
    // behavior for closure propagation. Keep nonlocal directives there.
    !fn_name.starts_with("_dp_fn__dp_")
}

pub(crate) fn flatten_stmt(stmt: &Stmt, out: &mut Vec<Stmt>) {
    out.push(stmt.clone());
}

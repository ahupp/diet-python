use ruff_python_ast::Stmt;

pub(crate) fn flatten_stmt_boxes(stmts: &[Box<Stmt>]) -> Vec<Box<Stmt>> {
    let mut out = Vec::new();
    for stmt in stmts {
        flatten_stmt(stmt.as_ref(), &mut out);
    }
    out
}

pub(crate) fn strip_nonlocal_directives(stmts: Vec<Box<Stmt>>) -> Vec<Box<Stmt>> {
    stmts
        .into_iter()
        .filter(|stmt| !matches!(stmt.as_ref(), Stmt::Global(_) | Stmt::Nonlocal(_)))
        .collect()
}

pub(crate) fn should_strip_nonlocal_for_bb(fn_name: &str) -> bool {
    // Generated helper functions (comprehensions/lambdas/etc.) are prefixed
    // `_dp_fn__dp_...` and currently rely on their existing non-BB lowering
    // behavior for closure propagation. Keep nonlocal directives there.
    !fn_name.starts_with("_dp_fn__dp_")
}

pub(crate) fn flatten_stmt(stmt: &Stmt, out: &mut Vec<Box<Stmt>>) {
    if let Stmt::BodyStmt(body) = stmt {
        for child in &body.body {
            flatten_stmt(child.as_ref(), out);
        }
        return;
    }
    out.push(Box::new(stmt.clone()));
}

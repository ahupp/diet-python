use ruff_python_ast::{self as ast, Stmt};

pub type Suite = ast::Suite;
pub type Body = Suite;

pub fn suite_ref(body: &Body) -> &Suite {
    body
}

pub fn suite_mut(body: &mut Body) -> &mut Suite {
    body
}

pub fn take_suite(body: &mut Body) -> Suite {
    std::mem::take(body)
}

pub fn body_from_suite(body: Suite) -> Body {
    body
}

pub fn empty_body() -> Body {
    body_from_suite(empty_suite())
}

pub fn empty_suite() -> Suite {
    vec![]
}

pub fn cloned_suite(body: &Body) -> Suite {
    body.clone()
}

pub fn stmt_ref(body: &Body, index: usize) -> &Stmt {
    &body[index]
}

pub fn split_docstring(body: &Suite) -> (Option<String>, Vec<Stmt>) {
    let mut rest = body.clone();
    let Some(docstring) = rest.first().and_then(|first| match first {
        Stmt::Expr(ast::StmtExpr { value, .. }) => match value.as_ref() {
            ast::Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
                Some(value.to_string())
            }
            _ => None,
        },
        _ => None,
    }) else {
        return (None, rest);
    };
    rest.remove(0);
    (Some(docstring), rest)
}

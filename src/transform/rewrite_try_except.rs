use std::cell::Cell;

use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::py_stmt;

pub fn rewrite(
    ast::StmtTry {
        body,
        handlers,
        orelse,
        finalbody,
        is_star,
        ..
    }: ast::StmtTry,
    count: &Cell<usize>,
) -> Stmt {
    let id = count.get() + 1;
    count.set(id);
    let exc_name = format!("_dp_exc_{}", id);

    let exc_assign = py_stmt!(
        "{exc:id} = __dp__.current_exception()",
        exc = exc_name.as_str(),
    );

    let mut processed: Vec<(Option<Expr>, Vec<Stmt>)> = Vec::new();
    for handler in handlers {
        let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
            type_,
            name,
            body,
            ..
        }) = handler;
        let mut body_stmts = body;
        if let Some(name) = name {
            let assign = py_stmt!(
                "{name:id} = {exc:id}",
                name = name.id.as_str(),
                exc = exc_name.as_str(),
            );
            body_stmts.insert(0, assign);
        }
        processed.push((type_.map(|e| *e), body_stmts));
    }

    let mut new_body = vec![exc_assign];
    let mut chain: Vec<Stmt> = vec![py_stmt!("raise")];
    for (type_, body) in processed.into_iter().rev() {
        chain = if let Some(t) = type_ {
            vec![py_stmt!(
                r#"
if __dp__.isinstance({exc:id}, {typ:expr}):
  {body:stmt}
else:
  {next:stmt}
"#,
                exc = exc_name.as_str(),
                typ = t,
                body = body,
                next = chain,
            )]
        } else {
            body
        };
    }
    new_body.extend(chain);

    py_stmt!(
        r#"
try:
    {body:stmt}
except:
    {handler:stmt}
else:
    {orelse:stmt}
finally:
    {finally:stmt}
    "#,
        body = body,
        handler = new_body,
        orelse = orelse,
        finally = finalbody,
    )
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_typed_except() {
        let input = r#"
try:
    f()
except E as e:
    g(e)
"#;
        let expected = r#"
try:
    f()
except:
    _dp_exc_1 = __dp__.current_exception()
    if __dp__.isinstance(_dp_exc_1, E):
        e = _dp_exc_1
        g(e)
    else:
        raise
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_with_bare_except() {
        let input = r#"
try:
    f()
except E:
    h()
except:
    g()
"#;
        let expected = r#"
try:
    f()
except:
    _dp_exc_1 = __dp__.current_exception()
    if __dp__.isinstance(_dp_exc_1, E):
        h()
    else:
        g()
"#;
        assert_transform_eq(input, expected);
    }
}

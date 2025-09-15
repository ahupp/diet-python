use std::cell::Cell;

use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::py_stmt;

pub fn rewrite(stmt: ast::StmtTry, count: &Cell<usize>) -> Stmt {
    if !has_non_default_handler(&stmt) {
        return Stmt::Try(stmt);
    }

    let ast::StmtTry {
        body,
        handlers,
        orelse,
        finalbody,
        is_star: _,
        ..
    } = stmt;
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

fn has_non_default_handler(stmt: &ast::StmtTry) -> bool {
    stmt.handlers.iter().any(|handler| {
        matches!(
            handler,
            ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                type_: Some(_),
                ..
            })
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::transform::Options;
    use crate::transform_str_to_ruff_with_options;
    use ruff_python_ast::{self as ast, Stmt};

    fn has_non_default_handler(try_stmt: &ast::StmtTry) -> bool {
        try_stmt.handlers.iter().any(|handler| {
            matches!(
                handler,
                ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                    type_: Some(_),
                    ..
                })
            )
        })
    }

    fn first_try_with_options(source: &str, options: Options) -> ast::StmtTry {
        let module = transform_str_to_ruff_with_options(source, options).unwrap();
        match module.body.first() {
            Some(Stmt::Try(try_stmt)) => try_stmt.clone(),
            _ => panic!("expected try statement"),
        }
    }

    fn first_try(source: &str) -> ast::StmtTry {
        first_try_with_options(source, Options::for_test())
    }

    #[test]
    fn rewrites_typed_except() {
        let try_stmt = first_try(
            r#"
try:
    f()
except E as e:
    g(e)
"#,
        );
        assert!(!has_non_default_handler(&try_stmt));
    }

    #[test]
    fn rewrites_with_bare_except() {
        let try_stmt = first_try(
            r#"
try:
    f()
except E:
    h()
except:
    g()
"#,
        );
        assert!(!has_non_default_handler(&try_stmt));
    }

    #[test]
    fn skips_already_rewritten_try() {
        let try_stmt = first_try_with_options(
            r#"
try:
    f()
except:
    _dp_exc_1 = getattr(__dp__, "current_exception")()
    if getattr(__dp__, "isinstance")(_dp_exc_1, E):
        g()
    else:
        raise
"#,
            Options {
                inject_import: false,
                ..Options::default()
            },
        );
        assert!(!has_non_default_handler(&try_stmt));
        let assign_count = match try_stmt.handlers.first() {
            Some(ast::ExceptHandler::ExceptHandler(handler)) => handler
                .body
                .iter()
                .filter(|stmt| matches!(stmt, Stmt::Assign(_)))
                .count(),
            _ => 0,
        };
        assert_eq!(assign_count, 1);
    }
}

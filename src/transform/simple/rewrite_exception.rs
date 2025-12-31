use crate::transform::{context::Context, driver::Rewrite};
use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt};

pub fn rewrite_try(stmt: ast::StmtTry, _ctx: &Context) -> Rewrite {
    if stmt.is_star {
        return Rewrite::Walk(vec![Stmt::Try(stmt)]);
    }

    if !has_non_default_handler(&stmt) {
        return Rewrite::Walk(vec![Stmt::Try(stmt)]);
    }

    let base = if has_default_handler(&stmt) {
        py_stmt!("pass")
    } else {
        py_stmt!("raise")
    };

    let ast::StmtTry {
        body,
        handlers,
        orelse,
        finalbody,
        is_star: _,
        ..
    } = stmt;

    let handler_chain = handlers.into_iter().rev().fold(base, |acc, handler| {
        let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
            type_,
            name,
            body,
            ..
        }) = handler;

        if type_.is_none() {
            debug_assert!(name.is_none());
            return py_stmt!(
                r#"
{body:stmt}
{next:stmt}
"#,
                body = body,
                next = acc,
            );
        }

        let condition = py_expr!(
            "__dp__.exception_matches(__dp__.current_exception(), {typ:expr})",
            typ = type_.unwrap()
        );

        let (exc_target, body) = if let Some(ast::Identifier { id, .. }) = &name {
            let target = id.as_str();
            let exc_target = py_stmt!(
                "{target:id} = __dp__.current_exception()",
                target = target,
            );
            let body = py_stmt!(
                r#"
try:
    {body:stmt}
finally:
    try:
        del {target:id}
    except NameError:
        pass
"#,
                body = body,
                target = target,
            );
            (exc_target, body)
        } else {
            (py_stmt!("pass"), body)
        };

        py_stmt!(
            r#"
if {condition:expr}:
    {exc_target:stmt}
    {body:stmt}
else:
    {next:stmt}
"#,
            condition = condition,
            exc_target = exc_target,
            body = body,
            next = acc,
        )
    });

    Rewrite::Visit(py_stmt!(
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
        handler = handler_chain,
        orelse = orelse,
        finally = finalbody,
    ))
}

pub fn rewrite_raise(mut raise: ast::StmtRaise) -> Rewrite {
    match (raise.exc.take(), raise.cause.take()) {
        (Some(exc), Some(cause)) => Rewrite::Visit(py_stmt!(
            "raise __dp__.raise_from({exc:expr}, {cause:expr})",
            exc = exc,
            cause = cause,
        )),
        (exc, None) => {
            raise.exc = exc;
            Rewrite::Walk(vec![Stmt::Raise(raise)])
        }
        (None, Some(_)) => {
            panic!("raise with a cause but without an exception should be impossible")
        }
    }
}

pub(crate) fn has_non_default_handler(stmt: &ast::StmtTry) -> bool {
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

fn has_default_handler(stmt: &ast::StmtTry) -> bool {
    stmt.handlers.iter().any(|handler| {
        matches!(
            handler,
            ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler { type_: None, .. })
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
        module
            .body
            .into_iter()
            .find_map(|stmt| match stmt {
                Stmt::Try(try_stmt) => Some(try_stmt),
                _ => None,
            })
            .expect("expected try statement")
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

    crate::transform_fixture_test!("tests_rewrite_exception.txt");

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

use super::context::Context;
use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt};

pub fn rewrite(stmt: ast::StmtTry, _ctx: &Context) -> Stmt {
    if !has_non_default_handler(&stmt) {
        return Stmt::Try(stmt);
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

        let target = if let Some(ast::Identifier { id, .. }) = &name {
            id.as_str()
        } else {
            "_"
        };

        let condition = if let Some(typ) = type_ {
            py_expr!(
                "__dp__.isinstance(__dp__.current_exception(), {typ:expr})",
                typ = typ
            )
        } else {
            py_expr!("True")
        };

        py_stmt!(
            r#"
if {condition:expr}:
    {target:id} = __dp__.current_exception()
    {body:stmt}
else:
    {next:stmt}
"#,
            target = target,
            condition = condition,
            body = body,
            next = acc,
        )
    });

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
        handler = handler_chain,
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

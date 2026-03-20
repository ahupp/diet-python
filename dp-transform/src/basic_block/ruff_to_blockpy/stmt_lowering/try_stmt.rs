use super::*;
use crate::basic_block::ast_to_ast::ast_rewrite::Rewrite;
use crate::basic_block::ast_to_ast::body::{suite_ref, take_suite, Suite};
use crate::{py_expr, py_stmt};

fn body_to_vec(body: Suite) -> Vec<Stmt> {
    body
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

pub(crate) fn rewrite_try_stmt(stmt: ast::StmtTry) -> Rewrite {
    if stmt.is_star {
        let ast::StmtTry {
            mut body,
            handlers,
            mut orelse,
            mut finalbody,
            is_star: _,
            ..
        } = stmt;
        let body = body_to_vec(take_suite(&mut body));
        let orelse = body_to_vec(take_suite(&mut orelse));
        let finalbody = body_to_vec(take_suite(&mut finalbody));

        let mut handler_body: Vec<Stmt> = Vec::new();
        handler_body.push(py_stmt!("_dp_exc = __dp_current_exception()"));
        handler_body.push(py_stmt!("_dp_rest = _dp_exc"));

        for handler in handlers {
            let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                type_,
                name,
                body: mut h_body,
                ..
            }) = handler;

            let typ = match type_ {
                Some(expr) => expr,
                None => Box::new(py_expr!("BaseException")),
            };

            let (exc_target, body) = if let Some(ast::Identifier { id, .. }) = &name {
                let target = id.as_str();
                let exc_target = py_stmt!("{target:id} = _dp_match", target = target);
                // TODO: Re-evaluate whether exception-name binding cleanup should live here
                // or stay in the earlier explicit-binding rewrite. For now, rely on the
                // upstream rewrite_names pass to own any required cleanup scaffolding.
                (exc_target, body_to_vec(take_suite(&mut h_body)))
            } else {
                (py_stmt!("pass"), body_to_vec(take_suite(&mut h_body)))
            };

            handler_body.push(py_stmt!(
                "_dp_match, _dp_rest = __dp_exceptiongroup_split(_dp_rest, {typ:expr})",
                typ = typ,
            ));
            handler_body.push(py_stmt!(
                r#"
if _dp_match is not None:
    {exc_target:stmt}
    {body:stmt}
"#,
                exc_target = exc_target,
                body = body,
            ));
        }

        handler_body.push(py_stmt!(
            r#"
if _dp_rest is not None:
    raise _dp_rest
"#
        ));

        return Rewrite::Walk(vec![py_stmt!(
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
            handler = handler_body,
            orelse = orelse,
            finally = finalbody,
        )]);
    }
    if !has_non_default_handler(&stmt) {
        return Rewrite::Unmodified(stmt.into());
    }

    let base = if has_default_handler(&stmt) {
        py_stmt!("pass")
    } else {
        py_stmt!("raise")
    };

    let ast::StmtTry {
        mut body,
        handlers,
        mut orelse,
        mut finalbody,
        is_star: _,
        ..
    } = stmt;
    let body = body_to_vec(take_suite(&mut body));
    let orelse = body_to_vec(take_suite(&mut orelse));
    let finalbody = body_to_vec(take_suite(&mut finalbody));

    let handler_chain = handlers.into_iter().rev().fold(base, |acc, handler| {
        let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
            type_,
            name,
            mut body,
            ..
        }) = handler;

        if type_.is_none() {
            assert!(name.is_none());
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
            "__dp_exception_matches(__dp_current_exception(), {typ:expr})",
            typ = type_.unwrap()
        );

        let (exc_target, body) = if let Some(ast::Identifier { id, .. }) = &name {
            let target = id.as_str();
            let exc_target = py_stmt!("{target:id} = __dp_current_exception()", target = target,);
            // TODO: Re-evaluate whether exception-name binding cleanup should live here
            // or stay in the earlier explicit-binding rewrite. For now, rely on the
            // upstream rewrite_names pass to own any required cleanup scaffolding.
            (exc_target, body_to_vec(take_suite(&mut body)))
        } else {
            (py_stmt!("pass"), body_to_vec(take_suite(&mut body)))
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

    Rewrite::Walk(vec![py_stmt!(
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
    )])
}

impl StmtLowerer for ast::StmtTry {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(rewrite_try_stmt(self))
    }
}

pub(crate) fn lower_star_try_stmt_sequence<F>(
    try_stmt: ast::StmtTry,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    jump_label: Option<String>,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, &mut Vec<BlockPyBlock>) -> String,
{
    let rewritten_try = match rewrite_try_stmt(try_stmt) {
        Rewrite::Unmodified(stmt) => stmt_to_stmts(stmt),
        Rewrite::Walk(stmts) => stmts,
    };
    lower_expanded_stmt_sequence(
        rewritten_try,
        remaining_stmts,
        cont_label,
        linear,
        blocks,
        jump_label,
        lower_sequence,
    )
}

pub(crate) fn lower_try_stmt_sequence<F>(
    try_stmt: ast::StmtTry,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    try_plan: TryPlan,
    lower_sequence: &mut F,
) -> (String, TryRegionPlan)
where
    F: FnMut(&[Stmt], String, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_sequence(remaining_stmts, cont_label.clone(), blocks);

    let else_body = flatten_stmt_boxes(suite_ref(&try_stmt.orelse));
    let try_body = flatten_stmt_boxes(suite_ref(&try_stmt.body));
    let except_body =
        (!try_stmt.handlers.is_empty()).then(|| prepare_except_body(&try_stmt.handlers));
    let finally_body = if !suite_ref(&try_stmt.finalbody).is_empty() {
        Some(prepare_finally_body(
            suite_ref(&try_stmt.finalbody),
            try_plan.finally_exc_name.as_deref(),
        ))
    } else {
        None
    };

    let lowered_try = lower_try_regions(
        blocks,
        &try_plan,
        rest_entry.as_str(),
        finally_body,
        else_body,
        try_body,
        except_body,
        lower_sequence,
    );

    finalize_try_regions(
        blocks,
        label,
        linear,
        lowered_try.body_label,
        lowered_try.except_label,
        try_plan,
        lowered_try.body_region_range,
        lowered_try.else_region_range,
        lowered_try.except_region_range,
        lowered_try.finally_region_range,
        lowered_try.finally_label,
        lowered_try.finally_normal_entry,
    )
}

#[cfg(test)]
mod tests {
    use super::super::simplify_stmt_ast_once_for_blockpy;
    use super::*;
    use crate::basic_block::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_try_simplify_ast_rewrites_typed_except_before_blockpy_lowering() {
        let stmt = py_stmt!(
            r#"
try:
    work()
except ValueError as exc:
    handle(exc)
"#
        );
        let Stmt::Try(try_stmt) = stmt else {
            panic!("expected try stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Try(try_stmt));
        let rendered = crate::ruff_ast_to_string(simplified.as_slice());

        assert!(rendered.contains("__dp_exception_matches"), "{rendered}");
        assert!(rendered.contains("__dp_current_exception()"), "{rendered}");
    }

    #[test]
    fn stmt_try_simplify_ast_rewrites_except_star_before_blockpy_lowering() {
        let stmt = py_stmt!(
            r#"
try:
    work()
except* ValueError as exc:
    handle(exc)
"#
        );
        let Stmt::Try(try_stmt) = stmt else {
            panic!("expected try stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Try(try_stmt));
        let rendered = crate::ruff_ast_to_string(simplified.as_slice());

        assert!(rendered.contains("__dp_exceptiongroup_split"), "{rendered}");
    }
}

use crate::py_stmt;
use crate::template::empty_body;
use crate::transform::ast_rewrite::Rewrite;
use crate::transform::context::Context;
use ruff_python_ast::{self as ast, Stmt};

pub fn rewrite(context: &Context, with_stmt: ast::StmtWith) -> Rewrite {
    if with_stmt.items.is_empty() {
        return Rewrite::Unmodified(with_stmt.into());
    }

    let ast::StmtWith {
        items,
        body,
        is_async,
        ..
    } = with_stmt;

    let mut body: Stmt = body.into();

    for ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } in items.into_iter().rev()
    {
        let target = optional_vars.map(|var| *var);

        let exit_name = context.fresh("with_exit");
        let ok_name = context.fresh("with_ok");
        let suppress_name = context.fresh("with_suppress");

        let ctx_placeholder = context.maybe_placeholder_lowered(context_expr);

        // TODO: more formal handling of placeholder reuse
        let ctx_cleanup = if ctx_placeholder.modified {
            py_stmt!("{ctx:expr} = None", ctx = ctx_placeholder.expr.clone(),)
        } else {
            empty_body().into()
        };

        body = if is_async {
            let enter_stmt = if let Some(target) = target.clone() {
                py_stmt!(
                    "{target:expr} = await __dp__.asynccontextmanager_aenter({ctx:expr})",
                    target = target,
                    ctx = ctx_placeholder.expr.clone(),
                )
            } else {
                py_stmt!(
                    "await __dp__.asynccontextmanager_aenter({ctx:expr})",
                    ctx = ctx_placeholder.expr.clone(),
                )
            };
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp__.asynccontextmanager_get_aexit({ctx_placeholder_expr_1:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except:
    {ok_name:id} = False
    {suppress_name:id} = await __dp__.asynccontextmanager_aexit({exit_name:id}, __dp__.exc_info())
    if not {suppress_name:id}:
        raise
finally:
    if {ok_name:id}:
        await __dp__.asynccontextmanager_aexit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_expr_1 = ctx_placeholder.expr.clone(),
                ctx_placeholder_stmt = ctx_placeholder.stmt,
                enter_stmt = enter_stmt,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                suppress_name = suppress_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        } else {
            let enter_stmt = if let Some(target) = target.clone() {
                py_stmt!(
                    "{target:expr} = __dp__.contextmanager_enter({ctx:expr})",
                    target = target,
                    ctx = ctx_placeholder.expr.clone(),
                )
            } else {
                py_stmt!(
                    "__dp__.contextmanager_enter({ctx:expr})",
                    ctx = ctx_placeholder.expr.clone(),
                )
            };
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp__.contextmanager_get_exit({ctx_placeholder_expr_1:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except:
    {ok_name:id} = False
    __dp__.contextmanager_exit({exit_name:id}, __dp__.exc_info())
finally:
    if {ok_name:id}:
        __dp__.contextmanager_exit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_expr_1 = ctx_placeholder.expr.clone(),
                ctx_placeholder_stmt = ctx_placeholder.stmt,
                enter_stmt = enter_stmt,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        };
    }

    Rewrite::Walk(body)
}

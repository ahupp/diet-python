use crate::transform::ast_rewrite::Rewrite;
use crate::transform::context::Context;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Stmt};


pub fn rewrite(
    context: &Context,
    with_stmt: ast::StmtWith,
) -> Rewrite {
    if with_stmt.items.is_empty() {
        return Rewrite::Unmodified(with_stmt.into());
    }

    let ast::StmtWith { items, body, is_async, .. } = with_stmt;

    let mut body: Stmt = body.into();
    
    for ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } in items.into_iter().rev()
    {
        let target = if let Some(var) = optional_vars {
            *var
        } else {
            py_expr!("{tmp:id}", tmp = context.fresh("tmp"))
        };

        let exit_name = context.fresh("with_exit");
        let ok_name = context.fresh("with_ok");


        let ctx_placeholder = context.maybe_placeholder_lowered(context_expr);

        // TODO: more formal handling of placeholder reuse
        body = if is_async {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp__.asynccontextmanager_get_aexit({ctx_placeholder_expr_1:expr})
{target:expr} = await __dp__.asynccontextmanager_aenter({ctx_placeholder_expr_2:expr})
{ok_name:id} = True
try:
    {body:stmt}
except:
    {ok_name:id} = False
    await __dp__.asynccontextmanager_aexit({exit_name:id}, __dp__.exc_info())
finally:
    if {ok_name:id}:
        await __dp__.asynccontextmanager_aexit({exit_name:id}, None)
    {exit_name:id} = None
"#,
                ctx_placeholder_expr_1 = ctx_placeholder.expr.clone(),
                ctx_placeholder_expr_2 = ctx_placeholder.expr.clone(),
                ctx_placeholder_stmt = ctx_placeholder.stmt,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
            )
        } else {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp__.contextmanager_get_exit({ctx_placeholder_expr_1:expr})
{target:expr} = __dp__.contextmanager_enter({ctx_placeholder_expr_2:expr})
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
"#,
                ctx_placeholder_expr_1 = ctx_placeholder.expr.clone(),
                ctx_placeholder_expr_2 = ctx_placeholder.expr.clone(),
                ctx_placeholder_stmt = ctx_placeholder.stmt,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
            )
        };
    }

    Rewrite::Walk(body)
}

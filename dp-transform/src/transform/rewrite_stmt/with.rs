use crate::transform::ast_rewrite::Rewrite;
use crate::transform::context::Context;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast};

pub fn rewrite(
    context: &Context,
    with_stmt: ast::StmtWith,
) -> Rewrite {
    if with_stmt.items.is_empty() {
        return Rewrite::Unmodified(with_stmt.into());
    }

    let ast::StmtWith {
        items,
        mut body,
        is_async,
        ..
    } = with_stmt;

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


        let (ctx_name, ctx_assign) = context.named_placeholder(context_expr);

        body = if is_async {
            py_stmt!(
                r#"
{ctx_assign:stmt}
{exit_name:id} = __dp__.asynccontextmanager_get_aexit({ctx_name:id})
{target:expr} = await __dp__.asynccontextmanager_aenter({ctx_name:id})
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
                ctx_name = ctx_name.as_str(),
                ctx_assign = ctx_assign,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
            )
        } else {
            py_stmt!(
                r#"
{ctx_assign:stmt}
{exit_name:id} = __dp__.contextmanager_get_exit({ctx_name:id})
{target:expr} = __dp__.contextmanager_enter({ctx_name:id})
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
                ctx_name = ctx_name.as_str(),
                ctx_assign = ctx_assign,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
            )
        };
    }

    Rewrite::Walk(body)
}

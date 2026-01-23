use crate::transform::driver::ExprRewriter;
use crate::transform::driver::Rewrite;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast};

pub fn rewrite(
    ast::StmtWith {
        items,
        mut body,
        is_async,
        ..
    }: ast::StmtWith,
    transformer: &mut ExprRewriter,
) -> Rewrite {
    if items.is_empty() {
        return Rewrite::Walk(py_stmt!("pass"));
    }

    for ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } in items.into_iter().rev()
    {
        let target = if let Some(var) = optional_vars {
            *var
        } else {
            py_expr!("_")
        };

        let exit_name = transformer.context().fresh("with_exit");
        let ok_name = transformer.context().fresh("with_ok");

        body = if is_async {
            py_stmt!(
                r#"
({target:expr}, {exit_name:id}) = await __dp__.with_aenter({ctx:expr})
{ok_name:id} = True
try:
    {body:stmt}
except:
    {ok_name:id} = False
    await __dp__.with_aexit({exit_name:id}, __dp__.exc_info())
finally:
    if {ok_name:id}:
        await __dp__.with_aexit({exit_name:id}, None)
    {exit_name:id} = None
"#,
                ctx = context_expr,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
            )
        } else {
            py_stmt!(
                r#"
({target:expr}, {exit_name:id}) = __dp__.with_enter({ctx:expr})
{ok_name:id} = True
try:
    {body:stmt}
except:
    {ok_name:id} = False
    __dp__.with_exit({exit_name:id}, __dp__.exc_info())
finally:
    if {ok_name:id}:
        __dp__.with_exit({exit_name:id}, None)
    {exit_name:id} = None
"#,
                ctx = context_expr,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
            )
        };
    }

    Rewrite::Visit(body)
}

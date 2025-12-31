use crate::body_transform::Transformer;
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
        return Rewrite::Visit(py_stmt!("pass"));
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

        let enter_name = transformer.context().fresh("with_enter");
        let exit_name = transformer.context().fresh("with_exit");
        let active_name = transformer.context().fresh("with_active");

        body = if is_async {
            py_stmt!(
                r#"
{enter_name:id} = await __dp__.with_aenter({ctx:expr})
{target:expr} = __dp__.getitem({enter_name:id}, 0)
{exit_name:id} = __dp__.getitem({enter_name:id}, 1)
{active_name:id} = True
try:
    {body:stmt}
except:
    {active_name:id} = False
    await __dp__.with_aexit({exit_name:id}, __dp__.exc_info())
finally:
    if {active_name:id}:
        await __dp__.with_aexit({exit_name:id}, None)
    {exit_name:id} = None
    {enter_name:id} = None
"#,
                ctx = context_expr,
                target = target,
                body = body,
                enter_name = enter_name.as_str(),
                exit_name = exit_name.as_str(),
                active_name = active_name.as_str(),
            )
        } else {
            py_stmt!(
                r#"
{enter_name:id} = __dp__.with_enter({ctx:expr})
{target:expr} = __dp__.getitem({enter_name:id}, 0)
{exit_name:id} = __dp__.getitem({enter_name:id}, 1)
{active_name:id} = True
try:
    {body:stmt}
except:
    {active_name:id} = False
    __dp__.with_exit({exit_name:id}, __dp__.exc_info())
finally:
    if {active_name:id}:
        __dp__.with_exit({exit_name:id}, None)
    {exit_name:id} = None
    {enter_name:id} = None
"#,
                ctx = context_expr,
                target = target,
                body = body,
                enter_name = enter_name.as_str(),
                exit_name = exit_name.as_str(),
                active_name = active_name.as_str(),
            )
        };
    }

    transformer.visit_body(&mut body);
    Rewrite::Visit(body)
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_with.txt");
}

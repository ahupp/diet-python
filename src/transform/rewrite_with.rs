use super::{context::Context, driver::Rewrite};
use crate::body_transform::Transformer;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast};

pub fn rewrite(
    ast::StmtWith {
        items,
        mut body,
        is_async,
        ..
    }: ast::StmtWith,
    ctx: &Context,
    transformer: &mut impl Transformer,
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

        body = if is_async {
            let exit_name = ctx.fresh("awith_exit");
            let active_name = ctx.fresh("awith_active");
            py_stmt!(
                r#"
({target:expr}, {exit_name:id}) = await __dp__.with_aenter({ctx:expr})
{active_name:id} = True
try:
    try:
        {body:stmt}
    except:
        {active_name:id} = False
        await __dp__.with_aexit({exit_name:id}, __dp__.exc_info())
    else:
        {active_name:id} = False
        await __dp__.with_aexit({exit_name:id}, None)
finally:
    if {active_name:id}:
        await __dp__.with_aexit({exit_name:id}, None)
"#,
                ctx = context_expr,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
                active_name = active_name.as_str(),
            )
        } else {
            let exit_name = ctx.fresh("with_exit");
            let active_name = ctx.fresh("with_active");
            py_stmt!(
                r#"
({target:expr}, {exit_name:id}) = __dp__.with_enter({ctx:expr})
{active_name:id} = True
try:
    try:
        {body:stmt}
    except:
        {active_name:id} = False
        __dp__.with_exit({exit_name:id}, __dp__.exc_info())
    else:
        {active_name:id} = False
        __dp__.with_exit({exit_name:id}, None)
finally:
    if {active_name:id}:
        __dp__.with_exit({exit_name:id}, None)
"#,
                ctx = context_expr,
                target = target,
                body = body,
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

use super::context::Context;
use crate::body_transform::Transformer;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Stmt};

pub fn rewrite(
    ast::StmtWith {
        items,
        mut body,
        is_async,
        ..
    }: ast::StmtWith,
    ctx: &Context,
    transformer: &impl Transformer,
) -> Stmt {
    if items.is_empty() {
        let mut stmt = py_stmt!("pass");
        transformer.visit_stmt(&mut stmt);
        return stmt;
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

        let wrapper = if is_async {
            let exit_name = ctx.fresh("awith_exit");
            py_stmt!(
                r#"
({target:expr}, {exit_name:id}) = await __dp__.with_aenter({ctx:expr})
try:
    {body:stmt}
except:
    await __dp__.with_aexit({exit_name:id}, __dp__.exc_info())
else:
    await __dp__.with_aexit({exit_name:id}, None)
"#,
                ctx = context_expr,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
            )
        } else {
            let exit_name = ctx.fresh("with_exit");
            py_stmt!(
                r#"
({target:expr}, {exit_name:id}) = __dp__.with_enter({ctx:expr})
try:
    {body:stmt}
except:
    __dp__.with_exit({exit_name:id}, __dp__.exc_info())
else:
    __dp__.with_exit({exit_name:id}, None)
"#,
                ctx = context_expr,
                target = target,
                body = body,
                exit_name = exit_name.as_str(),
            )
        };
        body = vec![wrapper];
    }

    let mut stmt = body.into_iter().next().unwrap();
    transformer.visit_stmt(&mut stmt);
    stmt
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_with.txt");
}

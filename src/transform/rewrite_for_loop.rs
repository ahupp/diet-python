use super::context::Context;
use crate::body_transform::Transformer;
use crate::py_stmt;
use ruff_python_ast::{self as ast, Stmt};

pub fn rewrite(
    ast::StmtFor {
        target,
        iter,
        body,
        orelse,
        is_async,
        ..
    }: ast::StmtFor,
    ctx: &Context,
    transformer: &mut impl Transformer,
) -> Stmt {
    let iter_name = ctx.fresh("iter");

    let mut rewritten = if is_async {
        py_stmt!(
            r#"
{iter_name:id} = __dp__.aiter({iter:expr})
while True:
    try:
        {target:expr} = await __dp__.anext({iter_name:id})
    except:
        __dp__.acheck_stopiteration()
        {orelse:stmt}
        break
    else:
        {body:stmt}
    "#,
            iter_name = iter_name.as_str(),
            iter = iter,
            target = target,
            orelse = orelse,
            body = body,
        )
    } else {
        py_stmt!(
            r#"
{iter_name:id} = __dp__.iter({iter:expr})
while True:
    try:
        {target:expr} = __dp__.next({iter_name:id})
    except:
        __dp__.check_stopiteration()
        {orelse:stmt}
        break
    else:
        {body:stmt}
    "#,
            iter_name = iter_name.as_str(),
            iter = iter,
            target = target,
            orelse = orelse,
            body = body,
        )
    };

    transformer.visit_stmt(&mut rewritten);

    rewritten
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_for_loop.txt");
}

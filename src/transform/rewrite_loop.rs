use super::{
    context::Context,
    driver::{ExprRewriter, Rewrite},
};
use crate::body_transform::Transformer;
use crate::py_stmt;
use ruff_python_ast::{self as ast, Stmt};

pub fn rewrite_for(
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
) -> Rewrite {
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

    transformer.visit_body(&mut rewritten);

    Rewrite::Visit(rewritten)
}

pub fn rewrite_while(mut while_stmt: ast::StmtWhile, rewriter: &mut ExprRewriter) -> Rewrite {
    let guard = rewriter.expand_here(while_stmt.test.as_mut());

    if guard.is_empty() {
        // Unclear if / when this ever happens
        return Rewrite::Walk(vec![Stmt::While(while_stmt)]);
    }

    let ast::StmtWhile {
        test, body, orelse, ..
    } = while_stmt;

    Rewrite::Visit(py_stmt!(
        r#"
while True:
    {guard:stmt}
    if not {condition:expr}:
        {orelse:stmt}
        break
    {body:stmt}
"#,
        guard = guard,
        condition = *test,
        body = body,
        orelse = orelse,
    ))
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_loop.txt");
}

use super::context::Context;
use ruff_python_ast::{self as ast, Stmt};

use crate::py_stmt;

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
) -> Stmt {
    let iter_name = ctx.fresh("iter");

    if is_async {
        py_stmt!(
            r#"
{iter_name:id} = __dp__.aiter({iter:expr})
while True:
    try:
        {target:expr} = await __dp__.anext({iter_name:id})
    except StopAsyncIteration:
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
        {target:expr} = __dp__.anext({iter_name:id})
    except StopIteration:
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
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_for_loop_with_else() {
        let input = r#"
for a in b:
    if cond:
        break
else:
    c()
"#;
        let expected = r#"
_dp_iter_1 = __dp__.iter(b)
while True:
    try:
        a = __dp__.next(_dp_iter_1)
    except:
        _dp_exc_2 = __dp__.current_exception()
        if __dp__.isinstance(_dp_exc_2, StopIteration):
            c()
            break
        else:
            raise
    if cond:
        break
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_for_loop_without_else() {
        let input = r#"
for a in b:
    c(a)
"#;
        let expected = r#"
_dp_iter_1 = __dp__.iter(b)
while True:
    try:
        a = __dp__.next(_dp_iter_1)
    except:
        _dp_exc_2 = __dp__.current_exception()
        if __dp__.isinstance(_dp_exc_2, StopIteration):
            break
        else:
            raise
    c(a)
"#;

        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_async_for_loop_with_else() {
        let input = r#"
async def f():
    async for a in b:
        if cond:
            break
    else:
        c()
"#;
        let expected = r#"
async def f():
    _dp_iter_1 = __dp__.aiter(b)
    while True:
        try:
            a = await __dp__.anext(_dp_iter_1)
        except:
            _dp_exc_2 = __dp__.current_exception()
            if __dp__.isinstance(_dp_exc_2, StopAsyncIteration):
                c()
                break
            else:
                raise
        if cond:
            break
"#;
        assert_transform_eq(input, expected);
    }
}

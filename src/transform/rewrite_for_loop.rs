use std::cell::Cell;

use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt};

pub fn rewrite(
    ast::StmtFor {
        target,
        iter,
        body,
        mut orelse,
        is_async,
        ..
    }: ast::StmtFor,
    iter_count: &Cell<usize>,
) -> Stmt {
    let id = iter_count.get() + 1;
    iter_count.set(id);
    let iter_name = format!("_dp_iter_{}", id);

    let (iter_fn, next_fn, stop_exc, await_) = if is_async {
        (
            py_expr!("__dp__.aiter"),
            py_expr!("__dp__.anext"),
            "StopAsyncIteration",
            "await ",
        )
    } else {
        (
            py_expr!("__dp__.iter"),
            py_expr!("__dp__.next"),
            "StopIteration",
            "",
        )
    };

    orelse.push(py_stmt!("break"));

    py_stmt!(
        r#"
{iter_name:id} = {iter_fn:expr}({iter:expr})
while True:
    try:
        {target:expr} = {await_:id}{next_fn:expr}({iter_name:id})
    except {stop_exc:id}:
        {orelse:stmt}
    {body:stmt}
"#,
        iter_name = iter_name.as_str(),
        iter_fn = iter_fn,
        iter = iter,
        target = target,
        await_ = await_,
        next_fn = next_fn,
        stop_exc = stop_exc,
        orelse = orelse,
        body = body,
    )
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
        _dp_exc_1 = __dp__.current_exception()
        if __dp__.isinstance(_dp_exc_1, StopIteration):
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
        _dp_exc_1 = __dp__.current_exception()
        if __dp__.isinstance(_dp_exc_1, StopIteration):
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
            _dp_exc_1 = __dp__.current_exception()
            if __dp__.isinstance(_dp_exc_1, StopAsyncIteration):
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

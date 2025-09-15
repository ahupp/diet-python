use std::cell::Cell;

use ruff_python_ast::{self as ast, Stmt};

use crate::py_stmt;

pub fn rewrite(
    ast::StmtWith {
        items,
        body,
        is_async,
        ..
    }: ast::StmtWith,
    count: &Cell<usize>,
) -> Stmt {
    if items.is_empty() {
        return py_stmt!(
            "
pass
"
        );
    }

    let is_async_stmt = is_async;
    let mut body_stmts = body;

    let mut work = Vec::new();
    for item in items {
        let id = count.get() + 1;
        count.set(id);
        work.push((item, id));
    }

    for (
        ast::WithItem {
            context_expr,
            optional_vars,
            ..
        },
        id,
    ) in work.into_iter().rev()
    {
        let enter_name = format!("_dp_enter_{}", id);
        let exit_name = format!("_dp_exit_{}", id);
        let ctx_name = format!("_dp_ctx_{}", id);

        let ctx_assign = py_stmt!(
            "{ctx_var:id} = {ctx:expr}",
            ctx_var = ctx_name.as_str(),
            ctx = context_expr,
        );

        let (enter_method, exit_method, await_) = if is_async_stmt {
            ("__aenter__", "__aexit__", "await ")
        } else {
            ("__enter__", "__exit__", "")
        };

        let pre_stmt = if let Some(var) = optional_vars {
            py_stmt!(
                "{var:expr} = {await_:id}{enter:id}({ctx_var:id})",
                var = *var,
                await_ = await_,
                enter = enter_name.as_str(),
                ctx_var = ctx_name.as_str(),
            )
        } else {
            py_stmt!(
                "{await_:id}{enter:id}({ctx_var:id})",
                await_ = await_,
                enter = enter_name.as_str(),
                ctx_var = ctx_name.as_str(),
            )
        };

        let wrapper = py_stmt!(
            "
{ctx_assign:stmt}
{enter:id} = type({ctx_var:id}).{enter_method:id}
{exit:id} = type({ctx_var:id}).{exit_method:id}
{pre:stmt}
try:
    {body:stmt}
except:
    if not {await_:id}{exit:id}({ctx_var:id}, *__dp__.exc_info()):
        raise
else:
    {await_:id}{exit:id}({ctx_var:id}, None, None, None)
",
            ctx_assign = ctx_assign,
            enter = enter_name.as_str(),
            exit = exit_name.as_str(),
            ctx_var = ctx_name.as_str(),
            enter_method = enter_method,
            exit_method = exit_method,
            await_ = await_,
            pre = pre_stmt,
            body = body_stmts,
        );

        body_stmts = vec![wrapper];
    }

    body_stmts.into_iter().next().unwrap()
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_with_statement() {
        let input = r#"
with a as b:
    c
"#;
        let expected = r#"
_dp_ctx_1 = a
_dp_enter_1 = type(_dp_ctx_1).__enter__
_dp_exit_1 = type(_dp_ctx_1).__exit__
b = _dp_enter_1(_dp_ctx_1)
try:
    c
except:
    _dp_exc_1 = __dp__.current_exception()
    if __dp__.not_(_dp_exit_1(_dp_ctx_1, *__dp__.exc_info())):
        raise
else:
    _dp_exit_1(_dp_ctx_1, None, None, None)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_multiple_with_statement() {
        let input = r#"
with a as b, c as d:
    e
"#;
        let expected = r#"
_dp_ctx_1 = a
_dp_enter_1 = type(_dp_ctx_1).__enter__
_dp_exit_1 = type(_dp_ctx_1).__exit__
b = _dp_enter_1(_dp_ctx_1)
try:
    _dp_ctx_2 = c
    _dp_enter_2 = type(_dp_ctx_2).__enter__
    _dp_exit_2 = type(_dp_ctx_2).__exit__
    d = _dp_enter_2(_dp_ctx_2)
    try:
        e
    except:
        _dp_exc_2 = __dp__.current_exception()
        if __dp__.not_(_dp_exit_2(_dp_ctx_2, *__dp__.exc_info())):
            raise
    else:
        _dp_exit_2(_dp_ctx_2, None, None, None)
except:
    _dp_exc_1 = __dp__.current_exception()
    if __dp__.not_(_dp_exit_1(_dp_ctx_1, *__dp__.exc_info())):
        raise
else:
    _dp_exit_1(_dp_ctx_1, None, None, None)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_async_with_statement() {
        let input = r#"
async def f():
    async with a as b:
        c
"#;
        let expected = r#"
async def f():
    _dp_ctx_1 = a
    _dp_enter_1 = type(_dp_ctx_1).__aenter__
    _dp_exit_1 = type(_dp_ctx_1).__aexit__
    b = await _dp_enter_1(_dp_ctx_1)
    try:
        c
    except:
        _dp_exc_1 = __dp__.current_exception()
        if __dp__.not_(await _dp_exit_1(_dp_ctx_1, *__dp__.exc_info())):
            raise
    else:
        await _dp_exit_1(_dp_ctx_1, None, None, None)
"#;
        assert_transform_eq(input, expected);
    }
}

use super::context::Context;
use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt};

pub fn rewrite(
    ast::StmtWith {
        items,
        mut body,
        is_async,
        ..
    }: ast::StmtWith,
    ctx: &Context,
) -> Stmt {
    if items.is_empty() {
        return py_stmt!("pass");
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
            py_stmt!(
                r#"
{awith_state:id} = __dp__.with_aenter({ctx:expr})
({target:expr}, _) = awith_state
try:
    {body:stmt}
except:
    await __dp__.with_aexit(awith_state, __dp__.exc_info())
else:
    await __dp__.with_aexit(awith_state, None)
"#,
                awith_state = ctx.fresh("awith_state"),
                ctx = context_expr,
                target = target,
                body = body,
            )
        } else {
            py_stmt!(
                r#"
{with_state:id} = __dp__.with_enter({ctx:expr})
({target:expr}, _) = with_state
try:
    {body:stmt}
except:
    __dp__.with_exit(with_state, __dp__.exc_info())
else:
    __dp__.with_exit(with_state, None)
"#,
                with_state = ctx.fresh("with_state"),
                ctx = context_expr,
                target = target,
                body = body,
            )
        };
        body = vec![wrapper];
    }

    body.into_iter().next().unwrap()
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
_dp_enter_2 = type(_dp_ctx_1).__enter__
_dp_exit_3 = type(_dp_ctx_1).__exit__
b = _dp_enter_2(_dp_ctx_1)
try:
    c
except:
    if __dp__.not_(_dp_exit_3(_dp_ctx_1, *__dp__.exc_info())):
        raise
else:
    _dp_exit_3(_dp_ctx_1, None, None, None)
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
_dp_enter_2 = type(_dp_ctx_1).__enter__
_dp_exit_3 = type(_dp_ctx_1).__exit__
b = _dp_enter_2(_dp_ctx_1)
try:
    _dp_ctx_4 = c
    _dp_enter_5 = type(_dp_ctx_4).__enter__
    _dp_exit_6 = type(_dp_ctx_4).__exit__
    d = _dp_enter_5(_dp_ctx_4)
    try:
        e
    except:
        if __dp__.not_(_dp_exit_6(_dp_ctx_4, *__dp__.exc_info())):
            raise
    else:
        _dp_exit_6(_dp_ctx_4, None, None, None)
except:
    if __dp__.not_(_dp_exit_3(_dp_ctx_1, *__dp__.exc_info())):
        raise
else:
    _dp_exit_3(_dp_ctx_1, None, None, None)
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
    _dp_enter_2 = type(_dp_ctx_1).__aenter__
    _dp_exit_3 = type(_dp_ctx_1).__aexit__
    b = await _dp_enter_2(_dp_ctx_1)
    try:
        c
    except:
        if __dp__.not_(await _dp_exit_3(_dp_ctx_1, *__dp__.exc_info())):
            raise
    else:
        await _dp_exit_3(_dp_ctx_1, None, None, None)
"#;
        assert_transform_eq(input, expected);
    }
}

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
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_with_statement() {
        let input = r#"
with a as b:
    c
"#;
        let expected = r#"
_dp_tmp_2 = __dp__.with_enter(a)
b = __dp__.getitem(_dp_tmp_2, 0)
_dp_with_exit_1 = __dp__.getitem(_dp_tmp_2, 1)
try:
    c
except:
    __dp__.with_exit(_dp_with_exit_1, __dp__.exc_info())
else:
    __dp__.with_exit(_dp_with_exit_1, None)
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
_dp_tmp_3 = __dp__.with_enter(a)
b = __dp__.getitem(_dp_tmp_3, 0)
_dp_with_exit_2 = __dp__.getitem(_dp_tmp_3, 1)
try:
    _dp_tmp_4 = __dp__.with_enter(c)
    d = __dp__.getitem(_dp_tmp_4, 0)
    _dp_with_exit_1 = __dp__.getitem(_dp_tmp_4, 1)
    try:
        e
    except:
        __dp__.with_exit(_dp_with_exit_1, __dp__.exc_info())
    else:
        __dp__.with_exit(_dp_with_exit_1, None)
except:
    __dp__.with_exit(_dp_with_exit_2, __dp__.exc_info())
else:
    __dp__.with_exit(_dp_with_exit_2, None)
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
    _dp_tmp_2 = await __dp__.with_aenter(a)
    b = __dp__.getitem(_dp_tmp_2, 0)
    _dp_awith_exit_1 = __dp__.getitem(_dp_tmp_2, 1)
    try:
        c
    except:
        await __dp__.with_aexit(_dp_awith_exit_1, __dp__.exc_info())
    else:
        await __dp__.with_aexit(_dp_awith_exit_1, None)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_with_starred_target() {
        let input = r#"
with a as (b, *c):
    pass
"#;
        let expected = r#"
_dp_tmp_2 = __dp__.with_enter(a)
_dp_tmp_3 = __dp__.getitem(_dp_tmp_2, 0)
b = __dp__.getitem(_dp_tmp_3, 0)
c = tuple(__dp__.getitem(_dp_tmp_3, slice(1, None, None)))
_dp_with_exit_1 = __dp__.getitem(_dp_tmp_2, 1)
try:
    pass
except:
    __dp__.with_exit(_dp_with_exit_1, __dp__.exc_info())
else:
    __dp__.with_exit(_dp_with_exit_1, None)
"#;
        assert_transform_eq(input, expected);
    }
}

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
    transformer: &impl Transformer,
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
        __dp__.check_stopiteration()
        c()
        break
    else:
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
        __dp__.check_stopiteration()
        break
    else:
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
            __dp__.acheck_stopiteration()
            c()
            break
        else:
            if cond:
                break
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_for_loop_with_starred_target() {
        let input = r#"
for (a, *b) in c:
    pass
"#;
        let expected = r#"
_dp_iter_1 = __dp__.iter(c)
while True:
    try:
        _dp_tmp_2 = __dp__.next(_dp_iter_1)
        a = __dp__.getitem(_dp_tmp_2, 0)
        b = tuple(__dp__.getitem(_dp_tmp_2, slice(1, None, None)))
    except:
        __dp__.check_stopiteration()
        break
    else:
        pass
"#;
        assert_transform_eq(input, expected);
    }
}

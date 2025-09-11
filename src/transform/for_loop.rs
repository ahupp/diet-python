use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Stmt};

pub struct ForLoopRewriter {
    iter_count: Cell<usize>,
}

impl ForLoopRewriter {
    pub fn new() -> Self {
        Self {
            iter_count: Cell::new(0),
        }
    }
}

impl Transformer for ForLoopRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        if let Stmt::For(ast::StmtFor {
            target,
            iter: iter_expr,
            body,
            orelse,
            is_async,
            ..
        }) = stmt
        {
            let id = self.iter_count.get() + 1;
            self.iter_count.set(id);
            let iter_name = format!("_dp_iter_{}", id);

            let body_stmts = std::mem::take(body);
            let mut orelse_stmts = std::mem::take(orelse);

            let (iter_fn, next_fn, stop_exc, await_) = if *is_async {
                (
                    crate::py_expr!("__dp__.aiter"),
                    crate::py_expr!("__dp__.anext"),
                    "StopAsyncIteration",
                    "await ",
                )
            } else {
                (
                    crate::py_expr!("__dp__.iter"),
                    crate::py_expr!("__dp__.next"),
                    "StopIteration",
                    "",
                )
            };

            orelse_stmts.push(crate::py_stmt!("break"));

            let wrapper = crate::py_stmt!(
                "
{iter_name:id} = {iter_fn:expr}({iter:expr})
while True:
    try:
        {target:expr} = {await_:id}{next_fn:expr}({iter_name:id})
    except {stop_exc:id}:
        {orelse:stmt}
    {body:stmt}
",
                iter_name = iter_name.as_str(),
                iter_fn = iter_fn,
                iter = *iter_expr.clone(),
                target = *target.clone(),
                await_ = await_,
                next_fn = next_fn,
                stop_exc = stop_exc,
                orelse = orelse_stmts,
                body = body_stmts,
            );

            *stmt = wrapper;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite_for(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();

        let rewriter = ForLoopRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_for_loop_with_else() {
        let input = r#"
for a in b:
    if a % 2 == 0:
        c(a)
    else:
        break
else:
    c(0)
"#;
        let expected = r#"
_dp_iter_1 = __dp__.iter(b)
while True:
    try:
        a = __dp__.next(_dp_iter_1)
    except StopIteration:
        c(0)
        break
    if a % 2 == 0:
        c(a)
    else:
        break
"#;
        let output = rewrite_for(input);
        assert_flatten_eq!(output, expected);
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
    except StopIteration:
        break
    c(a)
"#;
        let output = rewrite_for(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_async_for_loop_with_else() {
        let input = r#"
async def f():
    async for a in b:
        if a % 2 == 0:
            c(a)
        else:
            break
    else:
        c(0)
"#;
        let expected = r#"
async def f():
    _dp_iter_1 = __dp__.aiter(b)
    while True:
        try:
            a = await __dp__.anext(_dp_iter_1)
        except StopAsyncIteration:
            c(0)
            break
        if a % 2 == 0:
            c(a)
        else:
            break
"#;
        let output = rewrite_for(input);
        assert_flatten_eq!(output, expected);
    }
}

use crate::py_stmt;
use crate::transform::driver::{ExprRewriter, Rewrite};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::{TextRange, TextSize};

fn non_placeholder_pass() -> Vec<Stmt> {
    let mut stmts = py_stmt!("pass");
    if let Some(Stmt::Pass(pass)) = stmts.get_mut(0) {
        pass.range = TextRange::new(TextSize::new(0), TextSize::new(4));
    }
    stmts
}

pub fn rewrite_for(
    ast::StmtFor {
        target,
        iter,
        body,
        orelse,
        is_async,
        ..
    }: ast::StmtFor,
    rewriter: &mut ExprRewriter,
) -> Rewrite {
    let iter_name = rewriter.context().fresh("iter");
    let orelse = if orelse.is_empty() {
        non_placeholder_pass()
    } else {
        orelse
    };

    Rewrite::Visit(if is_async {
        py_stmt!(
            r#"
{iter_name:id} = __dp__.aiter({iter:expr})
try:
    while True:
        {target:expr} = await __dp__.anext({iter_name:id})
        {body:stmt}
    {orelse:stmt}
except StopAsyncIteration:
    pass
"#,
            iter_name = iter_name.as_str(),
            iter = iter,
            target = target,
            body = body,
            orelse = orelse,
        )
    } else {
        py_stmt!(
            r#"
{iter_name:id} = __dp__.iter({iter:expr})
try:
    while True:
        {target:expr} = __dp__.next({iter_name:id})
        {body:stmt}
    {orelse:stmt}
except StopIteration:
    pass
"#,
            iter_name = iter_name.as_str(),
            iter = iter,
            target = target,
            body = body,
            orelse = orelse,
        )
    })
}

pub fn rewrite_while(while_stmt: ast::StmtWhile, rewriter: &mut ExprRewriter) -> Rewrite {

    // Since the is written to another (simpler) while loop, avoid infinite rewrite
    if while_stmt.orelse.is_empty()
        && matches!(
            while_stmt.test.as_ref(),
            Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })
        )
    {
        return Rewrite::Walk(vec![Stmt::While(while_stmt)]);
    }

    let ast::StmtWhile { test, body, orelse, .. } = while_stmt;
    let orelse = if orelse.is_empty() {
        non_placeholder_pass()
    } else {
        orelse
    };

    // Move the test into the loop body so a) if/when the test expression is lowered, 
    // any new statements are re-evaluated each loop, and b) to explicitly handle the or-else case that only runs if 
    // there was no break
    // The orelse handling needs to be outside the loop in case it has a break, where it should apply to the outer loop.

    let test_flag : String = rewriter.context().fresh("test_flag");

    Rewrite::Walk(py_stmt!(
        r#"
{test_flag:id} = True
while True:
    {test_flag:id} = {test:expr}
    if not {test_flag:id}:
        break
    {body:stmt}
if {test_flag:id}:
    {orelse:stmt}
"#,
        test = *test,
        body = body,
        orelse = orelse,
        test_flag = test_flag.as_str(),
    ))
}

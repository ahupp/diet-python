use crate::transform::ast_rewrite::Rewrite;
use crate::transform::context::Context;
use crate::{py_stmt};
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
    context: &Context,
    ast::StmtFor {
        target,
        iter,
        body,
        orelse,
        is_async,
        ..
    }: ast::StmtFor,
) -> Rewrite {
    let iter_name = context.fresh("iter");
    let has_orelse = !orelse.is_empty();
    if has_orelse {
        let is_complete_name = context.fresh("is_complete");
        return Rewrite::Visit(if is_async {
            py_stmt!(
                r#"
{iter_name:id} = __dp__.aiter({iter:expr})
{is_complete:id} = False
try:
    while True:
        {target:expr} = await __dp__.anext({iter_name:id})
        {body:stmt}
except StopAsyncIteration:
    {is_complete:id} = True
if {is_complete:id}:
    {orelse:stmt}
"#,
                iter_name = iter_name.as_str(),
                iter = iter,
                target = target,
                body = body,
                orelse = orelse,
                is_complete = is_complete_name.as_str(),
            )
        } else {
            py_stmt!(
                r#"
{iter_name:id} = __dp__.iter({iter:expr})
{is_complete:id} = False
try:
    while True:
        {target:expr} = __dp__.next({iter_name:id})
        {body:stmt}
except StopIteration:
    {is_complete:id} = True
if {is_complete:id}:
    {orelse:stmt}
"#,
                iter_name = iter_name.as_str(),
                iter = iter,
                target = target,
                body = body,
                orelse = orelse,
                is_complete = is_complete_name.as_str(),
            )
        });
    }

    Rewrite::Visit(if is_async {
        py_stmt!(
            r#"
{iter_name:id} = __dp__.aiter({iter:expr})
try:
    while True:
        {target:expr} = await __dp__.anext({iter_name:id})
        {body:stmt}
except StopAsyncIteration:
    pass
"#,
            iter_name = iter_name.as_str(),
            iter = iter,
            target = target,
            body = body,
        )
    } else {
        py_stmt!(
            r#"
{iter_name:id} = __dp__.iter({iter:expr})
try:
    while True:
        {target:expr} = __dp__.next({iter_name:id})
        {body:stmt}
except StopIteration:
    pass
"#,
            iter_name = iter_name.as_str(),
            iter = iter,
            target = target,
            body = body,
        )
    })
}

pub fn rewrite_while(context: &Context, while_stmt: ast::StmtWhile) -> Rewrite {

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
    let has_orelse = !orelse.is_empty();
    let orelse = if has_orelse {
        orelse
    } else {
        non_placeholder_pass()
    };

    // Move the test into the loop body so a) if/when the test expression is lowered, 
    // any new statements are re-evaluated each loop, and b) to explicitly handle the or-else case that only runs if 
    // there was no break
    // The orelse handling needs to be outside the loop in case it has a break, where it should apply to the outer loop.

    let test_flag : String = context.fresh("test_flag");

    Rewrite::Walk(py_stmt!(
            r#"
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
        )
    )
}

pub fn expand_if_chain(mut if_stmt: ast::StmtIf) -> ast::StmtIf {
    let mut else_body: Option<Vec<Stmt>> = None;

    for clause in if_stmt.elif_else_clauses.into_iter().rev() {
        match clause.test {
            Some(test) => {
                let mut nested_if = ast::StmtIf {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    test: Box::new(test),
                    body: clause.body,
                    elif_else_clauses: Vec::new(),
                };

                if let Some(body) = else_body.take() {
                    nested_if.elif_else_clauses.push(ast::ElifElseClause {
                        test: None,
                        body,
                        range: TextRange::default(),
                        node_index: ast::AtomicNodeIndex::default(),
                    });
                }

                else_body = Some(vec![Stmt::If(nested_if)]);
            }
            None => {
                else_body = Some(clause.body);
            }
        }
    }

    if let Some(body) = else_body {
        if_stmt.elif_else_clauses = vec![ast::ElifElseClause {
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
            test: None,
            body,
        }];
    } else {
        if_stmt.elif_else_clauses = Vec::new();
    }

    if_stmt
}

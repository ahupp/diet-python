

use crate::transform::rewrite_expr::lower_expr;
use crate::{py_stmt, transform::ast_rewrite::Rewrite};
use crate::transform::context::Context;

use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::{TextRange};


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
    let iter_tmp = context.fresh("iter");
    let target_tmp = context.fresh("tmp");
    let completed_flag = context.fresh("completed");

    Rewrite::Walk(if is_async {
            py_stmt!(
                r#"
{completed_flag:id} = False            
{iter_name:id} = __dp__.aiter({iter:expr})
while not {completed_flag:id}:
    {target_tmp:id} = await __dp__.anext_or_sentinel({iter_name:id})
    if {target_tmp:id} is __dp__.ITER_COMPLETE:
        {completed_flag:id} = True
    else:
        {target:expr} = {target_tmp:id}
        {body:stmt}
if {completed_flag:id}:
    {orelse:stmt}
"#,
                iter_name = iter_tmp.as_str(),
                iter = iter,
                target = target,
                body = body,
                orelse = orelse,
                target_tmp = target_tmp.as_str(),
                completed_flag = completed_flag.as_str(),
            )
        } else {
            py_stmt!(
                r#"
{completed_flag:id} = False            
{iter_name:id} = __dp__.iter({iter:expr})
while not {completed_flag:id}:
    {target_tmp:id} = __dp__.next_or_sentinel({iter_name:id})
    if {target_tmp:id} is __dp__.ITER_COMPLETE:
        {completed_flag:id} = True
    else:
        {target:expr} = {target_tmp:id}
        {body:stmt}
if {completed_flag:id}:
    {orelse:stmt}
"#,
                iter_name = iter_tmp.as_str(),
                iter = iter,
                target = target,
                body = body,
                orelse = orelse,
                target_tmp = target_tmp.as_str(),
                completed_flag = completed_flag.as_str(),
            )
        })
}

pub fn rewrite_while(context: &Context, while_stmt: ast::StmtWhile) -> Rewrite {

    let test_lowered = lower_expr(context, *while_stmt.test.clone());

    if !test_lowered.modified
    {
        return Rewrite::Unmodified(while_stmt.into());
    }

    let ast::StmtWhile {  body, orelse, .. } = while_stmt;

    let did_exit_normally = context.fresh("did_exit_normally");
    // Move the test into the loop body so a) if/when the test expression is
    // lowered, any new statements are re-evaluated each loop, and b) to
    // explicitly handle the or-else case that only runs if there was no break
    // The orelse handling needs to be outside the loop in case it has a break,
    // where it should apply to the outer loop.

    Rewrite::Walk(py_stmt!(r#"
{did_exit_normally:id} = False
while True:
    {test_lowered:stmt}
    if not {test_expr:expr}:
        {did_exit_normally:id} = True
        break
    {body:stmt}
if {did_exit_normally:id}:
    {orelse:stmt}
"#,
            test_expr = test_lowered.expr,
            test_lowered = test_lowered.stmt,
            body = body,
            orelse = orelse,
            did_exit_normally = did_exit_normally.as_str(),
        )
    )
}

pub fn expand_if_chain(mut if_stmt: ast::StmtIf) -> Rewrite {
    if !if_stmt.elif_else_clauses
        .iter()
        .any(|clause| clause.test.is_some()) {
            return Rewrite::Unmodified(if_stmt.into());
    }
    let mut else_body: Option<ast::StmtBody> = None;

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

                else_body = Some(ast::StmtBody {
                    body: vec![Box::new(Stmt::If(nested_if))],
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                });
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

    Rewrite::Walk(if_stmt.into())
}

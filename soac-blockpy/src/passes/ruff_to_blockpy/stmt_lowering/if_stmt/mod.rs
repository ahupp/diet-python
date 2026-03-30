use super::*;
use crate::passes::ast_to_ast::ast_rewrite::Rewrite;
use crate::passes::ast_to_ast::body::Suite;
use ruff_text_size::TextRange;

pub(crate) fn expand_if_chain(mut if_stmt: ast::StmtIf) -> Rewrite {
    if !if_stmt
        .elif_else_clauses
        .iter()
        .any(|clause| clause.test.is_some())
    {
        return Rewrite::Unmodified(if_stmt.into());
    }
    let mut else_body: Option<Suite> = None;

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
                let mut body = clause.body;
                else_body = Some(std::mem::take(&mut body));
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

    Rewrite::Walk(vec![if_stmt.into()])
}

impl StmtLowerer for ast::StmtIf {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(expand_if_chain(self))
    }

    fn to_blockpy<E>(
        &self,
        context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        match simplify_stmt_head_ast_for_blockpy(context, Stmt::If(self.clone())).as_slice() {
            [Stmt::If(simplified_if)] => {
                let body = lower_nested_body_to_stmts_with_expr(
                    context,
                    &simplified_if.body,
                    loop_ctx,
                    next_label_id,
                )?;
                let orelse = lower_orelse_to_stmts_with_expr(
                    context,
                    &simplified_if.elif_else_clauses,
                    &Stmt::If(simplified_if.clone()),
                    loop_ctx,
                    next_label_id,
                )?;
                let test =
                    crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
                        (*simplified_if.test).clone(),
                        out,
                        loop_ctx,
                        next_label_id,
                    )?;
                out.push_stmt(StructuredBlockPyStmt::If(BlockPyIf { test, body, orelse }));
                Ok(())
            }
            expanded => {
                for stmt in expanded {
                    lower_nested_stmt_into_with_expr(context, stmt, out, loop_ctx, next_label_id)?;
                }
                Ok(())
            }
        }
    }
}

fn lower_nested_body_to_stmts_with_expr<E>(
    context: &Context,
    body: &Suite,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<crate::block_py::BlockPyCfgFragment<StructuredBlockPyStmt<E>, BlockPyTerm<E>>, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<
        StructuredBlockPyStmt<E>,
        BlockPyTerm<E>,
    >::new();
    for stmt in body {
        lower_nested_stmt_into_with_expr(context, stmt, &mut out, loop_ctx, next_label_id)?;
    }
    Ok(out.finish())
}

fn lower_orelse_to_stmts_with_expr<E>(
    context: &Context,
    clauses: &[ast::ElifElseClause],
    stmt: &Stmt,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<crate::block_py::BlockPyCfgFragment<StructuredBlockPyStmt<E>, BlockPyTerm<E>>, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    match clauses {
        [] => Ok(crate::block_py::BlockPyCfgFragment::<
            StructuredBlockPyStmt<E>,
            BlockPyTerm<E>,
        >::from_stmts(Vec::new())),
        [clause] if clause.test.is_none() => {
            lower_nested_body_to_stmts_with_expr(context, &clause.body, loop_ctx, next_label_id)
        }
        _ => Err(format!(
            "`elif` chain reached Ruff AST -> BlockPy conversion\nstmt:\n{}",
            ruff_ast_to_string(stmt).trim_end()
        )),
    }
}

#[cfg(test)]
mod test;

use super::*;
use crate::basic_block::ast_to_ast::ast_rewrite::Rewrite;
use ruff_text_size::TextRange;

pub(crate) fn expand_if_chain(mut if_stmt: ast::StmtIf) -> Rewrite {
    if !if_stmt
        .elif_else_clauses
        .iter()
        .any(|clause| clause.test.is_some())
    {
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

impl StmtLowerer for ast::StmtIf {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        stmt_from_rewrite(expand_if_chain(self))
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
        match simplify_stmt_head_ast_for_blockpy(context, Stmt::If(self.clone())) {
            Stmt::If(simplified_if) => {
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
                    crate::basic_block::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
                        context,
                        (*simplified_if.test).clone(),
                        out,
                        loop_ctx,
                        next_label_id,
                    )?;
                out.push_stmt(BlockPyStmt::If(BlockPyIf { test, body, orelse }));
                Ok(())
            }
            Stmt::BodyStmt(body) => {
                for stmt in &body.body {
                    lower_nested_stmt_into_with_expr(
                        context,
                        stmt.as_ref(),
                        out,
                        loop_ctx,
                        next_label_id,
                    )?;
                }
                Ok(())
            }
            other => panic!(
                "if simplification should remain an if or expand to a body stmt, got:\n{}",
                ruff_ast_to_string(&other).trim_end()
            ),
        }
    }
}

fn lower_nested_body_to_stmts_with_expr<E>(
    context: &Context,
    body: &StmtBody,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<crate::basic_block::block_py::BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
        BlockPyStmt<E>,
        BlockPyTerm<E>,
    >::new();
    for stmt in &body.body {
        lower_nested_stmt_into_with_expr(
            context,
            stmt.as_ref(),
            &mut out,
            loop_ctx,
            next_label_id,
        )?;
    }
    Ok(out.finish())
}

fn lower_orelse_to_stmts_with_expr<E>(
    context: &Context,
    clauses: &[ast::ElifElseClause],
    stmt: &Stmt,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<crate::basic_block::block_py::BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    match clauses {
        [] => Ok(crate::basic_block::block_py::BlockPyCfgFragment::<
            BlockPyStmt<E>,
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
mod tests {
    use super::super::{simplify_stmt_ast_for_blockpy, BlockPyStmtFragmentBuilder};
    use super::*;
    use crate::basic_block::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_if_simplify_ast_expands_elif_chain_before_blockpy_lowering() {
        let stmt = py_stmt!("if x:\n    a()\nelif y:\n    b()\nelse:\n    c()");
        let Stmt::If(if_stmt) = stmt else {
            panic!("expected if stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let Stmt::If(simplified_if) = simplify_stmt_ast_for_blockpy(&context, Stmt::If(if_stmt))
        else {
            panic!("if simplification should remain an if stmt");
        };

        assert_eq!(simplified_if.elif_else_clauses.len(), 1);
        let clause = &simplified_if.elif_else_clauses[0];
        assert!(clause.test.is_none());
        assert!(matches!(clause.body.body[0].as_ref(), Stmt::If(_)));
    }

    #[test]
    fn stmt_if_to_blockpy_uses_trait_owned_simplification_path_for_elif() {
        let stmt = py_stmt!("if x:\n    a()\nelif y:\n    b()\nelse:\n    c()");
        let Stmt::If(if_stmt) = stmt else {
            panic!("expected if stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<BlockPyExpr>::new();
        let mut next_label_id = 0usize;

        if_stmt
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("if lowering should succeed");

        let fragment = out.finish();
        let [BlockPyStmt::If(lowered_if)] = fragment.body.as_slice() else {
            panic!("expected one lowered if stmt");
        };
        assert!(matches!(
            lowered_if.orelse.body.as_slice(),
            [BlockPyStmt::If(_)]
        ));
    }
}

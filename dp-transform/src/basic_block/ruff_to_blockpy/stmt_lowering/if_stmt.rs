use super::*;

impl StmtLowerer for ast::StmtIf {
    fn simplify_ast(self) -> Stmt {
        stmt_from_rewrite(
            crate::basic_block::ast_to_ast::rewrite_stmt::loop_cond::expand_if_chain(self),
        )
    }

    fn to_blockpy<E>(
        &self,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        let Stmt::If(simplified_if) = self.clone().simplify_ast() else {
            panic!("if simplification should remain an if stmt");
        };
        let body =
            lower_nested_body_to_stmts_with_expr(&simplified_if.body, loop_ctx, next_label_id)?;
        let orelse = lower_orelse_to_stmts_with_expr(
            &simplified_if.elif_else_clauses,
            &Stmt::If(simplified_if.clone()),
            loop_ctx,
            next_label_id,
        )?;
        out.push_stmt(BlockPyStmt::If(BlockPyIf {
            test: (*simplified_if.test).clone().into(),
            body,
            orelse,
        }));
        Ok(())
    }
}

fn lower_nested_body_to_stmts_with_expr<E>(
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
        lower_stmt_into_with_expr(stmt.as_ref(), &mut out, loop_ctx, next_label_id)?;
    }
    Ok(out.finish())
}

fn lower_orelse_to_stmts_with_expr<E>(
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
            lower_nested_body_to_stmts_with_expr(&clause.body, loop_ctx, next_label_id)
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

    #[test]
    fn stmt_if_simplify_ast_expands_elif_chain_before_blockpy_lowering() {
        let stmt = py_stmt!("if x:\n    a()\nelif y:\n    b()\nelse:\n    c()");
        let Stmt::If(if_stmt) = stmt else {
            panic!("expected if stmt");
        };

        let Stmt::If(simplified_if) = simplify_stmt_ast_for_blockpy(Stmt::If(if_stmt)) else {
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
        let mut out = BlockPyStmtFragmentBuilder::<BlockPyExpr>::new();
        let mut next_label_id = 0usize;

        if_stmt
            .to_blockpy(&mut out, None, &mut next_label_id)
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

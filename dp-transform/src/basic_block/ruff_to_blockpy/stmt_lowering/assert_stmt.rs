use super::*;

impl StmtLowerer for ast::StmtAssert {
    fn simplify_ast(self) -> Stmt {
        stmt_from_rewrite(crate::basic_block::ast_to_ast::rewrite_stmt::assert::rewrite(self))
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
        lower_stmt_via_simplify(self, out, loop_ctx, next_label_id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{simplify_stmt_ast_for_blockpy, BlockPyStmtFragmentBuilder};
    use super::*;

    #[test]
    fn stmt_assert_simplify_ast_desugars_before_blockpy_lowering() {
        let stmt = py_stmt!("assert cond, msg");
        let Stmt::Assert(assert_stmt) = stmt else {
            panic!("expected assert stmt");
        };

        let simplified = simplify_stmt_ast_for_blockpy(Stmt::Assert(assert_stmt));

        assert!(!matches!(simplified, Stmt::Assert(_)));
    }

    #[test]
    fn stmt_assert_to_blockpy_uses_trait_owned_simplification_path() {
        let stmt = py_stmt!("assert cond, msg");
        let Stmt::Assert(assert_stmt) = stmt else {
            panic!("expected assert stmt");
        };
        let mut out = BlockPyStmtFragmentBuilder::<BlockPyExpr>::new();
        let mut next_label_id = 0usize;

        assert_stmt
            .to_blockpy(&mut out, None, &mut next_label_id)
            .expect("assert lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::If(_)]));
    }
}

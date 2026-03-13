use super::*;

impl StmtLowerer for ast::StmtDelete {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        stmt_from_rewrite(
            crate::basic_block::ast_to_ast::rewrite_stmt::assign_del::rewrite_delete(self),
        )
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        _loop_ctx: Option<&LoopContext>,
        _next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        if self.targets.len() != 1 {
            return Err(assign_delete_error(
                "multi-target delete reached BlockPy conversion",
                &Stmt::Delete(self.clone()),
            ));
        }
        let Some(target) = self.targets[0].as_name_expr().cloned() else {
            return Err(assign_delete_error(
                "non-name delete target reached BlockPy conversion",
                &Stmt::Delete(self.clone()),
            ));
        };
        out.push_stmt(BlockPyStmt::Delete(BlockPyDelete { target }));
        Ok(())
    }
}

use super::*;

impl StmtLowerer for ast::StmtAugAssign {
    fn simplify_ast(self, context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(super::assign_stmt::rewrite_augassign_stmt(context, self))
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
        lower_stmt_via_simplify(context, self, out, loop_ctx, next_label_id)
    }
}

#[cfg(test)]
mod test;

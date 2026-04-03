use super::*;

impl StmtLowerer for ast::StmtImportFrom {
    fn simplify_ast(self, context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(crate::passes::ast_to_ast::rewrite_import::rewrite_from(
            context, self,
        ))
    }

    fn to_blockpy<E>(
        &self,
        context: &Context,
        out: &mut BlockPyStmtBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: RuffToBlockPyExpr,
    {
        lower_stmt_via_simplify(context, self, out, loop_ctx, next_label_id)
    }
}

#[cfg(test)]
mod test;

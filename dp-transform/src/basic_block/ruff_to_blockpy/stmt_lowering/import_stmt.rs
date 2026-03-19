use super::*;

impl StmtLowerer for ast::StmtImport {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(crate::basic_block::ast_to_ast::rewrite_import::rewrite(
            self,
        ))
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
mod tests {
    use super::super::{simplify_stmt_ast_for_blockpy, BlockPyStmtFragmentBuilder};
    use super::*;
    use crate::basic_block::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_import_simplify_ast_desugars_before_blockpy_lowering() {
        let stmt = py_stmt!("import pkg.sub");
        let Stmt::Import(import_stmt) = stmt else {
            panic!("expected import stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_for_blockpy(&context, Stmt::Import(import_stmt));

        assert!(!matches!(simplified.as_slice(), [Stmt::Import(_)]));
    }

    #[test]
    fn stmt_import_to_blockpy_uses_trait_owned_simplification_path() {
        let stmt = py_stmt!("import pkg.sub");
        let Stmt::Import(import_stmt) = stmt else {
            panic!("expected import stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        import_stmt
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("import lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::Assign(_)]));
    }
}

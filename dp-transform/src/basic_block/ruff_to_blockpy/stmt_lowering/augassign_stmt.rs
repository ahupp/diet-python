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
mod tests {
    use super::super::{simplify_stmt_ast_once_for_blockpy, BlockPyStmtFragmentBuilder};
    use super::*;
    use crate::basic_block::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_augassign_simplify_ast_desugars_before_blockpy_lowering() {
        let stmt = py_stmt!("x += y");
        let Stmt::AugAssign(aug_stmt) = stmt else {
            panic!("expected augassign stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::AugAssign(aug_stmt));

        assert!(!matches!(simplified.as_slice(), [Stmt::AugAssign(_)]));
    }

    #[test]
    fn stmt_augassign_to_blockpy_uses_trait_owned_simplification_path() {
        let stmt = py_stmt!("x += y");
        let Stmt::AugAssign(aug_stmt) = stmt else {
            panic!("expected augassign stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        aug_stmt
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("augassign lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::Assign(_)]));
    }
}

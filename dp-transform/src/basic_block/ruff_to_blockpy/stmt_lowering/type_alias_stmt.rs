use super::*;

impl StmtLowerer for ast::StmtTypeAlias {
    fn simplify_ast(self, context: &Context) -> Stmt {
        stmt_from_rewrite(
            crate::basic_block::ast_to_ast::rewrite_stmt::type_alias::rewrite_type_alias(
                context, self,
            ),
        )
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
    fn stmt_type_alias_simplify_ast_desugars_before_blockpy_lowering() {
        let stmt = py_stmt!("type X = int");
        let Stmt::TypeAlias(type_alias) = stmt else {
            panic!("expected type alias stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_for_blockpy(&context, Stmt::TypeAlias(type_alias));

        assert!(!matches!(simplified, Stmt::TypeAlias(_)));
    }

    #[test]
    fn stmt_type_alias_to_blockpy_uses_trait_owned_simplification_path() {
        let stmt = py_stmt!("type X = int");
        let Stmt::TypeAlias(type_alias) = stmt else {
            panic!("expected type alias stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<BlockPyExpr>::new();
        let mut next_label_id = 0usize;

        type_alias
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("type alias lowering should succeed");

        let fragment = out.finish();
        assert!(!fragment.body.is_empty());
    }
}

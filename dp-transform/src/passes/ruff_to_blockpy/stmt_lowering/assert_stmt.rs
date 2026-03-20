use super::*;
use crate::{passes::ast_to_ast::ast_rewrite::Rewrite, py_stmt};

pub(crate) fn rewrite_assert_stmt(ast::StmtAssert { test, msg, .. }: ast::StmtAssert) -> Rewrite {
    Rewrite::Walk(vec![if let Some(msg_expr) = msg {
        py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise __dp_AssertionError({msg:expr})
",
            test = test,
            msg = *msg_expr
        )
    } else {
        py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise __dp_AssertionError
        ",
            test = test
        )
    }])
}

impl StmtLowerer for ast::StmtAssert {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(rewrite_assert_stmt(self))
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
    use crate::passes::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_assert_simplify_ast_desugars_before_blockpy_lowering() {
        let stmt = py_stmt!("assert cond, msg");
        let Stmt::Assert(assert_stmt) = stmt else {
            panic!("expected assert stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Assert(assert_stmt));

        assert!(!matches!(simplified.as_slice(), [Stmt::Assert(_)]));
    }

    #[test]
    fn stmt_assert_to_blockpy_uses_trait_owned_simplification_path() {
        let stmt = py_stmt!("assert cond, msg");
        let Stmt::Assert(assert_stmt) = stmt else {
            panic!("expected assert stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        assert_stmt
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("assert lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::If(_)]));
    }
}

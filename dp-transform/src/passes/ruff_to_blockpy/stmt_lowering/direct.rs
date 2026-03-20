use super::*;
use crate::passes::ast_to_ast::ast_rewrite::Rewrite;
use crate::py_stmt;

// Direct stmt lowerers map one Ruff stmt to either a BlockPy stmt, a terminator,
// or no output at all. They do not need their own AST rewrite helpers.

pub(crate) fn rewrite_raise_stmt(mut raise: ast::StmtRaise) -> Rewrite {
    match (raise.exc.take(), raise.cause.take()) {
        (Some(exc), Some(cause)) => Rewrite::Walk(vec![py_stmt!(
            "raise __dp_raise_from({exc:expr}, {cause:expr})",
            exc = exc,
            cause = cause,
        )]),
        (exc, None) => {
            raise.exc = exc;
            Rewrite::Unmodified(raise.into())
        }
        (None, Some(_)) => {
            panic!("raise with a cause but without an exception should be impossible")
        }
    }
}

impl StmtLowerer for ast::StmtGlobal {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        single_stmt(Stmt::Global(self))
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        _out: &mut BlockPyStmtFragmentBuilder<E>,
        _loop_ctx: Option<&LoopContext>,
        _next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        Ok(())
    }
}

impl StmtLowerer for ast::StmtNonlocal {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        single_stmt(Stmt::Nonlocal(self))
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        _out: &mut BlockPyStmtFragmentBuilder<E>,
        _loop_ctx: Option<&LoopContext>,
        _next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        Ok(())
    }
}

impl StmtLowerer for ast::StmtPass {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        single_stmt(Stmt::Pass(self))
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        _out: &mut BlockPyStmtFragmentBuilder<E>,
        _loop_ctx: Option<&LoopContext>,
        _next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        Ok(())
    }
}

impl StmtLowerer for ast::StmtExpr {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        single_stmt(Stmt::Expr(self))
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        let value = crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
            (*self.value).clone(),
            out,
            loop_ctx,
            next_label_id,
        )?;
        out.push_stmt(BlockPyStmt::Expr(value));
        Ok(())
    }
}

impl StmtLowerer for ast::StmtBreak {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        single_stmt(Stmt::Break(self))
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        _next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        if let Some(loop_ctx) = loop_ctx {
            out.set_term(BlockPyTerm::Jump(loop_ctx.break_label.clone().into()));
            Ok(())
        } else {
            panic!("Break should be lowered before Ruff AST -> BlockPy conversion");
        }
    }
}

impl StmtLowerer for ast::StmtContinue {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        single_stmt(Stmt::Continue(self))
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        _next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        if let Some(loop_ctx) = loop_ctx {
            out.set_term(BlockPyTerm::Jump(loop_ctx.continue_label.clone().into()));
            Ok(())
        } else {
            panic!("Continue should be lowered before Ruff AST -> BlockPy conversion");
        }
    }
}

impl StmtLowerer for ast::StmtReturn {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        single_stmt(Stmt::Return(self))
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        let value = match self.value.as_ref() {
            Some(value) => Some(
                crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
                    (**value).clone(),
                    out,
                    loop_ctx,
                    next_label_id,
                )?,
            ),
            None => None,
        };
        out.set_term(BlockPyTerm::Return(value));
        Ok(())
    }
}

impl StmtLowerer for ast::StmtRaise {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(rewrite_raise_stmt(self))
    }

    fn to_blockpy<E>(
        &self,
        _context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        if self.cause.is_some() {
            panic!("raise-from should be lowered before Ruff AST -> BlockPy conversion");
        }
        let exc = match self.exc.as_ref() {
            Some(exc) => Some(
                crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
                    (**exc).clone(),
                    out,
                    loop_ctx,
                    next_label_id,
                )?,
            ),
            None => None,
        };
        out.set_term(BlockPyTerm::Raise(BlockPyRaise { exc }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{simplify_stmt_ast_once_for_blockpy, BlockPyStmtFragmentBuilder};
    use super::*;
    use crate::passes::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_raise_simplify_ast_desugars_raise_from_before_blockpy_lowering() {
        let stmt = py_stmt!("raise exc from cause");
        let Stmt::Raise(raise_stmt) = stmt else {
            panic!("expected raise stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Raise(raise_stmt));

        assert!(!matches!(
            simplified.as_slice(),
            [Stmt::Raise(ast::StmtRaise { cause: Some(_), .. })]
        ));
    }

    #[test]
    fn stmt_raise_to_blockpy_handles_bare_raise_directly() {
        let stmt = py_stmt!("raise");
        let Stmt::Raise(raise_stmt) = stmt else {
            panic!("expected raise stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        raise_stmt
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("raise lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(fragment.term, Some(BlockPyTerm::Raise(_))));
    }

    #[test]
    fn stmt_expr_to_blockpy_emits_setup_for_named_exprs() {
        let stmt = py_stmt!("(x := y)");
        let Stmt::Expr(expr_stmt) = stmt else {
            panic!("expected expr stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        expr_stmt
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("expr lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(
            fragment.body.as_slice(),
            [BlockPyStmt::Assign(_), BlockPyStmt::Expr(_)]
        ));
    }

    #[test]
    fn stmt_return_to_blockpy_emits_setup_for_if_exprs() {
        let stmt = py_stmt!("return x if cond else y");
        let Stmt::Return(return_stmt) = stmt else {
            panic!("expected return stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        return_stmt
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("return lowering should succeed");

        let fragment = out.finish();
        assert!(!fragment.body.is_empty());
        assert!(matches!(fragment.term, Some(BlockPyTerm::Return(Some(_)))));
    }
}

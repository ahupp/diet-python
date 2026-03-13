use super::*;

// Direct stmt lowerers map one Ruff stmt to either a BlockPy stmt, a terminator,
// or no output at all. They do not need their own AST rewrite helpers.

impl StmtLowerer for ast::StmtBody {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        Stmt::BodyStmt(self)
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
        for stmt in &self.body {
            lower_stmt_into_with_expr(context, stmt.as_ref(), out, loop_ctx, next_label_id)?;
        }
        Ok(())
    }
}

impl StmtLowerer for ast::StmtGlobal {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        Stmt::Global(self)
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
    fn simplify_ast(self, _context: &Context) -> Stmt {
        Stmt::Nonlocal(self)
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
    fn simplify_ast(self, _context: &Context) -> Stmt {
        Stmt::Pass(self)
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
        out.push_stmt(BlockPyStmt::Pass);
        Ok(())
    }
}

impl StmtLowerer for ast::StmtExpr {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        Stmt::Expr(self)
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
        out.push_stmt(BlockPyStmt::Expr((*self.value).clone().into()));
        Ok(())
    }
}

impl StmtLowerer for ast::StmtBreak {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        Stmt::Break(self)
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
            out.set_term(BlockPyTerm::Jump(loop_ctx.break_label.clone()));
            Ok(())
        } else {
            panic!("Break should be lowered before Ruff AST -> BlockPy conversion");
        }
    }
}

impl StmtLowerer for ast::StmtContinue {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        Stmt::Continue(self)
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
            out.set_term(BlockPyTerm::Jump(loop_ctx.continue_label.clone()));
            Ok(())
        } else {
            panic!("Continue should be lowered before Ruff AST -> BlockPy conversion");
        }
    }
}

impl StmtLowerer for ast::StmtReturn {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        Stmt::Return(self)
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
        out.set_term(BlockPyTerm::Return(
            self.value.as_ref().map(|v| (**v).clone().into()),
        ));
        Ok(())
    }
}

impl StmtLowerer for ast::StmtRaise {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        stmt_from_rewrite(
            crate::basic_block::ast_to_ast::rewrite_stmt::exception::rewrite_raise(self),
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
        if self.cause.is_some() {
            panic!("raise-from should be lowered before Ruff AST -> BlockPy conversion");
        }
        out.set_term(BlockPyTerm::Raise(BlockPyRaise {
            exc: self.exc.as_ref().map(|exc| (**exc).clone().into()),
        }));
        Ok(())
    }
}

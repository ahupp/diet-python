use super::*;
use crate::{passes::ast_to_ast::ast_rewrite::Rewrite, py_stmt};

pub(crate) fn rewrite_assert_stmt(ast::StmtAssert { test, msg, .. }: ast::StmtAssert) -> Rewrite {
    Rewrite::Walk(vec![if let Some(msg_expr) = msg {
        py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise __soac__.AssertionError({msg:expr})
",
            test = test,
            msg = *msg_expr
        )
    } else {
        py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise __soac__.AssertionError
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
        E: RuffToBlockPyExpr,
    {
        lower_stmt_via_simplify(context, self, out, loop_ctx, next_label_id)
    }
}

#[cfg(test)]
mod test;

use crate::passes::ast_to_ast::ast_rewrite::{Rewrite, StmtRewritePass};
use crate::passes::ast_to_ast::context::Context;
use ruff_python_ast::Stmt;

pub struct SingleNamedAssignmentPass;

impl StmtRewritePass for SingleNamedAssignmentPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::Assign(assign) => {
                crate::passes::ruff_to_blockpy::rewrite_assign_stmt(context, assign)
            }
            Stmt::Delete(del) => crate::passes::ruff_to_blockpy::rewrite_delete_stmt(del),
            other => Rewrite::Unmodified(other),
        }
    }
}

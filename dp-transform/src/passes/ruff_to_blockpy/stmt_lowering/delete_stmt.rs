use super::*;
use crate::passes::ast_to_ast::ast_rewrite::Rewrite;

pub(crate) fn rewrite_delete_stmt(delete: ast::StmtDelete) -> Rewrite {
    if !super::assign_stmt::should_rewrite_assignment_targets(&delete.targets) {
        return Rewrite::Unmodified(delete.into());
    }

    let stmts: Vec<Stmt> = delete
        .targets
        .into_iter()
        .map(|target| match target {
            Expr::Subscript(sub) => py_stmt!(
                "__dp_delitem({obj:expr}, {key:expr})",
                obj = sub.value,
                key = sub.slice
            ),
            Expr::Attribute(attr) => {
                py_stmt!(
                    "__dp_delattr({obj:expr}, {name:literal})",
                    obj = attr.value,
                    name = attr.attr.as_str(),
                )
            }
            other => py_stmt!("del {target:expr}", target = other),
        })
        .collect();
    Rewrite::Walk(stmts)
}

impl StmtLowerer for ast::StmtDelete {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(rewrite_delete_stmt(self))
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
        if self.targets.len() != 1 {
            return Err(assign_delete_error(
                "multi-target delete reached BlockPy conversion",
                &Stmt::Delete(self.clone()),
            ));
        }
        let Some(target) = self.targets[0].as_name_expr().cloned() else {
            return Err(assign_delete_error(
                "non-name delete target reached BlockPy conversion",
                &Stmt::Delete(self.clone()),
            ));
        };
        out.push_stmt(BlockPyStmt::Delete(BlockPyDelete { target }));
        Ok(())
    }
}

#[cfg(test)]
mod test;

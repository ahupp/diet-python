use super::*;
use crate::basic_block::ast_to_ast::ast_rewrite::Rewrite;
use crate::template::into_body;

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
    Rewrite::Walk(into_body(stmts))
}

impl StmtLowerer for ast::StmtDelete {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        stmt_from_rewrite(rewrite_delete_stmt(self))
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
mod tests {
    use super::super::{
        lower_stmt_into_with_expr, simplify_stmt_ast_for_blockpy, BlockPyStmtFragmentBuilder,
    };
    use super::*;
    use crate::basic_block::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_delete_simplify_ast_desugars_attribute_delete_before_blockpy_lowering() {
        let stmt = py_stmt!("del obj.attr");
        let Stmt::Delete(delete_stmt) = stmt else {
            panic!("expected delete stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_for_blockpy(&context, Stmt::Delete(delete_stmt));

        assert!(!matches!(simplified, Stmt::Delete(_)));
    }

    #[test]
    fn stmt_delete_lowering_uses_trait_owned_simplification_path() {
        let stmt = py_stmt!("del obj.attr");
        let Stmt::Delete(delete_stmt) = stmt else {
            panic!("expected delete stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;
        let simplified = simplify_stmt_ast_for_blockpy(&context, Stmt::Delete(delete_stmt));

        lower_stmt_into_with_expr(&context, &simplified, &mut out, None, &mut next_label_id)
            .expect("delete lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::Expr(_)]));
    }
}

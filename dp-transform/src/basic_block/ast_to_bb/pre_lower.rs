use super::*;

pub struct BBSimplifyStmtPass;
pub(super) struct AnnotationHelperForLoweringPass;
pub type FunctionIdentityByNode = HashMap<NodeIndex, (String, String, String, BindingTarget)>;

pub(crate) fn lower_stmt_default(context: &Context, stmt: Stmt) -> Rewrite {
    match stmt {
        Stmt::Try(try_stmt) => rewrite_stmt::exception::rewrite_try(try_stmt),
        Stmt::If(if_stmt) => rewrite_stmt::loop_cond::expand_if_chain(if_stmt),
        Stmt::Assert(assert) => rewrite_stmt::assert::rewrite(assert),
        Stmt::Match(match_stmt) => rewrite_stmt::match_case::rewrite(context, match_stmt),
        Stmt::Import(import) => rewrite_import::rewrite(import),
        Stmt::ImportFrom(import_from) => rewrite_import::rewrite_from(context, import_from),
        Stmt::Assign(assign) => rewrite_stmt::assign_del::rewrite_assign(context, assign),
        Stmt::AugAssign(aug) => rewrite_stmt::assign_del::rewrite_aug_assign(context, aug),
        Stmt::Delete(del) => rewrite_stmt::assign_del::rewrite_delete(del),
        Stmt::Raise(raise) => rewrite_stmt::exception::rewrite_raise(raise),
        Stmt::TypeAlias(type_alias) => {
            rewrite_stmt::type_alias::rewrite_type_alias(context, type_alias)
        }
        Stmt::AnnAssign(_) => {
            panic!("should be removed by rewrite_ann_assign_to_dunder_annotate")
        }
        other => Rewrite::Unmodified(other),
    }
}

pub(crate) fn lower_stmt_bb(context: &Context, stmt: Stmt) -> Rewrite {
    match stmt {
        Stmt::Try(try_stmt) => lower_stmt_default(context, Stmt::Try(try_stmt)),
        other => lower_stmt_default(context, other),
    }
}

impl StmtRewritePass for AnnotationHelperForLoweringPass {
    fn lower_stmt(&self, _context: &Context, stmt: Stmt) -> Rewrite {
        match stmt {
            other => Rewrite::Unmodified(other),
        }
    }
}

impl StmtRewritePass for BBSimplifyStmtPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        lower_stmt_bb(context, stmt)
    }
}

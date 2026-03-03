use super::*;

pub struct BBSimplifyStmtPass;
pub(super) struct AnnotationHelperForLoweringPass;
pub type FunctionIdentityByNode = HashMap<NodeIndex, (String, String, String, BindingTarget)>;

pub(crate) fn lower_stmt_default(context: &Context, stmt: Stmt) -> Rewrite {
    match stmt {
        Stmt::With(with) => rewrite_stmt::with::rewrite(context, with),
        Stmt::While(while_stmt) => rewrite_stmt::loop_cond::rewrite_while(context, while_stmt),
        Stmt::For(for_stmt) => rewrite_stmt::loop_cond::rewrite_for(context, for_stmt),
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
        Stmt::With(with_stmt) => rewrite_with_for_bb(context, with_stmt),
        Stmt::Try(try_stmt) => lower_stmt_default(context, Stmt::Try(try_stmt)),
        Stmt::For(for_stmt) => {
            if context.options.emit_basic_blocks {
                Rewrite::Unmodified(Stmt::For(for_stmt))
            } else {
                lower_stmt_default(context, Stmt::For(for_stmt))
            }
        }
        other => lower_stmt_default(context, other),
    }
}

impl StmtRewritePass for AnnotationHelperForLoweringPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::For(for_stmt) => lower_stmt_default(context, Stmt::For(for_stmt)),
            other => Rewrite::Unmodified(other),
        }
    }
}

fn rewrite_with_for_bb(context: &Context, with_stmt: ast::StmtWith) -> Rewrite {
    if with_stmt.is_async {
        return rewrite_stmt::with::rewrite(context, with_stmt);
    }
    if with_stmt.items.is_empty() {
        return Rewrite::Unmodified(with_stmt.into());
    }

    let ast::StmtWith { items, body, .. } = with_stmt;
    let mut body: Stmt = body.into();

    for ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } in items.into_iter().rev()
    {
        let target = optional_vars.map(|var| *var);
        let exit_name = context.fresh("with_exit");
        let ok_name = context.fresh("with_ok");
        let body_needs_transfer_safe_cleanup = contains_control_transfer_stmt(&body);

        let ctx_placeholder = context.maybe_placeholder_lowered(context_expr);
        let ctx_cleanup = if ctx_placeholder.modified {
            py_stmt!("{ctx:expr} = None", ctx = ctx_placeholder.expr.clone())
        } else {
            empty_body().into()
        };

        let enter_stmt = if let Some(target) = target {
            py_stmt!(
                "{target:expr} = __dp_contextmanager_enter({ctx:expr})",
                target = target,
                ctx = ctx_placeholder.expr.clone(),
            )
        } else {
            py_stmt!(
                "__dp_contextmanager_enter({ctx:expr})",
                ctx = ctx_placeholder.expr.clone(),
            )
        };

        body = if body_needs_transfer_safe_cleanup {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp_contextmanager_get_exit({ctx_placeholder_expr_1:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    __dp_contextmanager_exit({exit_name:id}, __dp_exc_info())
finally:
    if {ok_name:id}:
        __dp_contextmanager_exit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder.stmt,
                ctx_placeholder_expr_1 = ctx_placeholder.expr.clone(),
                enter_stmt = enter_stmt,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        } else {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp_contextmanager_get_exit({ctx_placeholder_expr_1:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    __dp_contextmanager_exit({exit_name:id}, __dp_exc_info())
if {ok_name:id}:
    __dp_contextmanager_exit({exit_name:id}, None)
{exit_name:id} = None
{ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder.stmt,
                ctx_placeholder_expr_1 = ctx_placeholder.expr.clone(),
                enter_stmt = enter_stmt,
                body = body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        };
    }

    Rewrite::Walk(body)
}

fn contains_control_transfer_stmt(stmt: &Stmt) -> bool {
    let mut probe = stmt.clone();
    let mut visitor = ControlTransferVisitor { found: false };
    visitor.visit_stmt(&mut probe);
    visitor.found
}

struct ControlTransferVisitor {
    found: bool,
}

pub(super) fn is_simple_index_target(target: &Expr) -> bool {
    match target {
        Expr::Name(_) => true,
        Expr::Tuple(tuple) => tuple.elts.iter().all(is_simple_index_target),
        Expr::List(list) => list.elts.iter().all(is_simple_index_target),
        Expr::Starred(_) => false,
        _ => false,
    }
}

impl Transformer for ControlTransferVisitor {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if self.found {
            return;
        }
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_) => {
                self.found = true;
            }
            _ => walk_stmt(self, stmt),
        }
    }
}

impl StmtRewritePass for BBSimplifyStmtPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        lower_stmt_bb(context, stmt)
    }
}

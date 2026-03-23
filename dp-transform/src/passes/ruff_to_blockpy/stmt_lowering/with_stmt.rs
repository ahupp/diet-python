use super::assign_stmt::rewrite_assignment_target;
use super::*;
use crate::passes::ast_to_ast::body::take_suite;

impl StmtLowerer for ast::StmtWith {
    fn simplify_ast(self, _context: &Context) -> Vec<Stmt> {
        desugar_structured_with_stmt_for_blockpy(self)
    }

    fn plan_head(self, _context: &Context) -> StmtSequenceHeadPlan {
        StmtSequenceHeadPlan::With(self)
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

fn maybe_placeholder(expr: Expr) -> (Vec<Stmt>, Expr, bool) {
    if is_simple(&expr) && !matches!(&expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_)) {
        return (Vec::new(), expr, false);
    }
    let tmp = fresh_name("tmp");
    let stmt = py_stmt!("{tmp:id} = {expr:expr}", tmp = tmp.as_str(), expr = expr);
    (vec![stmt], py_expr!("{tmp:id}", tmp = tmp.as_str()), true)
}

pub(super) fn desugar_structured_with_stmt_for_blockpy(with_stmt: ast::StmtWith) -> Vec<Stmt> {
    if with_stmt.items.is_empty() {
        let mut body = with_stmt.body;
        return take_suite(&mut body);
    }

    let ast::StmtWith {
        items,
        body,
        is_async,
        ..
    } = with_stmt;

    let mut body = body;
    let mut lowered_body: Vec<Stmt> = take_suite(&mut body);

    for ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } in items.into_iter().rev()
    {
        let target = optional_vars.map(|var| *var);
        let exit_name = fresh_name("with_exit");
        let ok_name = fresh_name("with_ok");
        let reraise_name = fresh_name("with_reraise");
        let (ctx_placeholder_stmt, ctx_expr, ctx_was_placeholder) = maybe_placeholder(context_expr);
        let ctx_cleanup = if ctx_was_placeholder {
            vec![py_stmt!("{ctx:expr} = None", ctx = ctx_expr.clone())]
        } else {
            Vec::new()
        };

        let enter_value = if is_async {
            py_expr!(
                "await __dp_asynccontextmanager_aenter({ctx:expr})",
                ctx = ctx_expr.clone()
            )
        } else {
            py_expr!(
                "__dp_contextmanager_enter({ctx:expr})",
                ctx = ctx_expr.clone()
            )
        };
        let enter_stmt = if let Some(target) = target.clone() {
            let mut enter_stmts = Vec::new();
            let mut next_temp = |prefix: &str| fresh_name(prefix);
            rewrite_assignment_target(target, enter_value, &mut enter_stmts, &mut next_temp);
            enter_stmts
        } else {
            vec![py_stmt!("{value:expr}", value = enter_value)]
        };

        lowered_body = if is_async {
            crate::py_stmts!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp_asynccontextmanager_get_aexit({ctx_expr:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    {reraise_name:id} = await __dp_asynccontextmanager_exit({exit_name:id}, __dp_exc_info())
    if __dp_is_not({reraise_name:id}, None):
        raise {reraise_name:id}
finally:
    if {ok_name:id}:
        await __dp_asynccontextmanager_exit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder_stmt,
                ctx_expr = ctx_expr.clone(),
                enter_stmt = enter_stmt,
                body = lowered_body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                reraise_name = reraise_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        } else {
            crate::py_stmts!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp_contextmanager_get_exit({ctx_expr:expr})
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
                ctx_placeholder_stmt = ctx_placeholder_stmt,
                ctx_expr = ctx_expr.clone(),
                enter_stmt = enter_stmt,
                body = lowered_body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        };
    }

    lowered_body
}

pub(crate) fn lower_with_stmt_sequence<F>(
    with_stmt: ast::StmtWith,
    remaining_stmts: &[Stmt],
    targets: RegionTargets,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    _cell_slots: &HashSet<String>,
    name_gen: &NameGen,
    _needs_finally_return_flow: bool,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Stmt], RegionTargets, &mut Vec<BlockPyBlock>) -> String,
{
    if with_stmt.items.is_empty() {
        let jump_label = if linear.is_empty() {
            None
        } else {
            Some(name_gen.next_block_name())
        };
        return lower_expanded_stmt_sequence(
            {
                let mut body = with_stmt.body;
                take_suite(&mut body)
            },
            remaining_stmts,
            targets,
            linear,
            blocks,
            jump_label,
            lower_sequence,
        );
    }

    let jump_label = if linear.is_empty() {
        None
    } else {
        Some(name_gen.next_block_name())
    };
    lower_expanded_stmt_sequence(
        desugar_structured_with_stmt_for_blockpy(with_stmt),
        remaining_stmts,
        targets,
        linear,
        blocks,
        jump_label,
        lower_sequence,
    )
}

#[cfg(test)]
mod tests {
    use super::super::{simplify_stmt_ast_once_for_blockpy, BlockPyStmtFragmentBuilder};
    use super::*;
    use crate::passes::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_with_simplify_ast_desugars_before_blockpy_lowering() {
        let stmt = py_stmt!("with cm:\n    body()");
        let Stmt::With(with_stmt) = stmt else {
            panic!("expected with stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::With(with_stmt));

        assert!(!matches!(simplified.as_slice(), [Stmt::With(_)]));
    }

    #[test]
    #[should_panic(expected = "StmtTry should have already been reduced before BlockPy lowering")]
    fn stmt_with_to_blockpy_simplifies_before_hitting_sequence_only_try_lowering() {
        let stmt = py_stmt!("with cm:\n    body()");
        let Stmt::With(with_stmt) = stmt else {
            panic!("expected with stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        let _ = with_stmt.to_blockpy(&context, &mut out, None, &mut next_label_id);
    }
}

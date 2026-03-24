use super::*;
use crate::passes::ast_to_ast::ast_rewrite::Rewrite;
use crate::passes::ast_to_ast::expr_utils::make_binop;
use ruff_python_ast::Operator;

pub(crate) fn should_rewrite_assignment_targets(targets: &[Expr]) -> bool {
    if targets.len() > 1 {
        return true;
    }

    let Some(first) = targets.first() else {
        return false;
    };

    !matches!(first, Expr::Name(_))
}

fn rewrite_stmt_target(context: &Context, target: Expr, rhs: Expr, out: &mut Vec<Stmt>) {
    match target {
        Expr::Tuple(tuple) => {
            rewrite_unpack_target_stmt(context, tuple.elts, rhs, out, UnpackTargetKind::Tuple);
        }
        Expr::List(list) => {
            rewrite_unpack_target_stmt(context, list.elts, rhs, out, UnpackTargetKind::List);
        }
        Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
            let object_expr = with_target_object_expr(*value);
            out.push(py_stmt!(
                "__dp_setitem({value:expr}, {slice:expr}, {rhs:expr})",
                value = object_expr,
                slice = slice,
                rhs = rhs,
            ));
        }
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            let object_expr = with_target_object_expr(*value);
            out.push(py_stmt!(
                "__dp_setattr({value:expr}, {name:literal}, {rhs:expr})",
                value = object_expr,
                name = attr.as_str(),
                rhs = rhs
            ));
        }
        Expr::Name(ast::ExprName { id, .. }) => {
            out.push(py_stmt!(
                "{name:id} = {rhs:expr}",
                name = id.as_str(),
                rhs = rhs
            ));
        }
        other => {
            panic!("unsupported assignment target: {other:?}");
        }
    }
}

enum UnpackTargetKind {
    Tuple,
    List,
}

fn rewrite_unpack_target_stmt(
    context: &Context,
    elts: Vec<Expr>,
    value: Expr,
    out: &mut Vec<Stmt>,
    kind: UnpackTargetKind,
) {
    let tmp_expr = value;
    let mut starred_seen = false;
    for elt in &elts {
        if let Expr::Starred(_) = elt {
            if starred_seen {
                panic!("unsupported starred assignment target");
            }
            starred_seen = true;
        }
    }

    let unpacked_name = context.fresh("tmp");
    let unpacked_tmp = py_expr!("{tmp:id}", tmp = unpacked_name.as_str());

    let mut body_stmts = Vec::new();
    let use_indexable_synthetic_tmp = !starred_seen
        && matches!(
            &tmp_expr,
            Expr::Name(ast::ExprName { id, .. }) if id.as_str().starts_with("_dp_tmp_")
        );

    if starred_seen || !use_indexable_synthetic_tmp {
        let mut spec_elts = Vec::new();
        for elt in &elts {
            if matches!(elt, Expr::Starred(_)) {
                spec_elts.push(py_expr!("False"));
            } else {
                spec_elts.push(py_expr!("True"));
            }
        }
        let spec_expr = make_tuple(spec_elts);
        body_stmts.push(py_stmt!(
            "{tmp:id} = __dp_unpack({value:expr}, {spec:expr})",
            tmp = unpacked_name.as_str(),
            value = tmp_expr.clone(),
            spec = spec_expr,
        ));
    } else {
        body_stmts.push(py_stmt!(
            "{tmp:id} = {value:expr}",
            tmp = unpacked_name.as_str(),
            value = tmp_expr.clone(),
        ));
    }

    for (i, elt) in elts.into_iter().enumerate() {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                let star_value = py_expr!(
                    "__dp_getitem({tmp:expr}, {idx:literal})",
                    tmp = unpacked_tmp.clone(),
                    idx = i,
                );
                let collection_expr = match kind {
                    UnpackTargetKind::Tuple | UnpackTargetKind::List => {
                        py_expr!("__dp_list({value:expr})", value = star_value)
                    }
                };
                rewrite_stmt_target(context, *value, collection_expr, &mut body_stmts);
            }
            _ => {
                let value = py_expr!(
                    "__dp_getitem({tmp:expr}, {idx:literal})",
                    tmp = unpacked_tmp.clone(),
                    idx = i,
                );
                rewrite_stmt_target(context, elt, value, &mut body_stmts);
            }
        }
    }

    body_stmts.push(py_stmt!("del {tmp:id}", tmp = unpacked_name.as_str()));
    out.extend(body_stmts);
}

pub(crate) fn rewrite_assign_stmt(context: &Context, assign: ast::StmtAssign) -> Rewrite {
    if !should_rewrite_assignment_targets(&assign.targets) {
        return Rewrite::Unmodified(assign.into());
    }

    let ast::StmtAssign { targets, value, .. } = assign;
    let mut stmts = Vec::new();
    if targets.len() > 1 {
        let lowered = context.maybe_placeholder_lowered(*value);
        stmts.extend(lowered.stmts);
        for target in targets {
            stmts.push(py_stmt!(
                "{target:expr} = {value:expr}",
                target = target,
                value = lowered.expr.clone()
            ));
        }
    } else {
        let target = targets.into_iter().next().unwrap();
        rewrite_stmt_target(context, target, *value, &mut stmts);
    }

    Rewrite::Walk(stmts)
}

pub(crate) fn rewrite_augassign_stmt(context: &Context, aug_assign: ast::StmtAugAssign) -> Rewrite {
    let ast::StmtAugAssign {
        mut target,
        op,
        value,
        ..
    } = aug_assign;

    let func_name = match op {
        Operator::Add => "iadd",
        Operator::Sub => "isub",
        Operator::Mult => "imul",
        Operator::MatMult => "imatmul",
        Operator::Div => "itruediv",
        Operator::Mod => "imod",
        Operator::Pow => "ipow",
        Operator::LShift => "ilshift",
        Operator::RShift => "irshift",
        Operator::BitOr => "ior",
        Operator::BitXor => "ixor",
        Operator::BitAnd => "iand",
        Operator::FloorDiv => "ifloordiv",
    };

    match &mut *target {
        Expr::Name(name) => name.ctx = ast::ExprContext::Load,
        Expr::Attribute(attr) => attr.ctx = ast::ExprContext::Load,
        Expr::Subscript(sub) => sub.ctx = ast::ExprContext::Load,
        _ => {}
    }

    let call = make_binop(func_name, *target.clone(), *value);
    let mut stmts = Vec::new();
    rewrite_stmt_target(context, *target, call, &mut stmts);
    Rewrite::Walk(stmts)
}

impl StmtLowerer for ast::StmtAssign {
    fn simplify_ast(self, context: &Context) -> Vec<Stmt> {
        stmts_from_rewrite(rewrite_assign_stmt(context, self))
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
        if self.targets.len() != 1 {
            return Err(assign_delete_error(
                "multi-target assignment reached BlockPy conversion",
                &Stmt::Assign(self.clone()),
            ));
        }
        let Some(target) = self.targets[0].as_name_expr().cloned() else {
            return Err(assign_delete_error(
                "non-name assignment target reached BlockPy conversion",
                &Stmt::Assign(self.clone()),
            ));
        };
        let value = crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
            (*self.value).clone(),
            out,
            loop_ctx,
            next_label_id,
        )?;
        out.push_stmt(BlockPyStmt::Assign(BlockPyAssign { target, value }));
        Ok(())
    }
}

pub(super) fn with_target_object_expr(value: Expr) -> Expr {
    if let Expr::Name(name) = &value {
        py_expr!(
            "__dp_load_deleted_name({name:literal}, {value:expr})",
            name = name.id.as_str(),
            value = value,
        )
    } else {
        value
    }
}

pub(super) fn rewrite_assignment_target<F>(
    target: Expr,
    rhs: Expr,
    out: &mut Vec<Stmt>,
    next_temp: &mut F,
) where
    F: FnMut(&str) -> String,
{
    match target {
        Expr::Tuple(tuple) => rewrite_unpack_target(tuple.elts, rhs, out, next_temp),
        Expr::List(list) => rewrite_unpack_target(list.elts, rhs, out, next_temp),
        Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
            out.push(py_stmt!(
                "__dp_setitem({obj:expr}, {key:expr}, {rhs:expr})",
                obj = with_target_object_expr(*value),
                key = *slice,
                rhs = rhs,
            ));
        }
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            out.push(py_stmt!(
                "__dp_setattr({obj:expr}, {name:literal}, {rhs:expr})",
                obj = with_target_object_expr(*value),
                name = attr.as_str(),
                rhs = rhs,
            ));
        }
        Expr::Name(ast::ExprName { id, .. }) => {
            out.push(py_stmt!(
                "{name:id} = {rhs:expr}",
                name = id.as_str(),
                rhs = rhs
            ));
        }
        other => {
            panic!("unsupported assignment target in Ruff AST -> BlockPy lowering: {other:?}");
        }
    }
}

fn rewrite_unpack_target<F>(elts: Vec<Expr>, value: Expr, out: &mut Vec<Stmt>, next_temp: &mut F)
where
    F: FnMut(&str) -> String,
{
    let unpacked_name = next_temp("tmp");
    let unpacked_tmp = py_expr!("{tmp:id}", tmp = unpacked_name.as_str());

    let mut spec_elts = Vec::new();
    let mut starred_seen = false;
    for elt in &elts {
        match elt {
            Expr::Starred(_) => {
                if starred_seen {
                    panic!("unsupported starred with-target assignment");
                }
                starred_seen = true;
                spec_elts.push(py_expr!("False"));
            }
            _ => spec_elts.push(py_expr!("True")),
        }
    }

    out.push(py_stmt!(
        "{tmp:id} = __dp_unpack({value:expr}, {spec:expr})",
        tmp = unpacked_name.as_str(),
        value = value,
        spec = make_tuple(spec_elts),
    ));

    let starred_index = elts.iter().position(|elt| matches!(elt, Expr::Starred(_)));
    for (idx, elt) in elts.into_iter().enumerate() {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) if Some(idx) == starred_index => {
                rewrite_assignment_target(
                    *value,
                    py_expr!(
                        "__dp_list(__dp_getitem({tmp:expr}, {idx:literal}))",
                        tmp = unpacked_tmp.clone(),
                        idx = idx as i64,
                    ),
                    out,
                    next_temp,
                );
            }
            other => {
                rewrite_assignment_target(
                    other,
                    py_expr!(
                        "__dp_getitem({tmp:expr}, {idx:literal})",
                        tmp = unpacked_tmp.clone(),
                        idx = idx as i64,
                    ),
                    out,
                    next_temp,
                );
            }
        }
    }

    out.push(py_stmt!("del {tmp:id}", tmp = unpacked_name.as_str()));
}

pub(crate) fn build_for_target_assign_body<F>(
    target: &Expr,
    tmp_expr: Expr,
    tmp_name: &str,
    next_temp: &mut F,
) -> Vec<Stmt>
where
    F: FnMut(&str) -> String,
{
    let mut out = Vec::new();
    rewrite_assignment_target(target.clone(), tmp_expr, &mut out, next_temp);
    out.push(py_stmt!("{tmp:id} = None", tmp = tmp_name));
    out
}

#[cfg(test)]
mod tests {
    use super::super::BlockPyStmtFragmentBuilder;
    use super::*;
    use crate::passes::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_assign_to_blockpy_emits_setup_for_if_expr_rhs() {
        let stmt = py_stmt!("result = x if cond else y");
        let Stmt::Assign(assign_stmt) = stmt else {
            panic!("expected assign stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
        let mut next_label_id = 0usize;

        assign_stmt
            .to_blockpy(&context, &mut out, None, &mut next_label_id)
            .expect("assign lowering should succeed");

        let fragment = out.finish();
        assert!(fragment.body.len() >= 2, "{fragment:?}");
        assert!(matches!(fragment.body.last(), Some(BlockPyStmt::Assign(_))));
    }
}

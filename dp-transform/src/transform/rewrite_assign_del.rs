use super::driver::{ExprRewriter, Rewrite};
use crate::body_transform::Transformer;
use crate::template::{is_simple, make_binop, make_tuple};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Operator, Stmt};


pub(crate) fn should_rewrite_targets(rewriter: &ExprRewriter, targets: &[Expr]) -> bool {
    if targets.len() > 1 {
        return true;
    }

    let Some(first) = targets.first() else {
        return false;
    };

    match first {
        Expr::Name(_) => false,
        Expr::Attribute(_) => rewriter.context().options.lower_attributes,
        Expr::Tuple(_) | Expr::List(_) | Expr::Subscript(_) => true,
        _ => true,
    }
}

pub(crate) fn rewrite_target(
    rewriter: &mut ExprRewriter,
    target: Expr,
    rhs: Expr,
    out: &mut Vec<Stmt>,
) {

    match target {
        Expr::Tuple(tuple) => {
            rewrite_unpack_target(rewriter, tuple.elts, rhs, out, UnpackTargetKind::Tuple);
        }
        Expr::List(list) => {
            rewrite_unpack_target(rewriter, list.elts, rhs, out, UnpackTargetKind::List);
        }
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            if rewriter.context().options.lower_attributes {
                let stmt = py_stmt!(
                    "__dp__.setattr({value:expr}, {name:literal}, {rhs:expr})",
                    value = value,
                    name = attr.as_str(),
                    rhs = rhs,
                );
                out.extend(stmt);
            } else {
                out.extend(py_stmt!(
                    "{value:expr}.{attr:id} = {rhs:expr}",
                    value = value,
                    attr = attr.as_str(),
                    rhs = rhs
                ));
            }
        }
        Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
            let stmt = py_stmt!(
                "__dp__.setitem({value:expr}, {slice:expr}, {rhs:expr})",
                value = value,
                slice = slice,
                rhs = rhs,
            );
            out.extend(stmt);
        }
        Expr::Name(_) => {
            let stmt = py_stmt!("{target:expr} = {rhs:expr}", target = target, rhs = rhs,);
            out.extend(stmt);
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

fn temp_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(ast::ExprName { id, .. }) if id.as_str().starts_with("_dp_tmp_") => {
            Some(id.as_str())
        }
        _ => None,
    }
}

fn maybe_placeholder_in(
    rewriter: &mut ExprRewriter,
    mut expr: Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    fn is_temp_skippable(expr: &Expr) -> bool {
        is_simple(expr) && !matches!(expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_))
    }

    if is_temp_skippable(&expr) {
        return expr;
    }

    out.extend(rewriter.expand_here(&mut expr));

    if is_temp_skippable(&expr) {
        return expr;
    }

    let tmp = rewriter.context().fresh("tmp");
    out.extend(py_stmt!(
        "{tmp:id} = {value:expr}",
        tmp = tmp.as_str(),
        value = expr
    ));
    py_expr!("{tmp:id}", tmp = tmp.as_str())
}

fn rewrite_unpack_target(
    rewriter: &mut ExprRewriter,
    elts: Vec<Expr>,
    value: Expr,
    out: &mut Vec<Stmt>,
    kind: UnpackTargetKind,
) {
    let value_is_name = matches!(&value, Expr::Name(_));
    let tmp_expr = maybe_placeholder_in(rewriter, value, out);

    let mut spec_elts = Vec::new();
    let mut starred_seen = false;
    for elt in &elts {
        match elt {
            Expr::Starred(_) => {
                if starred_seen {
                    panic!("unsupported starred assignment target");
                }
                spec_elts.push(py_expr!("False"));
                starred_seen = true;
            }
            _ => spec_elts.push(py_expr!("True")),
        }
    }

    let spec_expr = make_tuple(spec_elts);
    let unpacked_name = rewriter.context().fresh("tmp");
    let unpacked_tmp = py_expr!("{tmp:id}", tmp = unpacked_name.as_str());
    let unpack_stmt = py_stmt!(
        "{tmp:id} = __dp__.unpack({value:expr}, {spec:expr})",
        tmp = unpacked_name.as_str(),
        value = tmp_expr.clone(),
        spec = spec_expr,
    );

    let mut body_stmts = Vec::new();
    body_stmts.extend(unpack_stmt);

    for (i, elt) in elts.into_iter().enumerate() {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                let star_value = py_expr!(
                    "__dp__.getitem({tmp:expr}, {idx:literal})",
                    tmp = unpacked_tmp.clone(),
                    idx = i,
                );
                let collection_expr = match kind {
                    UnpackTargetKind::Tuple => {
                        py_expr!("__dp__.list({value:expr})", value = star_value)
                    }
                    UnpackTargetKind::List => {
                        py_expr!("__dp__.list({value:expr})", value = star_value)
                    }
                };
                rewrite_target(rewriter, *value, collection_expr, &mut body_stmts);
            }
            _ => {
                let value = py_expr!(
                    "__dp__.getitem({tmp:expr}, {idx:literal})",
                    tmp = unpacked_tmp.clone(),
                    idx = i,
                );
                rewrite_target(rewriter, elt, value, &mut body_stmts);
            }
        }
    }

    let mut cleanup_stmts = Vec::new();
    cleanup_stmts.extend(py_stmt!("{name:id} = None", name = unpacked_name.as_str()));
    if !value_is_name {
        if let Some(name) = temp_name(&tmp_expr) {
            cleanup_stmts.extend(py_stmt!("{name:id} = None", name = name));
        }
    }

    if cleanup_stmts.is_empty() {
        out.extend(body_stmts);
    } else {
        let try_stmt = py_stmt!(
            r#"
try:
    {body:stmt}
finally:
    {cleanup:stmt}
"#,
            body = body_stmts,
            cleanup = cleanup_stmts,
        );
        out.extend(try_stmt);
    }
}

pub(crate) fn rewrite_ann_assign(
    rewriter: &mut ExprRewriter,
    mut ann_assign: ast::StmtAnnAssign,
) -> Rewrite {
    let Some(value) = ann_assign.value.take() else {
        return Rewrite::Walk(vec![Stmt::AnnAssign(ann_assign)]);
    };

    let target = (*ann_assign.target).clone();

    let mut stmts = Vec::new();
    stmts.push(Stmt::AnnAssign(ann_assign));
    rewrite_target(rewriter, target, *value, &mut stmts);
    Rewrite::Visit(stmts)
}

pub(crate) fn rewrite_assign(rewriter: &mut ExprRewriter, assign: ast::StmtAssign) -> Rewrite {
    if !should_rewrite_targets(rewriter, &assign.targets) {
        return Rewrite::Walk(vec![Stmt::Assign(assign)]);
    }

    let ast::StmtAssign { targets, value, .. } = assign;
    let mut stmts = Vec::new();
    let mut value = value.as_ref().clone();
    let multi_assign = targets.len() > 1;

    let (shared_expr, mut single_value) = if multi_assign {
        // When multiple targets share the same value we need to evaluate the expression
        // exactly once and fan the result out, so materialize a placeholder.
        (Some(rewriter.maybe_placeholder(value)), None)
    } else {
        // With a single target there's no fan-out, so rewrite the value in place and feed
        // it directly to the lowering helpers without synthesizing an intermediate
        // placeholder.
        rewriter.visit_expr(&mut value);
        (None, Some(value))
    };

    for target in targets.into_iter() {
        let expr = shared_expr.as_ref().map_or_else(
            || single_value.take().expect("value already consumed"),
            Clone::clone,
        );
        rewrite_target(rewriter, target, expr, &mut stmts);
    }

    if let Some(expr) = shared_expr {
        if let Some(name) = temp_name(&expr) {
            stmts.extend(py_stmt!("{name:id} = None", name = name));
        }
    }

    Rewrite::Visit(stmts)
}

pub(crate) fn rewrite_aug_assign(
    rewriter: &mut ExprRewriter,
    aug_assign: ast::StmtAugAssign,
) -> Rewrite {
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
    rewrite_target(rewriter, *target, call, &mut stmts);
    Rewrite::Visit(stmts)
}

pub(crate) fn rewrite_delete(rewriter: &mut ExprRewriter, delete: ast::StmtDelete) -> Rewrite {
    if !should_rewrite_targets(rewriter, &delete.targets) {
        return Rewrite::Walk(vec![Stmt::Delete(delete)]);
    }

    Rewrite::Visit(
        delete
            .targets
            .into_iter()
            .map(|target| match target {
                Expr::Subscript(sub) => py_stmt!(
                    "__dp__.delitem({obj:expr}, {key:expr})",
                    obj = sub.value,
                    key = sub.slice
                ),
                Expr::Attribute(attr) => {
                    py_stmt!(
                        "__dp__.delattr({obj:expr}, {name:literal})",
                        obj = attr.value,
                        name = attr.attr.as_str(),
                    )
                }
                other => py_stmt!("del {target:expr}", target = other),
            })
            .flatten()
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_assign_del.txt");
}

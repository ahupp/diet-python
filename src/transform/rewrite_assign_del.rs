use super::driver::{ExprRewriter, Rewrite};
use crate::body_transform::Transformer;
use crate::template::{make_binop, make_tuple};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Operator, Stmt};

pub(crate) fn should_rewrite_targets(targets: &[Expr]) -> bool {
    targets.len() > 1 || !matches!(targets.first(), Some(Expr::Name(_)))
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
            let attr = attr.clone();
            let stmt = py_stmt!(
                "__dp__.setattr({value:expr}, {name:literal}, {rhs:expr})",
                value = value,
                name = attr.as_str(),
                rhs = rhs,
            );
            out.extend(stmt);
        }
        Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
            let slice = slice.clone();
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
        _ => {
            panic!("unsupported assignment target");
        }
    }
}

enum UnpackTargetKind {
    Tuple,
    List,
}

fn rewrite_unpack_target(
    rewriter: &mut ExprRewriter,
    elts: Vec<Expr>,
    value: Expr,
    out: &mut Vec<Stmt>,
    kind: UnpackTargetKind,
) {
    let tmp_expr = rewriter.maybe_placeholder(value);

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
    let unpacked_expr = py_expr!(
        "__dp__.unpack({tmp:expr}, {spec:expr})",
        tmp = tmp_expr.clone(),
        spec = spec_expr,
    );
    let unpacked_tmp = rewriter.maybe_placeholder(unpacked_expr);

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
                        py_expr!("__dp__.tuple({value:expr})", value = star_value)
                    }
                    UnpackTargetKind::List => {
                        py_expr!("__dp__.list({value:expr})", value = star_value)
                    }
                };
                rewrite_target(rewriter, *value, collection_expr, out);
            }
            _ => {
                let value = py_expr!(
                    "__dp__.getitem({tmp:expr}, {idx:literal})",
                    tmp = unpacked_tmp.clone(),
                    idx = i,
                );
                rewrite_target(rewriter, elt, value, out);
            }
        }
    }
}

pub(crate) fn rewrite_ann_assign(
    rewriter: &mut ExprRewriter,
    ann_assign: ast::StmtAnnAssign,
) -> Rewrite {
    let ast::StmtAnnAssign {
        target,
        value: Some(value),
        ..
    } = ann_assign
    else {
        return Rewrite::Walk(vec![Stmt::AnnAssign(ann_assign)]);
    };

    let mut stmts = Vec::new();
    rewrite_target(rewriter, *target, *value, &mut stmts);
    Rewrite::Visit(stmts)
}

pub(crate) fn rewrite_assign(rewriter: &mut ExprRewriter, assign: ast::StmtAssign) -> Rewrite {
    if !should_rewrite_targets(&assign.targets) {
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

pub(crate) fn rewrite_delete(_rewriter: &mut ExprRewriter, delete: ast::StmtDelete) -> Rewrite {
    if !should_rewrite_targets(&delete.targets) {
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
                Expr::Attribute(attr) => py_stmt!(
                    "__dp__.delattr({obj:expr}, {name:literal})",
                    obj = attr.value,
                    name = attr.attr.as_str(),
                ),
                _ => py_stmt!("del {target:expr}", target = target),
            })
            .flatten()
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_assign_del.txt");
}

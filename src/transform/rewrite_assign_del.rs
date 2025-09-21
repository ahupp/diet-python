use super::expr::ExprRewriter;
use crate::body_transform::walk_stmt;
use crate::template::make_binop;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Operator, Stmt};

pub(crate) fn should_rewrite_targets(targets: &[Expr]) -> bool {
    targets.len() > 1 || !matches!(targets.first(), Some(Expr::Name(_)))
}

pub(crate) fn rewrite_target(
    rewriter: &mut ExprRewriter,
    target: Expr,
    value: Expr,
    out: &mut Vec<Stmt>,
) {
    match target {
        Expr::Tuple(tuple) => {
            rewrite_unpack_target(rewriter, tuple.elts, value, out, UnpackTargetKind::Tuple);
        }
        Expr::List(list) => {
            rewrite_unpack_target(rewriter, list.elts, value, out, UnpackTargetKind::List);
        }
        Expr::Attribute(attr) => {
            let obj = (*attr.value).clone();
            let stmt = py_stmt!(
                "\n__dp__.setattr({obj:expr}, {name:literal}, {value:expr})",
                obj = obj,
                name = attr.attr.as_str(),
                value = value,
            );
            out.push(stmt);
        }
        Expr::Subscript(sub) => {
            let obj = (*sub.value).clone();
            let key = (*sub.slice).clone();
            let stmt = py_stmt!(
                "\n__dp__.setitem({obj:expr}, {key:expr}, {value:expr})",
                obj = obj,
                key = key,
                value = value,
            );
            out.push(stmt);
        }
        Expr::Name(_) => {
            let stmt = py_stmt!(
                "{target:expr} = {value:expr}",
                target = target,
                value = value,
            );
            out.push(stmt);
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

    let elts_len = elts.len();
    let mut starred_index: Option<usize> = None;
    for (i, elt) in elts.iter().enumerate() {
        if matches!(elt, Expr::Starred(_)) {
            if starred_index.is_some() {
                panic!("unsupported starred assignment target");
            }
            starred_index = Some(i);
        }
    }

    let prefix_len = starred_index.unwrap_or(elts_len);
    let suffix_len = starred_index.map_or(0, |idx| elts_len - idx - 1);

    for (i, elt) in elts.into_iter().enumerate() {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                let stop_expr = if suffix_len == 0 {
                    py_expr!("None")
                } else {
                    let stop = -(suffix_len as isize);
                    py_expr!("{stop:literal}", stop = stop)
                };

                let slice_expr = py_expr!(
                    "__dp__.getitem({tmp:expr}, slice({start:literal}, {stop:expr}, None))",
                    tmp = tmp_expr.clone(),
                    start = prefix_len,
                    stop = stop_expr,
                );
                let collection_expr = match kind {
                    UnpackTargetKind::Tuple => {
                        py_expr!("__dp__.tuple({slice:expr})", slice = slice_expr)
                    }
                    UnpackTargetKind::List => {
                        py_expr!("__dp__.list({slice:expr})", slice = slice_expr)
                    }
                };
                rewrite_target(rewriter, *value, collection_expr, out);
            }
            _ => {
                let idx = match starred_index {
                    Some(star_idx) if i > star_idx => (i as isize) - (elts_len as isize),
                    _ => i as isize,
                };
                let value = py_expr!(
                    "__dp__.getitem({tmp:expr}, {idx:literal})",
                    tmp = tmp_expr.clone(),
                    idx = idx,
                );
                rewrite_target(rewriter, elt, value, out);
            }
        }
    }
}

pub(crate) fn rewrite_ann_assign(
    rewriter: &mut ExprRewriter,
    ann_assign: ast::StmtAnnAssign,
) -> Vec<Stmt> {
    let ast::StmtAnnAssign { target, value, .. } = ann_assign;
    let value = match value {
        Some(value) => value,
        None => return vec![],
    };

    let mut stmts = Vec::new();
    rewrite_target(rewriter, *target, *value, &mut stmts);
    stmts
}

pub(crate) fn rewrite_assign(rewriter: &mut ExprRewriter, assign: ast::StmtAssign) -> Vec<Stmt> {
    let mut stmts = Vec::new();
    let value = assign.value.as_ref().clone();

    let tmp_expr = rewriter.maybe_placeholder(value.clone());
    for target in assign.targets.into_iter() {
        rewrite_target(rewriter, target, tmp_expr.clone(), &mut stmts);
    }

    stmts
}

pub(crate) fn rewrite_aug_assign(
    rewriter: &mut ExprRewriter,
    aug_assign: ast::StmtAugAssign,
) -> Vec<Stmt> {
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
    stmts
}

pub(crate) fn rewrite_delete(_rewriter: &mut ExprRewriter, delete: ast::StmtDelete) -> Vec<Stmt> {
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
        .collect()
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_assign_del.txt");
}

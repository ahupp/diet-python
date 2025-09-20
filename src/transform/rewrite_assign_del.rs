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
            let mut stmt = py_stmt!(
                "\n__dp__.setattr({obj:expr}, {name:literal}, {value:expr})",
                obj = obj,
                name = attr.attr.as_str(),
                value = value,
            );
            walk_stmt(rewriter, &mut stmt);
            out.push(stmt);
        }
        Expr::Subscript(sub) => {
            let obj = (*sub.value).clone();
            let key = (*sub.slice).clone();
            let mut stmt = py_stmt!(
                "\n__dp__.setitem({obj:expr}, {key:expr}, {value:expr})",
                obj = obj,
                key = key,
                value = value,
            );
            walk_stmt(rewriter, &mut stmt);
            out.push(stmt);
        }
        Expr::Name(_) => {
            let mut stmt = py_stmt!(
                "\n{target:expr} = {value:expr}",
                target = target,
                value = value,
            );
            walk_stmt(rewriter, &mut stmt);
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
                let slice_expr = if suffix_len == 0 {
                    py_expr!(
                        "__dp__.getitem({tmp:expr}, slice({start:literal}, None, None))",
                        tmp = tmp_expr.clone(),
                        start = prefix_len,
                    )
                } else {
                    let stop = -(suffix_len as isize);
                    py_expr!(
                        "__dp__.getitem({tmp:expr}, slice({start:literal}, {stop:literal}, None))",
                        tmp = tmp_expr.clone(),
                        start = prefix_len,
                        stop = stop,
                    )
                };
                let collection_expr = match kind {
                    UnpackTargetKind::Tuple => {
                        py_expr!("tuple({slice:expr})", slice = slice_expr)
                    }
                    UnpackTargetKind::List => {
                        py_expr!("list({slice:expr})", slice = slice_expr)
                    }
                };
                rewrite_target(rewriter, *value, collection_expr, out);
            }
            _ => {
                let value = match starred_index {
                    Some(star_idx) if i > star_idx => {
                        let idx = (i as isize) - (elts_len as isize);
                        py_expr!(
                            "__dp__.getitem({tmp:expr}, {idx:literal})",
                            tmp = tmp_expr.clone(),
                            idx = idx,
                        )
                    }
                    _ => py_expr!(
                        "__dp__.getitem({tmp:expr}, {idx:literal})",
                        tmp = tmp_expr.clone(),
                        idx = i,
                    ),
                };
                rewrite_target(rewriter, elt, value, out);
            }
        }
    }
}

pub(crate) fn rewrite_ann_assign(
    rewriter: &mut ExprRewriter,
    ann_assign: &ast::StmtAnnAssign,
) -> Option<Vec<Stmt>> {
    let value = ann_assign.value.as_ref()?;
    let mut stmts = Vec::new();
    rewrite_target(
        rewriter,
        ann_assign.target.as_ref().clone(),
        value.as_ref().clone(),
        &mut stmts,
    );
    Some(stmts)
}

pub(crate) fn rewrite_assign(
    rewriter: &mut ExprRewriter,
    assign: &ast::StmtAssign,
) -> Option<Vec<Stmt>> {
    if !should_rewrite_targets(&assign.targets) {
        return None;
    }

    let mut stmts = Vec::new();
    let value = assign.value.as_ref().clone();

    if assign.targets.len() > 1 {
        let tmp_expr = rewriter.maybe_placeholder(value.clone());
        for target in &assign.targets {
            rewrite_target(rewriter, target.clone(), tmp_expr.clone(), &mut stmts);
        }
    } else if let Some(target) = assign.targets.first() {
        rewrite_target(rewriter, target.clone(), value, &mut stmts);
    }

    Some(stmts)
}

pub(crate) fn rewrite_aug_assign(
    rewriter: &mut ExprRewriter,
    aug_assign: &ast::StmtAugAssign,
) -> Vec<Stmt> {
    let target = aug_assign.target.as_ref().clone();
    let value = aug_assign.value.as_ref().clone();

    let func_name = match aug_assign.op {
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

    let mut target_expr = target.clone();
    match &mut target_expr {
        Expr::Name(name) => name.ctx = ast::ExprContext::Load,
        Expr::Attribute(attr) => attr.ctx = ast::ExprContext::Load,
        Expr::Subscript(sub) => sub.ctx = ast::ExprContext::Load,
        _ => {}
    }

    let call = make_binop(func_name, target_expr, value);
    let mut stmts = Vec::new();
    rewrite_target(rewriter, target, call, &mut stmts);
    stmts
}

pub(crate) fn rewrite_delete(
    _rewriter: &mut ExprRewriter,
    delete: &ast::StmtDelete,
) -> Option<Vec<Stmt>> {
    if !should_rewrite_targets(&delete.targets) {
        return None;
    }

    let mut stmts = Vec::with_capacity(delete.targets.len());
    for target in &delete.targets {
        let new_stmt = match target {
            Expr::Subscript(sub) => py_stmt!(
                "__dp__.delitem({obj:expr}, {key:expr})",
                obj = (*sub.value).clone(),
                key = (*sub.slice).clone(),
            ),
            Expr::Attribute(attr) => py_stmt!(
                "__dp__.delattr({obj:expr}, {name:literal})",
                obj = (*attr.value).clone(),
                name = attr.attr.as_str(),
            ),
            _ => py_stmt!("del {target:expr}", target = target.clone()),
        };

        stmts.push(new_stmt);
    }

    Some(stmts)
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_assign_del.txt");
}

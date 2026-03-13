use crate::basic_block::ast_to_ast::ast_rewrite::LoweredExpr;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_expr;
use crate::basic_block::ast_to_ast::rewrite_expr::{make_binop, make_unaryop};
use crate::basic_block::block_py::BlockPyStmtFragmentBuilder;
use crate::basic_block::ruff_to_blockpy::LoopContext;
use crate::basic_block::stmt_utils::flatten_stmt_boxes;
use crate::template::into_body;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, CmpOp, Expr, Stmt};

pub(crate) trait BlockPySetupExprLowerer {
    fn simplify_expr_ast(&self, context: &Context, expr: Expr) -> LoweredExpr;

    fn lower_expr_ast_into<E>(
        &self,
        context: &Context,
        expr: Expr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<Expr, String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        match expr {
            Expr::BoolOp(bool_op) => self.lower_setup_expr(
                context,
                expr_boolop_to_stmts(context, bool_op),
                out,
                loop_ctx,
                next_label_id,
            ),
            Expr::Compare(compare) => self.lower_setup_expr(
                context,
                expr_compare_to_stmts(context, compare),
                out,
                loop_ctx,
                next_label_id,
            ),
            Expr::Attribute(ast::ExprAttribute {
                value,
                attr,
                ctx,
                range,
                node_index,
            }) if matches!(ctx, ast::ExprContext::Load) => {
                Ok(Expr::Attribute(ast::ExprAttribute {
                    value: Box::new(self.lower_expr_ast_into(
                        context,
                        *value,
                        out,
                        loop_ctx,
                        next_label_id,
                    )?),
                    attr,
                    ctx,
                    range,
                    node_index,
                }))
            }
            Expr::Subscript(ast::ExprSubscript {
                value,
                slice,
                ctx,
                range,
                node_index,
            }) if matches!(ctx, ast::ExprContext::Load) => {
                Ok(Expr::Subscript(ast::ExprSubscript {
                    value: Box::new(self.lower_expr_ast_into(
                        context,
                        *value,
                        out,
                        loop_ctx,
                        next_label_id,
                    )?),
                    slice: Box::new(self.lower_expr_ast_into(
                        context,
                        *slice,
                        out,
                        loop_ctx,
                        next_label_id,
                    )?),
                    ctx,
                    range,
                    node_index,
                }))
            }
            Expr::Call(ast::ExprCall {
                func,
                arguments,
                range,
                node_index,
            }) => {
                let ast::Arguments {
                    args,
                    keywords,
                    range: args_range,
                    node_index: args_node_index,
                } = arguments;
                let func =
                    self.lower_expr_ast_into(context, *func, out, loop_ctx, next_label_id)?;
                let args = args
                    .into_vec()
                    .into_iter()
                    .map(|arg| match arg {
                        Expr::Starred(ast::ExprStarred {
                            value,
                            ctx,
                            range,
                            node_index,
                        }) => Ok(Expr::Starred(ast::ExprStarred {
                            value: Box::new(self.lower_expr_ast_into(
                                context,
                                *value,
                                out,
                                loop_ctx,
                                next_label_id,
                            )?),
                            ctx,
                            range,
                            node_index,
                        })),
                        other => {
                            self.lower_expr_ast_into(context, other, out, loop_ctx, next_label_id)
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into();
                let keywords = keywords
                    .into_vec()
                    .into_iter()
                    .map(|keyword| {
                        Ok(ast::Keyword {
                            arg: keyword.arg,
                            value: self.lower_expr_ast_into(
                                context,
                                keyword.value,
                                out,
                                loop_ctx,
                                next_label_id,
                            )?,
                            range: keyword.range,
                            node_index: keyword.node_index,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into();
                Ok(Expr::Call(ast::ExprCall {
                    func: Box::new(func),
                    arguments: ast::Arguments {
                        args,
                        keywords,
                        range: args_range,
                        node_index: args_node_index,
                    },
                    range,
                    node_index,
                }))
            }
            Expr::Tuple(ast::ExprTuple {
                elts,
                ctx,
                range,
                node_index,
                parenthesized,
            }) if matches!(ctx, ast::ExprContext::Load) => Ok(Expr::Tuple(ast::ExprTuple {
                elts: elts
                    .into_iter()
                    .map(|elt| match elt {
                        Expr::Starred(ast::ExprStarred {
                            value,
                            ctx,
                            range,
                            node_index,
                        }) => Ok(Expr::Starred(ast::ExprStarred {
                            value: Box::new(self.lower_expr_ast_into(
                                context,
                                *value,
                                out,
                                loop_ctx,
                                next_label_id,
                            )?),
                            ctx,
                            range,
                            node_index,
                        })),
                        other => {
                            self.lower_expr_ast_into(context, other, out, loop_ctx, next_label_id)
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into(),
                ctx,
                range,
                node_index,
                parenthesized,
            })),
            Expr::List(ast::ExprList {
                elts,
                ctx,
                range,
                node_index,
            }) if matches!(ctx, ast::ExprContext::Load) => Ok(Expr::List(ast::ExprList {
                elts: elts
                    .into_iter()
                    .map(|elt| match elt {
                        Expr::Starred(ast::ExprStarred {
                            value,
                            ctx,
                            range,
                            node_index,
                        }) => Ok(Expr::Starred(ast::ExprStarred {
                            value: Box::new(self.lower_expr_ast_into(
                                context,
                                *value,
                                out,
                                loop_ctx,
                                next_label_id,
                            )?),
                            ctx,
                            range,
                            node_index,
                        })),
                        other => {
                            self.lower_expr_ast_into(context, other, out, loop_ctx, next_label_id)
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into(),
                ctx,
                range,
                node_index,
            })),
            Expr::Set(ast::ExprSet {
                elts,
                range,
                node_index,
            }) => Ok(Expr::Set(ast::ExprSet {
                elts: elts
                    .into_iter()
                    .map(|elt| self.lower_expr_ast_into(context, elt, out, loop_ctx, next_label_id))
                    .collect::<Result<Vec<_>, String>>()?
                    .into(),
                range,
                node_index,
            })),
            Expr::Dict(ast::ExprDict {
                items,
                range,
                node_index,
            }) => Ok(Expr::Dict(ast::ExprDict {
                items: items
                    .into_iter()
                    .map(|item| {
                        Ok(ast::DictItem {
                            key: match item.key {
                                Some(key) => Some(self.lower_expr_ast_into(
                                    context,
                                    key,
                                    out,
                                    loop_ctx,
                                    next_label_id,
                                )?),
                                None => None,
                            },
                            value: self.lower_expr_ast_into(
                                context,
                                item.value,
                                out,
                                loop_ctx,
                                next_label_id,
                            )?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into(),
                range,
                node_index,
            })),
            Expr::UnaryOp(ast::ExprUnaryOp {
                op,
                operand,
                range,
                node_index,
            }) => Ok(Expr::UnaryOp(ast::ExprUnaryOp {
                op,
                operand: Box::new(self.lower_expr_ast_into(
                    context,
                    *operand,
                    out,
                    loop_ctx,
                    next_label_id,
                )?),
                range,
                node_index,
            })),
            Expr::BinOp(ast::ExprBinOp {
                left,
                op,
                right,
                range,
                node_index,
            }) => Ok(Expr::BinOp(ast::ExprBinOp {
                left: Box::new(self.lower_expr_ast_into(
                    context,
                    *left,
                    out,
                    loop_ctx,
                    next_label_id,
                )?),
                op,
                right: Box::new(self.lower_expr_ast_into(
                    context,
                    *right,
                    out,
                    loop_ctx,
                    next_label_id,
                )?),
                range,
                node_index,
            })),
            Expr::Slice(ast::ExprSlice {
                lower,
                upper,
                step,
                range,
                node_index,
            }) => Ok(Expr::Slice(ast::ExprSlice {
                lower: match lower {
                    Some(expr) => Some(Box::new(self.lower_expr_ast_into(
                        context,
                        *expr,
                        out,
                        loop_ctx,
                        next_label_id,
                    )?)),
                    None => None,
                },
                upper: match upper {
                    Some(expr) => Some(Box::new(self.lower_expr_ast_into(
                        context,
                        *expr,
                        out,
                        loop_ctx,
                        next_label_id,
                    )?)),
                    None => None,
                },
                step: match step {
                    Some(expr) => Some(Box::new(self.lower_expr_ast_into(
                        context,
                        *expr,
                        out,
                        loop_ctx,
                        next_label_id,
                    )?)),
                    None => None,
                },
                range,
                node_index,
            })),
            Expr::Named(_)
            | Expr::If(_)
            | Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => self.lower_setup_expr(
                context,
                self.simplify_expr_ast(context, expr),
                out,
                loop_ctx,
                next_label_id,
            ),
            other => Ok(other),
        }
    }

    fn lower_setup_expr<E>(
        &self,
        context: &Context,
        lowered: LoweredExpr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<Expr, String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        for stmt in flatten_stmt_boxes(&[Box::new(lowered.stmt)]) {
            crate::basic_block::ruff_to_blockpy::stmt_lowering::lower_nested_stmt_into_with_expr(
                context,
                stmt.as_ref(),
                out,
                loop_ctx,
                next_label_id,
            )?;
        }
        Ok(lowered.expr)
    }

    fn lower_expr_into<E>(
        &self,
        context: &Context,
        expr: Expr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<E, String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        Ok(self
            .lower_expr_ast_into(context, expr, out, loop_ctx, next_label_id)?
            .into())
    }
}

pub(crate) struct AstSetupExprLowerer;

impl BlockPySetupExprLowerer for AstSetupExprLowerer {
    fn simplify_expr_ast(&self, context: &Context, expr: Expr) -> LoweredExpr {
        match expr {
            Expr::Named(_)
            | Expr::If(_)
            | Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => rewrite_expr::lower_expr(context, expr),
            other => LoweredExpr::unmodified(other),
        }
    }
}

pub(crate) fn lower_expr_head_ast_for_blockpy(context: &Context, expr: Expr) -> LoweredExpr {
    match expr {
        Expr::BoolOp(bool_op) => expr_boolop_to_stmts(context, bool_op),
        Expr::Compare(compare) => expr_compare_to_stmts(context, compare),
        other => AstSetupExprLowerer.simplify_expr_ast(context, other),
    }
}

pub(crate) fn lower_expr_into_with_setup<E>(
    context: &Context,
    expr: Expr,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<E, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    AstSetupExprLowerer.lower_expr_into(context, expr, out, loop_ctx, next_label_id)
}

fn expr_boolop_to_stmts(context: &Context, bool_op: ast::ExprBoolOp) -> LoweredExpr {
    let target = context.fresh("target");

    LoweredExpr::modified(
        py_expr!("{target:id}", target = target.as_str()),
        expr_boolop_to_stmts_inner(target.as_str(), bool_op),
    )
}

fn expr_boolop_to_stmts_inner(target: &str, bool_op: ast::ExprBoolOp) -> Stmt {
    let ast::ExprBoolOp { op, values, .. } = bool_op;

    let mut values = values.into_iter();
    let first = values.next().expect("bool op expects at least one value");
    let stmts = match first {
        Expr::BoolOp(bool_op) => expr_boolop_to_stmts_inner(target, bool_op),
        other => py_stmt!("{target:id} = {value:expr}", target = target, value = other),
    };
    let mut stmts = vec![stmts];

    for value in values {
        let body_stmt = match value {
            Expr::BoolOp(bool_op) => expr_boolop_to_stmts_inner(target, bool_op),
            other => py_stmt!("{target:id} = {value:expr}", target = target, value = other),
        };
        let test_expr = match op {
            ast::BoolOp::And => py_expr!("{target:id}", target = target),
            ast::BoolOp::Or => py_expr!("not {target:id}", target = target),
        };
        let stmt = py_stmt!(
            r#"
if {test:expr}:
    {body:stmt}
"#,
            test = test_expr,
            body = body_stmt,
        );
        stmts.push(stmt);
    }

    into_body(stmts)
}

fn expr_compare_to_stmts(context: &Context, compare: ast::ExprCompare) -> LoweredExpr {
    let ast::ExprCompare {
        left,
        ops,
        comparators,
        ..
    } = compare;

    let ops = ops.into_vec();
    let comparators = comparators.into_vec();
    let count = ops.len();

    if count == 1 {
        return LoweredExpr::modified(
            compare_expr(ops[0], *left.clone(), comparators[0].clone()),
            Stmt::BodyStmt(ast::StmtBody {
                body: Vec::new(),
                range: Default::default(),
                node_index: Default::default(),
            }),
        );
    }

    let mut current_left = *left;
    let target = context.fresh("target");

    let mut steps: Vec<(Vec<Stmt>, Expr)> = Vec::with_capacity(count);
    let mut left_prelude: Vec<Stmt> = Vec::new();
    if count > 1 {
        let left_tmp = context.fresh("compare");
        left_prelude.push(py_stmt!(
            "{tmp:id} = {value:expr}",
            tmp = left_tmp.as_str(),
            value = current_left.clone(),
        ));
        current_left = py_expr!("{tmp:id}", tmp = left_tmp.as_str());
    }

    for (index, (op, comparator)) in ops.into_iter().zip(comparators.into_iter()).enumerate() {
        let mut comparator_expr = comparator;
        let mut prelude = Vec::new();
        if index == 0 {
            prelude.extend(left_prelude.clone());
        }
        if index < count - 1 {
            let tmp = context.fresh("compare");
            prelude.push(py_stmt!(
                "{tmp:id} = {value:expr}",
                tmp = tmp.as_str(),
                value = comparator_expr.clone(),
            ));
            comparator_expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
        }

        let comparison = compare_expr(op, current_left.clone(), comparator_expr.clone());
        steps.push((prelude, comparison));
        current_left = comparator_expr;
    }

    let mut stmt = Stmt::BodyStmt(ast::StmtBody {
        body: Vec::new(),
        range: Default::default(),
        node_index: Default::default(),
    });
    for (prelude, comparison) in steps.into_iter().rev() {
        if matches!(&stmt, Stmt::BodyStmt(ast::StmtBody { body, .. }) if body.is_empty()) {
            let mut stmts = prelude;
            stmts.push(py_stmt!(
                "{target:id} = {value:expr}",
                target = target.as_str(),
                value = comparison
            ));
            stmt = into_body(stmts);
        } else {
            stmt = py_stmt!(
                r#"
{prelude:stmt}
{target:id} = {value:expr}
if {target:id}:
    {body:stmt}
"#,
                prelude = prelude,
                target = target.as_str(),
                value = comparison,
                body = stmt,
            );
        }
    }

    LoweredExpr::modified(py_expr!("{tmp:id}", tmp = target.as_str()), stmt)
}

fn compare_expr(op: CmpOp, left: Expr, right: Expr) -> Expr {
    match op {
        CmpOp::Eq => make_binop("eq", left, right),
        CmpOp::NotEq => make_binop("ne", left, right),
        CmpOp::Lt => make_binop("lt", left, right),
        CmpOp::LtE => make_binop("le", left, right),
        CmpOp::Gt => make_binop("gt", left, right),
        CmpOp::GtE => make_binop("ge", left, right),
        CmpOp::Is => make_binop("is_", left, right),
        CmpOp::IsNot => make_binop("is_not", left, right),
        CmpOp::In => make_binop("contains", right, left),
        CmpOp::NotIn => make_unaryop("not_", make_binop("contains", right, left)),
    }
}

#[cfg(test)]
mod tests {
    use super::{lower_expr_head_ast_for_blockpy, lower_expr_into_with_setup};
    use crate::basic_block::ast_to_ast::{context::Context, Options};
    use crate::basic_block::block_py::{BlockPyExpr, BlockPyStmt, BlockPyStmtFragmentBuilder};
    use crate::py_expr;

    #[test]
    fn expr_head_simplify_rewrites_boolop_for_blockpy() {
        let context = Context::new(Options::for_test(), "");
        let lowered = lower_expr_head_ast_for_blockpy(&context, py_expr!("a and b"));
        let rendered = crate::ruff_ast_to_string(&lowered.stmt);
        assert!(rendered.contains("if _dp_target"), "{rendered}");
    }

    #[test]
    fn nested_boolop_in_call_argument_emits_setup_via_expr_lowering() {
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<BlockPyExpr>::new();
        let mut next_label_id = 0usize;

        let lowered = lower_expr_into_with_setup(
            &context,
            py_expr!("f(a and b)"),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("expr lowering should succeed");

        let fragment = out.finish();
        assert!(
            fragment
                .body
                .iter()
                .any(|stmt| matches!(stmt, BlockPyStmt::If(_))),
            "{fragment:?}"
        );
        let rendered = crate::ruff_ast_to_string(&lowered.to_expr());
        assert!(rendered.starts_with("f(_dp_target_"), "{rendered}");
    }
}

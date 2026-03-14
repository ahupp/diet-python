use super::BlockPySetupExprLowerer;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::block_py::BlockPyStmtFragmentBuilder;
use crate::basic_block::ruff_to_blockpy::expr_lowering::boolop_compare::{
    lower_boolop_into, lower_compare_into,
};
use crate::basic_block::ruff_to_blockpy::expr_lowering::if_expr::lower_if_expr_into;
use crate::basic_block::ruff_to_blockpy::LoopContext;
use ruff_python_ast::{self as ast, Expr};

pub(super) fn lower_expr_ast_recursive<L, E>(
    lowerer: &L,
    context: &Context,
    expr: Expr,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Expr, String>
where
    L: BlockPySetupExprLowerer + ?Sized,
    E: From<Expr> + std::fmt::Debug,
{
    match expr {
        Expr::BoolOp(bool_op) => {
            lower_boolop_into(lowerer, context, bool_op, out, loop_ctx, next_label_id)
        }
        Expr::Compare(compare) => {
            lower_compare_into(lowerer, context, compare, out, loop_ctx, next_label_id)
        }
        Expr::If(if_expr) => {
            lower_if_expr_into(lowerer, context, if_expr, out, loop_ctx, next_label_id)
        }
        Expr::Attribute(ast::ExprAttribute {
            value,
            attr,
            ctx,
            range,
            node_index,
        }) if matches!(ctx, ast::ExprContext::Load) => Ok(Expr::Attribute(ast::ExprAttribute {
            value: Box::new(lower_expr_ast_recursive(
                lowerer,
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
        })),
        Expr::Subscript(ast::ExprSubscript {
            value,
            slice,
            ctx,
            range,
            node_index,
        }) if matches!(ctx, ast::ExprContext::Load) => Ok(Expr::Subscript(ast::ExprSubscript {
            value: Box::new(lower_expr_ast_recursive(
                lowerer,
                context,
                *value,
                out,
                loop_ctx,
                next_label_id,
            )?),
            slice: Box::new(lower_expr_ast_recursive(
                lowerer,
                context,
                *slice,
                out,
                loop_ctx,
                next_label_id,
            )?),
            ctx,
            range,
            node_index,
        })),
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
                lower_expr_ast_recursive(lowerer, context, *func, out, loop_ctx, next_label_id)?;
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
                        value: Box::new(lower_expr_ast_recursive(
                            lowerer,
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
                    other => lower_expr_ast_recursive(
                        lowerer,
                        context,
                        other,
                        out,
                        loop_ctx,
                        next_label_id,
                    ),
                })
                .collect::<Result<Vec<_>, String>>()?
                .into();
            let keywords = keywords
                .into_vec()
                .into_iter()
                .map(|keyword| {
                    Ok(ast::Keyword {
                        arg: keyword.arg,
                        value: lower_expr_ast_recursive(
                            lowerer,
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
                        value: Box::new(lower_expr_ast_recursive(
                            lowerer,
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
                    other => lower_expr_ast_recursive(
                        lowerer,
                        context,
                        other,
                        out,
                        loop_ctx,
                        next_label_id,
                    ),
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
                        value: Box::new(lower_expr_ast_recursive(
                            lowerer,
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
                    other => lower_expr_ast_recursive(
                        lowerer,
                        context,
                        other,
                        out,
                        loop_ctx,
                        next_label_id,
                    ),
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
                .map(|elt| {
                    lower_expr_ast_recursive(lowerer, context, elt, out, loop_ctx, next_label_id)
                })
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
                            Some(key) => Some(lower_expr_ast_recursive(
                                lowerer,
                                context,
                                key,
                                out,
                                loop_ctx,
                                next_label_id,
                            )?),
                            None => None,
                        },
                        value: lower_expr_ast_recursive(
                            lowerer,
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
            operand: Box::new(lower_expr_ast_recursive(
                lowerer,
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
            left: Box::new(lower_expr_ast_recursive(
                lowerer,
                context,
                *left,
                out,
                loop_ctx,
                next_label_id,
            )?),
            op,
            right: Box::new(lower_expr_ast_recursive(
                lowerer,
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
                Some(expr) => Some(Box::new(lower_expr_ast_recursive(
                    lowerer,
                    context,
                    *expr,
                    out,
                    loop_ctx,
                    next_label_id,
                )?)),
                None => None,
            },
            upper: match upper {
                Some(expr) => Some(Box::new(lower_expr_ast_recursive(
                    lowerer,
                    context,
                    *expr,
                    out,
                    loop_ctx,
                    next_label_id,
                )?)),
                None => None,
            },
            step: match step {
                Some(expr) => Some(Box::new(lower_expr_ast_recursive(
                    lowerer,
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
        | Expr::Lambda(_)
        | Expr::Generator(_)
        | Expr::ListComp(_)
        | Expr::SetComp(_)
        | Expr::DictComp(_) => lowerer.lower_setup_expr(
            context,
            lowerer.simplify_expr_ast(context, expr),
            out,
            loop_ctx,
            next_label_id,
        ),
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use crate::basic_block::ast_to_ast::{context::Context, Options};
    use crate::basic_block::block_py::{BlockPyExpr, BlockPyStmt, BlockPyStmtFragmentBuilder};
    use crate::basic_block::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
    use crate::py_expr;

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

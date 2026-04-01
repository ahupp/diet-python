use crate::block_py::{
    core_operation_expr, core_positional_call_expr_with_meta, operation, BlockPyFunctionKind,
    BlockPyStmtFragmentBuilder, CoreBlockPyExprWithAwaitAndYield, FunctionId, Meta, WithMeta,
};
use crate::namegen::fresh_name;
use crate::passes::ruff_to_blockpy::LoopContext;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::TextRange;

mod boolop_compare;
mod if_expr;
mod named_expr;
mod recursive;

pub(crate) trait RuffToBlockPyExpr: From<Expr> + std::fmt::Debug + Clone + Sized {
    fn from_lowered_expr(expr: Expr) -> Self {
        expr.into()
    }

    fn helper_call(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        name: &'static str,
        args: Vec<Self>,
    ) -> Self;

    fn lower_augassign_value(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        op: ast::Operator,
        left: Self,
        right: Self,
    ) -> Self;

    fn load_deleted_name(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        name: String,
        value: Self,
    ) -> Self;

    fn get_attr(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        attr: String,
    ) -> Self;

    fn set_attr(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        attr: String,
        replacement: Self,
    ) -> Self;

    fn get_item(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        index: Self,
    ) -> Self;

    fn set_item(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        index: Self,
        replacement: Self,
    ) -> Self;

    fn del_item(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        index: Self,
    ) -> Self;
}

fn inplace_kind(op: ast::Operator) -> Option<operation::InplaceBinOpKind> {
    Some(match op {
        ast::Operator::Add => operation::InplaceBinOpKind::Add,
        ast::Operator::Sub => operation::InplaceBinOpKind::Sub,
        ast::Operator::Mult => operation::InplaceBinOpKind::Mul,
        ast::Operator::MatMult => operation::InplaceBinOpKind::MatMul,
        ast::Operator::Div => operation::InplaceBinOpKind::TrueDiv,
        ast::Operator::Mod => operation::InplaceBinOpKind::Mod,
        ast::Operator::LShift => operation::InplaceBinOpKind::LShift,
        ast::Operator::RShift => operation::InplaceBinOpKind::RShift,
        ast::Operator::BitOr => operation::InplaceBinOpKind::Or,
        ast::Operator::BitXor => operation::InplaceBinOpKind::Xor,
        ast::Operator::BitAnd => operation::InplaceBinOpKind::And,
        ast::Operator::FloorDiv => operation::InplaceBinOpKind::FloorDiv,
        ast::Operator::Pow => return None,
    })
}

impl RuffToBlockPyExpr for CoreBlockPyExprWithAwaitAndYield {
    fn from_lowered_expr(expr: Expr) -> Self {
        lower_direct_core_helper_expr(&expr).unwrap_or_else(|| expr.into())
    }

    fn helper_call(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        name: &'static str,
        args: Vec<Self>,
    ) -> Self {
        core_positional_call_expr_with_meta(name, node_index, range, args)
    }

    fn lower_augassign_value(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        op: ast::Operator,
        left: Self,
        right: Self,
    ) -> Self {
        let meta = Meta::new(node_index.clone(), range);
        if let Some(kind) = inplace_kind(op) {
            return core_operation_expr(
                operation::OperationDetail::from(operation::InplaceBinOp::new(
                    kind,
                    Box::new(left),
                    Box::new(right),
                ))
                .with_meta(meta),
            );
        }

        core_operation_expr(
            operation::OperationDetail::from(operation::TernaryOp::new(
                operation::TernaryOpKind::Pow,
                Box::new(left),
                Box::new(right),
                Box::new(CoreBlockPyExprWithAwaitAndYield::from(py_expr!(
                    "__dp_NONE"
                ))),
            ))
            .with_meta(meta),
        )
    }

    fn load_deleted_name(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        name: String,
        value: Self,
    ) -> Self {
        Self::helper_call(
            node_index,
            range,
            "__dp_load_deleted_name",
            vec![
                CoreBlockPyExprWithAwaitAndYield::from(py_expr!("{name:literal}", name = name)),
                value,
            ],
        )
    }

    fn get_attr(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        attr: String,
    ) -> Self {
        core_operation_expr(
            operation::GetAttr::new(Box::new(value), attr).with_meta(Meta::new(node_index, range)),
        )
    }

    fn set_attr(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        attr: String,
        replacement: Self,
    ) -> Self {
        core_operation_expr(
            operation::SetAttr::new(Box::new(value), attr, Box::new(replacement))
                .with_meta(Meta::new(node_index, range)),
        )
    }

    fn get_item(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        index: Self,
    ) -> Self {
        core_operation_expr(
            operation::GetItem::new(Box::new(value), Box::new(index))
                .with_meta(Meta::new(node_index, range)),
        )
    }

    fn set_item(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        index: Self,
        replacement: Self,
    ) -> Self {
        core_operation_expr(
            operation::SetItem::new(Box::new(value), Box::new(index), Box::new(replacement))
                .with_meta(Meta::new(node_index, range)),
        )
    }

    fn del_item(
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        value: Self,
        index: Self,
    ) -> Self {
        core_operation_expr(
            operation::DelItem::new(Box::new(value), Box::new(index))
                .with_meta(Meta::new(node_index, range)),
        )
    }
}

pub(crate) trait BlockPySetupExprLowerer {
    fn lower_expr_ast_into<E>(
        &self,
        expr: Expr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<Expr, String>
    where
        E: RuffToBlockPyExpr,
    {
        recursive::lower_expr_ast_recursive(self, expr, out, loop_ctx, next_label_id)
    }

    fn lower_expr_into<E>(
        &self,
        expr: Expr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<E, String>
    where
        E: RuffToBlockPyExpr,
    {
        Ok(E::from_lowered_expr(self.lower_expr_ast_into(
            expr,
            out,
            loop_ctx,
            next_label_id,
        )?))
    }
}

pub(crate) struct AstSetupExprLowerer;

impl BlockPySetupExprLowerer for AstSetupExprLowerer {}

pub(crate) fn lower_expr_head_ast_for_blockpy(expr: Expr) -> Expr {
    expr
}

pub(crate) fn lower_expr_into_with_setup<E>(
    expr: Expr,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<E, String>
where
    E: RuffToBlockPyExpr,
{
    AstSetupExprLowerer.lower_expr_into(expr, out, loop_ctx, next_label_id)
}

fn make_function_kind_from_literal(expr: &Expr) -> Option<BlockPyFunctionKind> {
    let Expr::StringLiteral(string) = expr else {
        return None;
    };
    Some(match string.value.to_str() {
        "function" => BlockPyFunctionKind::Function,
        "coroutine" => BlockPyFunctionKind::Coroutine,
        "generator" => BlockPyFunctionKind::Generator,
        "async_generator" => BlockPyFunctionKind::AsyncGenerator,
        _ => return None,
    })
}

fn make_function_id_from_literal(expr: &Expr) -> Option<FunctionId> {
    let Expr::NumberLiteral(number) = expr else {
        return None;
    };
    let ast::Number::Int(value) = &number.value else {
        return None;
    };
    value.to_string().parse().ok().map(FunctionId)
}

fn string_literal_value(expr: &Expr) -> Option<String> {
    let Expr::StringLiteral(string) = expr else {
        return None;
    };
    Some(string.value.to_str().to_string())
}

fn lowered_helper_call<'a>(
    expr: &'a Expr,
    expected_name: &str,
    arity: usize,
) -> Option<&'a ast::ExprCall> {
    let Expr::Call(call) = expr else {
        return None;
    };
    if !call.arguments.keywords.is_empty() || call.arguments.args.len() != arity {
        return None;
    }
    let Expr::Name(name) = call.func.as_ref() else {
        return None;
    };
    if name.id.as_str() != expected_name {
        return None;
    }
    Some(call)
}

fn lower_direct_core_helper_expr(expr: &Expr) -> Option<CoreBlockPyExprWithAwaitAndYield> {
    fn lowered(expr: Expr) -> CoreBlockPyExprWithAwaitAndYield {
        <CoreBlockPyExprWithAwaitAndYield as RuffToBlockPyExpr>::from_lowered_expr(expr)
    }

    if let Some(call) = lowered_helper_call(expr, "__dp_make_function", 5) {
        let function_id = make_function_id_from_literal(&call.arguments.args[0])?;
        let kind = make_function_kind_from_literal(&call.arguments.args[1])?;
        return Some(core_operation_expr(
            operation::OperationDetail::from(operation::MakeFunction::new(
                function_id,
                kind,
                Box::new(lowered(call.arguments.args[3].clone())),
                Box::new(lowered(call.arguments.args[4].clone())),
            ))
            .with_meta(Meta::new(call.node_index.clone(), call.range)),
        ));
    }

    if let Some(call) = lowered_helper_call(expr, "__dp_store_global", 3) {
        return Some(core_operation_expr(
            operation::OperationDetail::from(operation::StoreName::new(
                string_literal_value(&call.arguments.args[1])?,
                Box::new(lowered(call.arguments.args[2].clone())),
            ))
            .with_meta(Meta::new(call.node_index.clone(), call.range)),
        ));
    }

    if let Some(call) = lowered_helper_call(expr, "__dp_cell_ref", 1) {
        return Some(core_operation_expr(
            operation::OperationDetail::from(operation::CellRefForName::new(string_literal_value(
                &call.arguments.args[0],
            )?))
            .with_meta(Meta::new(call.node_index.clone(), call.range)),
        ));
    }

    None
}

pub(crate) fn fresh_setup_name(prefix: &str) -> String {
    fresh_name(prefix)
}

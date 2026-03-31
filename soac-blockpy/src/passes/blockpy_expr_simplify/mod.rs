use super::ast_to_ast::rewrite_expr::string::lower_string_templates_in_expr;
use super::core_eval_order::make_eval_order_explicit_in_core_block;
use crate::block_py::structured::IntoStructuredBlockPyStmt;
use crate::block_py::{
    convert_blockpy_stmt_expr, convert_blockpy_term_expr, core_call_expr_with_meta,
    core_positional_call_expr_with_meta, operation, BlockPyAssign, BlockPyBranchTable,
    BlockPyDelete, BlockPyFunction, BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmtFragment,
    BlockPyStmtFragmentBuilder, BlockPyTerm, CfgBlock, CoreBlockPyAwait, CoreBlockPyCallArg,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreBlockPyYield,
    CoreBlockPyYieldFrom, CoreBytesLiteral, CoreNumberLiteral, CoreNumberLiteralValue,
    CoreStringLiteral, HasMeta, Meta, RuffExpr, StructuredBlockPyStmt, WithMeta,
};
use crate::passes::ast_to_ast::expr_utils::{make_binop, make_tuple};
use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
use crate::passes::ruff_to_blockpy::lower_structured_blocks_to_bb_blocks;
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, RuffBlockPyPass};
use crate::py_expr;
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr};

type CoreStmtBuilder = BlockPyStmtFragmentBuilder<CoreBlockPyExprWithAwaitAndYield>;
type SemanticExpr = Expr;

struct SemanticExprBoundaryValidator;

impl Transformer for SemanticExprBoundaryValidator {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => panic!(
                "helper-scoped expr leaked past rewrite_ast_to_lowered_blockpy_module_plan: {}",
                crate::ruff_ast_to_string(&*expr)
            ),
            other => walk_expr(self, other),
        }
    }
}

fn assert_expr_simplify_boundary(expr: &SemanticExpr) {
    let mut expr = expr.clone();
    SemanticExprBoundaryValidator.visit_expr(&mut expr);
}

fn core_builtin_name(id: &str) -> CoreBlockPyExprWithAwaitAndYield {
    CoreBlockPyExprWithAwaitAndYield::Name(ast::ExprName {
        id: id.into(),
        ctx: ast::ExprContext::Load,
        range: Default::default(),
        node_index: ast::AtomicNodeIndex::default(),
    })
}

fn is_internal_core_name(id: &str) -> bool {
    id.starts_with("_dp_") || id.starts_with("__dp_") || id == "runtime"
}

pub(crate) trait PureCoreExprReducer {
    fn reduce_expr(&self, expr: &SemanticExpr) -> CoreBlockPyExprWithAwaitAndYield;
}

struct DefaultCoreExprReducer;

impl PureCoreExprReducer for DefaultCoreExprReducer {
    fn reduce_expr(&self, expr: &SemanticExpr) -> CoreBlockPyExprWithAwaitAndYield {
        let mut expr = expr.clone();
        lower_string_templates_in_expr(&mut expr);
        expr.into()
    }
}

fn reduce_core_blockpy_dict(items: Box<[ast::DictItem]>) -> CoreBlockPyExprWithAwaitAndYield {
    let mut segments: Vec<Expr> = Vec::new();
    let mut keyed_pairs = Vec::new();

    for item in items {
        match item {
            ast::DictItem {
                key: Some(key),
                value,
            } => {
                keyed_pairs.push(py_expr!(
                    "({key:expr}, {value:expr})",
                    key = key,
                    value = value,
                ));
            }
            ast::DictItem { key: None, value } => {
                if !keyed_pairs.is_empty() {
                    let tuple = make_tuple(std::mem::take(&mut keyed_pairs));
                    segments.push(py_expr!("__dp_dict({tuple:expr})", tuple = tuple));
                }
                segments.push(py_expr!("__dp_dict({mapping:expr})", mapping = value));
            }
        }
    }

    if !keyed_pairs.is_empty() {
        let tuple = make_tuple(keyed_pairs);
        segments.push(py_expr!("__dp_dict({tuple:expr})", tuple = tuple));
    }

    let expr = match segments.len() {
        0 => py_expr!("__dp_dict()"),
        _ => segments
            .into_iter()
            .reduce(|left, right| make_binop("or_", left, right))
            .expect("dict segments are non-empty"),
    };
    CoreBlockPyExprWithAwaitAndYield::from(expr)
}

fn core_operation_expr(
    operation: operation::Operation<CoreBlockPyExprWithAwaitAndYield, ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    CoreBlockPyExprWithAwaitAndYield::Op(operation)
}

fn core_operation_expr_with_meta(
    detail: impl Into<operation::OperationDetail<CoreBlockPyExprWithAwaitAndYield, ast::ExprName>>,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExprWithAwaitAndYield {
    core_operation_expr(operation::Operation::new(detail).with_meta(Meta::new(node_index, range)))
}

fn unary_op_expr_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    kind: operation::UnaryOpKind,
    operand: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    core_operation_expr_with_meta(
        operation::UnaryOp::new(kind, Box::new(operand)),
        node_index,
        range,
    )
}

fn binop_expr_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    kind: operation::BinOpKind,
    left: CoreBlockPyExprWithAwaitAndYield,
    right: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    core_operation_expr_with_meta(
        operation::BinOp::new(kind, Box::new(left), Box::new(right)),
        node_index,
        range,
    )
}

fn getattr_expr_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    value: CoreBlockPyExprWithAwaitAndYield,
    attr: String,
) -> CoreBlockPyExprWithAwaitAndYield {
    core_operation_expr_with_meta(
        operation::GetAttr::new(Box::new(value), attr),
        node_index,
        range,
    )
}

fn getitem_expr_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    value: CoreBlockPyExprWithAwaitAndYield,
    index: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    core_operation_expr_with_meta(
        operation::GetItem::new(Box::new(value), Box::new(index)),
        node_index,
        range,
    )
}

fn unary_op_expr_from_ast_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    op: ast::UnaryOp,
    operand: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    let kind = match op {
        ast::UnaryOp::Not => operation::UnaryOpKind::Not,
        ast::UnaryOp::Invert => operation::UnaryOpKind::Invert,
        ast::UnaryOp::USub => operation::UnaryOpKind::Neg,
        ast::UnaryOp::UAdd => operation::UnaryOpKind::Pos,
    };
    unary_op_expr_with_meta(node_index, range, kind, operand)
}

fn binop_expr_from_ast_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    op: ast::Operator,
    left: CoreBlockPyExprWithAwaitAndYield,
    right: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    match op {
        ast::Operator::Add => add_op_expr_with_meta(node_index, range, left, right),
        ast::Operator::Pow => core_operation_expr_with_meta(
            operation::TernaryOp::new(
                operation::TernaryOpKind::Pow,
                Box::new(left),
                Box::new(right),
                Box::new(core_builtin_name("__dp_NONE")),
            ),
            node_index,
            range,
        ),
        _ => {
            let kind = match op {
                ast::Operator::Add => unreachable!("handled above"),
                ast::Operator::Sub => operation::BinOpKind::Sub,
                ast::Operator::Mult => operation::BinOpKind::Mul,
                ast::Operator::MatMult => operation::BinOpKind::MatMul,
                ast::Operator::Div => operation::BinOpKind::TrueDiv,
                ast::Operator::Mod => operation::BinOpKind::Mod,
                ast::Operator::Pow => unreachable!("handled above"),
                ast::Operator::LShift => operation::BinOpKind::LShift,
                ast::Operator::RShift => operation::BinOpKind::RShift,
                ast::Operator::BitOr => operation::BinOpKind::Or,
                ast::Operator::BitXor => operation::BinOpKind::Xor,
                ast::Operator::BitAnd => operation::BinOpKind::And,
                ast::Operator::FloorDiv => operation::BinOpKind::FloorDiv,
            };
            binop_expr_with_meta(node_index, range, kind, left, right)
        }
    }
}

fn compare_expr_from_ast_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    op: ast::CmpOp,
    left: CoreBlockPyExprWithAwaitAndYield,
    right: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    match op {
        ast::CmpOp::Eq => {
            binop_expr_with_meta(node_index, range, operation::BinOpKind::Eq, left, right)
        }
        ast::CmpOp::NotEq => {
            binop_expr_with_meta(node_index, range, operation::BinOpKind::Ne, left, right)
        }
        ast::CmpOp::Lt => {
            binop_expr_with_meta(node_index, range, operation::BinOpKind::Lt, left, right)
        }
        ast::CmpOp::LtE => {
            binop_expr_with_meta(node_index, range, operation::BinOpKind::Le, left, right)
        }
        ast::CmpOp::Gt => {
            binop_expr_with_meta(node_index, range, operation::BinOpKind::Gt, left, right)
        }
        ast::CmpOp::GtE => {
            binop_expr_with_meta(node_index, range, operation::BinOpKind::Ge, left, right)
        }
        ast::CmpOp::Is => {
            binop_expr_with_meta(node_index, range, operation::BinOpKind::Is, left, right)
        }
        ast::CmpOp::IsNot => {
            binop_expr_with_meta(node_index, range, operation::BinOpKind::IsNot, left, right)
        }
        ast::CmpOp::In => binop_expr_with_meta(
            node_index,
            range,
            operation::BinOpKind::Contains,
            right,
            left,
        ),
        ast::CmpOp::NotIn => unary_op_expr_with_meta(
            node_index.clone(),
            range,
            operation::UnaryOpKind::Not,
            binop_expr_with_meta(
                node_index,
                range,
                operation::BinOpKind::Contains,
                right,
                left,
            ),
        ),
    }
}

fn add_op_expr_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    left: CoreBlockPyExprWithAwaitAndYield,
    right: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    binop_expr_with_meta(node_index, range, operation::BinOpKind::Add, left, right)
}

fn add_op_expr(
    left: CoreBlockPyExprWithAwaitAndYield,
    right: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    add_op_expr_with_meta(
        ast::AtomicNodeIndex::default(),
        Default::default(),
        left,
        right,
    )
}

fn lower_core_call_args(
    args: Vec<Expr>,
) -> Vec<CoreBlockPyCallArg<CoreBlockPyExprWithAwaitAndYield>> {
    args.into_iter()
        .map(|arg| match arg {
            Expr::Starred(starred) => {
                CoreBlockPyCallArg::Starred(CoreBlockPyExprWithAwaitAndYield::from(*starred.value))
            }
            other => CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::from(other)),
        })
        .collect()
}

fn lower_core_call_keywords(
    keywords: Vec<ast::Keyword>,
) -> Vec<CoreBlockPyKeywordArg<CoreBlockPyExprWithAwaitAndYield>> {
    keywords
        .into_iter()
        .map(|keyword| match keyword.arg {
            Some(arg) => CoreBlockPyKeywordArg::Named {
                arg,
                value: CoreBlockPyExprWithAwaitAndYield::from(keyword.value),
            },
            None => CoreBlockPyKeywordArg::Starred(CoreBlockPyExprWithAwaitAndYield::from(
                keyword.value,
            )),
        })
        .collect()
}

fn make_function_kind_from_literal(expr: &Expr) -> Option<crate::block_py::BlockPyFunctionKind> {
    let Expr::StringLiteral(node) = expr else {
        return None;
    };
    match node.value.to_str() {
        "function" => Some(crate::block_py::BlockPyFunctionKind::Function),
        "coroutine" => Some(crate::block_py::BlockPyFunctionKind::Coroutine),
        "generator" => Some(crate::block_py::BlockPyFunctionKind::Generator),
        "async_generator" => Some(crate::block_py::BlockPyFunctionKind::AsyncGenerator),
        _ => None,
    }
}

fn make_function_id_from_literal(expr: &Expr) -> Option<crate::block_py::FunctionId> {
    let Expr::NumberLiteral(node) = expr else {
        return None;
    };
    let ast::Number::Int(value) = &node.value else {
        return None;
    };
    value
        .to_string()
        .parse()
        .ok()
        .map(crate::block_py::FunctionId)
}

fn string_arg_from_core_expr(expr: CoreBlockPyExprWithAwaitAndYield) -> Option<String> {
    let CoreBlockPyExprWithAwaitAndYield::Literal(
        crate::block_py::CoreBlockPyLiteral::StringLiteral(literal),
    ) = expr
    else {
        return None;
    };
    Some(literal.value)
}

fn bytes_arg_from_core_expr(expr: CoreBlockPyExprWithAwaitAndYield) -> Option<Vec<u8>> {
    let CoreBlockPyExprWithAwaitAndYield::Literal(
        crate::block_py::CoreBlockPyLiteral::BytesLiteral(literal),
    ) = expr
    else {
        return None;
    };
    Some(literal.value)
}

fn name_arg_from_core_expr(expr: CoreBlockPyExprWithAwaitAndYield) -> Option<ast::ExprName> {
    let CoreBlockPyExprWithAwaitAndYield::Name(name) = expr else {
        return None;
    };
    Some(name)
}

fn operator_family_operation_from_helper_call(
    name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyExprWithAwaitAndYield>,
) -> Option<operation::Operation<CoreBlockPyExprWithAwaitAndYield, ast::ExprName>> {
    let mut args = args.into_iter();
    let meta = Meta::new(node_index, range);
    let operation = if let Some(kind) = operation::BinOpKind::from_helper_name(name) {
        let left = args.next()?;
        let right = args.next()?;
        if args.next().is_some() {
            return None;
        }
        operation::Operation::new(operation::BinOp::new(kind, Box::new(left), Box::new(right)))
            .with_meta(meta)
    } else if let Some(kind) = operation::UnaryOpKind::from_helper_name(name) {
        let operand = args.next()?;
        if args.next().is_some() {
            return None;
        }
        operation::Operation::new(operation::UnaryOp::new(kind, Box::new(operand))).with_meta(meta)
    } else if let Some(kind) = operation::InplaceBinOpKind::from_helper_name(name) {
        let left = args.next()?;
        let right = args.next()?;
        if args.next().is_some() {
            return None;
        }
        operation::Operation::new(operation::InplaceBinOp::new(
            kind,
            Box::new(left),
            Box::new(right),
        ))
        .with_meta(meta)
    } else if let Some(kind) = operation::TernaryOpKind::from_helper_name(name) {
        let base = args.next()?;
        let exponent = args.next()?;
        let modulus = args.next()?;
        if args.next().is_some() {
            return None;
        }
        operation::Operation::new(operation::TernaryOp::new(
            kind,
            Box::new(base),
            Box::new(exponent),
            Box::new(modulus),
        ))
        .with_meta(meta)
    } else {
        return None;
    };
    if args.next().is_some() {
        return None;
    }
    Some(operation)
}

fn non_operator_operation_from_helper_call(
    name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyExprWithAwaitAndYield>,
) -> Option<operation::Operation<CoreBlockPyExprWithAwaitAndYield, ast::ExprName>> {
    let mut args = args.into_iter();
    let meta = Meta::new(node_index, range);
    let operation = match name {
        "__dp_getattr" => operation::Operation::new(operation::GetAttr::new(
            Box::new(args.next()?),
            string_arg_from_core_expr(args.next()?)?,
        ))
        .with_meta(meta),
        "__dp_setattr" => operation::Operation::new(operation::SetAttr::new(
            Box::new(args.next()?),
            string_arg_from_core_expr(args.next()?)?,
            Box::new(args.next()?),
        ))
        .with_meta(meta),
        "__dp_getitem" => operation::Operation::new(operation::GetItem::new(
            Box::new(args.next()?),
            Box::new(args.next()?),
        ))
        .with_meta(meta),
        "__dp_setitem" => operation::Operation::new(operation::SetItem::new(
            Box::new(args.next()?),
            Box::new(args.next()?),
            Box::new(args.next()?),
        ))
        .with_meta(meta),
        "__dp_delitem" => operation::Operation::new(operation::DelItem::new(
            Box::new(args.next()?),
            Box::new(args.next()?),
        ))
        .with_meta(meta),
        "__dp_load_global" => operation::Operation::new(operation::LoadGlobal::new(
            Box::new(args.next()?),
            string_arg_from_core_expr(args.next()?)?,
        ))
        .with_meta(meta),
        "__dp_store_global" => operation::Operation::new(operation::StoreGlobal::new(
            Box::new(args.next()?),
            string_arg_from_core_expr(args.next()?)?,
            Box::new(args.next()?),
        ))
        .with_meta(meta),
        "__dp_make_cell" => {
            operation::Operation::new(operation::MakeCell::new(Box::new(args.next()?)))
                .with_meta(meta)
        }
        "__dp_decode_literal_bytes" => operation::Operation::new(operation::MakeString::new(
            bytes_arg_from_core_expr(args.next()?)?,
        ))
        .with_meta(meta),
        "__dp_cell_ref" => operation::Operation::new(operation::CellRef::new(
            operation::CellRefTarget::LogicalName(string_arg_from_core_expr(args.next()?)?),
        ))
        .with_meta(meta),
        "__dp_del_quietly" => operation::Operation::new(operation::DelQuietly::new(
            Box::new(args.next()?),
            string_arg_from_core_expr(args.next()?)?,
        ))
        .with_meta(meta),
        _ => return None,
    };
    if args.next().is_some() {
        return None;
    }
    Some(operation)
}

fn operation_from_helper_call_name_and_args(
    name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyExprWithAwaitAndYield>,
) -> Option<operation::Operation<CoreBlockPyExprWithAwaitAndYield, ast::ExprName>> {
    operator_family_operation_from_helper_call(name, node_index.clone(), range, args.clone())
        .or_else(|| non_operator_operation_from_helper_call(name, node_index, range, args))
}

fn lower_core_call_expr_with_meta(
    func: Expr,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<Expr>,
    keywords: Vec<ast::Keyword>,
) -> CoreBlockPyExprWithAwaitAndYield {
    if keywords.is_empty() {
        if let Expr::Name(name) = &func {
            if name.id.as_str() == "__dp_make_function" && args.len() == 5 {
                if let (Some(function_id), Some(kind)) = (
                    make_function_id_from_literal(&args[0]),
                    make_function_kind_from_literal(&args[1]),
                ) {
                    return core_operation_expr(
                        operation::Operation::new(operation::MakeFunction::new(
                            function_id,
                            kind,
                            Box::new(CoreBlockPyExprWithAwaitAndYield::from(args[3].clone())),
                            Box::new(CoreBlockPyExprWithAwaitAndYield::from(args[4].clone())),
                        ))
                        .with_meta(Meta::new(node_index, range)),
                    );
                }
            }
            let mut operation_args = Vec::with_capacity(args.len());
            let mut saw_starred = false;
            for arg in &args {
                if matches!(arg, Expr::Starred(_)) {
                    saw_starred = true;
                    break;
                }
            }
            if !saw_starred {
                for arg in &args {
                    operation_args.push(CoreBlockPyExprWithAwaitAndYield::from(arg.clone()));
                }
                let helper_name = if name.id.as_str() == "__dp_ipow" {
                    "__dp_pow"
                } else {
                    name.id.as_str()
                };
                if helper_name == "__dp_pow" && operation_args.len() == 2 {
                    operation_args.push(core_builtin_name("__dp_NONE"));
                }
                if let Some(operation) = operation_from_helper_call_name_and_args(
                    helper_name,
                    node_index.clone(),
                    range,
                    operation_args,
                ) {
                    return core_operation_expr(operation);
                }
            }
        }
    }

    core_call_expr_with_meta(
        CoreBlockPyExprWithAwaitAndYield::from(func),
        node_index,
        range,
        lower_core_call_args(args),
        lower_core_call_keywords(keywords),
    )
}

fn reduce_core_tuple_splat(elts: Vec<Expr>) -> CoreBlockPyExprWithAwaitAndYield {
    let mut segments: Vec<CoreBlockPyExprWithAwaitAndYield> = Vec::new();
    let mut values: Vec<CoreBlockPyExprWithAwaitAndYield> = Vec::new();

    for elt in elts {
        match elt {
            Expr::Starred(ast::ExprStarred {
                value,
                node_index,
                range,
                ..
            }) => {
                if !values.is_empty() {
                    segments.push(core_positional_call_expr_with_meta(
                        "__dp_tuple",
                        ast::AtomicNodeIndex::default(),
                        Default::default(),
                        std::mem::take(&mut values),
                    ));
                }
                segments.push(core_positional_call_expr_with_meta(
                    "__dp_tuple_from_iter",
                    node_index,
                    range,
                    vec![CoreBlockPyExprWithAwaitAndYield::from(*value)],
                ));
            }
            other => values.push(CoreBlockPyExprWithAwaitAndYield::from(other)),
        }
    }

    if !values.is_empty() {
        segments.push(core_positional_call_expr_with_meta(
            "__dp_tuple",
            ast::AtomicNodeIndex::default(),
            Default::default(),
            values,
        ));
    }

    segments.into_iter().reduce(add_op_expr).unwrap_or_else(|| {
        core_positional_call_expr_with_meta(
            "__dp_tuple",
            ast::AtomicNodeIndex::default(),
            Default::default(),
            Vec::new(),
        )
    })
}

impl From<Expr> for CoreBlockPyExprWithAwaitAndYield {
    fn from(value: Expr) -> Self {
        match value {
            Expr::Call(node) => lower_core_call_expr_with_meta(
                *node.func,
                node.node_index,
                node.range,
                node.arguments.args.into_vec(),
                node.arguments.keywords.into_vec(),
            ),
            Expr::Await(node) => Self::Await(CoreBlockPyAwait {
                node_index: node.node_index,
                range: node.range,
                value: Box::new(Self::from(*node.value)),
            }),
            Expr::Yield(node) => Self::Yield(CoreBlockPyYield {
                node_index: node.node_index,
                range: node.range,
                value: node.value.map(|value| Box::new(Self::from(*value))),
            }),
            Expr::YieldFrom(node) => Self::YieldFrom(CoreBlockPyYieldFrom {
                node_index: node.node_index,
                range: node.range,
                value: Box::new(Self::from(*node.value)),
            }),
            Expr::StringLiteral(node) => {
                Self::Literal(CoreBlockPyLiteral::StringLiteral(CoreStringLiteral {
                    node_index: node.node_index,
                    range: node.range,
                    value: node.value.to_str().to_string(),
                }))
            }
            Expr::BytesLiteral(node) => {
                Self::Literal(CoreBlockPyLiteral::BytesLiteral(CoreBytesLiteral {
                    node_index: node.node_index,
                    range: node.range,
                    value: {
                        let value: std::borrow::Cow<[u8]> = (&node.value).into();
                        value.into_owned()
                    },
                }))
            }
            Expr::NumberLiteral(node) => {
                Self::Literal(CoreBlockPyLiteral::NumberLiteral(CoreNumberLiteral {
                    node_index: node.node_index,
                    range: node.range,
                    value: match node.value {
                        ast::Number::Int(value) => CoreNumberLiteralValue::Int(value),
                        ast::Number::Float(value) => CoreNumberLiteralValue::Float(value),
                        ast::Number::Complex { .. } => {
                            panic!("complex literal reached late core BlockPy boundary")
                        }
                    },
                }))
            }
            Expr::BooleanLiteral(node) => {
                if node.value {
                    core_builtin_name("__dp_TRUE")
                } else {
                    core_builtin_name("__dp_FALSE")
                }
            }
            Expr::NoneLiteral(_) => core_builtin_name("__dp_NONE"),
            Expr::EllipsisLiteral(_) => core_builtin_name("__dp_Ellipsis"),
            Expr::Attribute(node) if matches!(node.ctx, ast::ExprContext::Load) => {
                let value = Self::from(*node.value);
                getattr_expr_with_meta(
                    node.node_index,
                    node.range,
                    value,
                    node.attr.id.as_str().to_string(),
                )
            }
            Expr::Subscript(node) if matches!(node.ctx, ast::ExprContext::Load) => {
                let value = Self::from(*node.value);
                let index = Self::from(*node.slice);
                getitem_expr_with_meta(node.node_index, node.range, value, index)
            }
            Expr::UnaryOp(node) => {
                let operand = Self::from(*node.operand);
                unary_op_expr_from_ast_with_meta(node.node_index, node.range, node.op, operand)
            }
            Expr::BinOp(node) => {
                let left = Self::from(*node.left);
                let right = Self::from(*node.right);
                binop_expr_from_ast_with_meta(node.node_index, node.range, node.op, left, right)
            }
            Expr::Compare(node) if node.ops.len() == 1 && node.comparators.len() == 1 => {
                let node_index = node.node_index;
                let range = node.range;
                let left = *node.left;
                let right = node
                    .comparators
                    .into_vec()
                    .into_iter()
                    .next()
                    .expect("single compare comparator");
                let op = node
                    .ops
                    .into_vec()
                    .into_iter()
                    .next()
                    .expect("single compare op");
                compare_expr_from_ast_with_meta(
                    node_index,
                    range,
                    op,
                    Self::from(left),
                    Self::from(right),
                )
            }
            Expr::Tuple(node) if matches!(node.ctx, ast::ExprContext::Load) => {
                let tuple = if node.elts.iter().any(Expr::is_starred_expr) {
                    return reduce_core_tuple_splat(node.elts);
                } else {
                    make_tuple(node.elts)
                };
                Self::from(tuple)
            }
            Expr::List(node) if matches!(node.ctx, ast::ExprContext::Load) => {
                let tuple = if node.elts.iter().any(Expr::is_starred_expr) {
                    reduce_core_tuple_splat(node.elts)
                } else {
                    Self::from(make_tuple(node.elts))
                };
                core_positional_call_expr_with_meta(
                    "__dp_list",
                    node.node_index,
                    node.range,
                    vec![tuple],
                )
            }
            Expr::Set(node) => {
                let tuple = if node.elts.iter().any(Expr::is_starred_expr) {
                    reduce_core_tuple_splat(node.elts)
                } else {
                    Self::from(make_tuple(node.elts))
                };
                core_positional_call_expr_with_meta(
                    "__dp_set",
                    node.node_index,
                    node.range,
                    vec![tuple],
                )
            }
            Expr::Slice(node) => Self::from(py_expr!(
                "__dp_slice({lower:expr}, {upper:expr}, {step:expr})",
                lower = node
                    .lower
                    .map(|expr| *expr)
                    .unwrap_or_else(|| py_expr!("None")),
                upper = node
                    .upper
                    .map(|expr| *expr)
                    .unwrap_or_else(|| py_expr!("None")),
                step = node
                    .step
                    .map(|expr| *expr)
                    .unwrap_or_else(|| py_expr!("None")),
            )),
            Expr::Dict(node) => reduce_core_blockpy_dict(node.items.into()),
            Expr::Name(node) => {
                if is_internal_core_name(node.id.as_str()) {
                    Self::Name(node)
                } else {
                    CoreBlockPyExprWithAwaitAndYield::Op(
                        operation::Operation::new(operation::LoadName::new(node.clone()))
                            .with_meta(node.meta()),
                    )
                }
            }
            other => panic!(
                "unexpected expr reached late core BlockPy boundary: {}",
                crate::ruff_ast_to_string(&other)
            ),
        }
    }
}

fn finish_expr_setup(
    builder: CoreStmtBuilder,
) -> Vec<StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>> {
    let fragment = builder.finish();
    assert!(
        fragment.term.is_none(),
        "semantic-to-core expression lowering produced an unexpected terminator",
    );
    fragment.body
}

fn lower_semantic_expr_into(
    builder: &mut CoreStmtBuilder,
    expr: &SemanticExpr,
) -> CoreBlockPyExprWithAwaitAndYield {
    assert_expr_simplify_boundary(expr);
    let mut next_label_id = 0usize;
    let mut setup_builder = BlockPyStmtFragmentBuilder::<Expr>::new();
    let lowered_expr: Expr =
        lower_expr_into_with_setup(expr.clone(), &mut setup_builder, None, &mut next_label_id)
            .expect("semantic-to-core expression lowering should succeed");
    let setup_fragment = setup_builder.finish();
    let lowered_setup = lower_semantic_stmt_fragment(setup_fragment);
    assert!(
        lowered_setup.term.is_none(),
        "semantic-to-core expression setup lowering unexpectedly emitted a terminator",
    );
    builder.extend(lowered_setup.body);
    DefaultCoreExprReducer.reduce_expr(&lowered_expr)
}

fn lower_semantic_stmt_fragment(
    fragment: CoreLikeStmtFragmentInput,
) -> BlockPyStmtFragment<CoreBlockPyExprWithAwaitAndYield> {
    let mut builder = CoreStmtBuilder::new();
    for stmt in fragment.body {
        lower_semantic_stmt_into(&mut builder, stmt);
    }
    if let Some(term) = fragment.term {
        lower_semantic_term_into(&mut builder, term);
    }
    builder.finish()
}

type CoreLikeStmtFragmentInput = BlockPyStmtFragment<Expr>;

fn lower_semantic_stmt_into(builder: &mut CoreStmtBuilder, stmt: StructuredBlockPyStmt<Expr>) {
    match stmt {
        StructuredBlockPyStmt::Assign(assign) => {
            let mut setup = CoreStmtBuilder::new();
            let value = lower_semantic_expr_into(&mut setup, &assign.value);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(StructuredBlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value,
            }));
        }
        StructuredBlockPyStmt::Expr(expr) => {
            let mut setup = CoreStmtBuilder::new();
            let expr = lower_semantic_expr_into(&mut setup, &expr);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(StructuredBlockPyStmt::Expr(expr));
        }
        StructuredBlockPyStmt::Delete(BlockPyDelete { target }) => {
            builder.push_stmt(StructuredBlockPyStmt::Delete(BlockPyDelete { target }));
        }
        StructuredBlockPyStmt::If(if_stmt) => {
            let mut setup = CoreStmtBuilder::new();
            let test = lower_semantic_expr_into(&mut setup, &if_stmt.test);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(StructuredBlockPyStmt::If(BlockPyIf {
                test,
                body: lower_semantic_stmt_fragment(if_stmt.body),
                orelse: lower_semantic_stmt_fragment(if_stmt.orelse),
            }));
        }
    }
}

fn lower_semantic_term_into(builder: &mut CoreStmtBuilder, term: BlockPyTerm<Expr>) {
    match term {
        BlockPyTerm::Jump(edge) => {
            let mut args = Vec::with_capacity(edge.args.len());
            for arg in edge.args {
                args.push(match arg {
                    crate::block_py::BlockArg::Name(name) => crate::block_py::BlockArg::Name(name),
                    crate::block_py::BlockArg::None => crate::block_py::BlockArg::None,
                    crate::block_py::BlockArg::CurrentException => {
                        crate::block_py::BlockArg::CurrentException
                    }
                    crate::block_py::BlockArg::AbruptKind(kind) => {
                        crate::block_py::BlockArg::AbruptKind(kind)
                    }
                });
            }
            builder.set_term(BlockPyTerm::Jump(crate::block_py::BlockPyEdge::with_args(
                edge.target,
                args,
            )));
        }
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => {
            let mut setup = CoreStmtBuilder::new();
            let test = lower_semantic_expr_into(&mut setup, &test);
            builder.extend(finish_expr_setup(setup));
            builder.set_term(BlockPyTerm::IfTerm(BlockPyIfTerm {
                test,
                then_label,
                else_label,
            }));
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => {
            let mut setup = CoreStmtBuilder::new();
            let index = lower_semantic_expr_into(&mut setup, &index);
            builder.extend(finish_expr_setup(setup));
            builder.set_term(BlockPyTerm::BranchTable(BlockPyBranchTable {
                index,
                targets,
                default_label,
            }));
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            let exc = exc.map(|exc| {
                let mut setup = CoreStmtBuilder::new();
                let exc = lower_semantic_expr_into(&mut setup, &exc);
                builder.extend(finish_expr_setup(setup));
                exc
            });
            builder.set_term(BlockPyTerm::Raise(BlockPyRaise { exc }));
        }
        BlockPyTerm::Return(value) => {
            let mut setup = CoreStmtBuilder::new();
            let value = lower_semantic_expr_into(&mut setup, &value);
            builder.extend(finish_expr_setup(setup));
            builder.set_term(BlockPyTerm::Return(value));
        }
    }
}

fn lower_semantic_block<S>(
    block: CfgBlock<S, BlockPyTerm<RuffExpr>>,
) -> CfgBlock<
    StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
    BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
>
where
    S: IntoStructuredBlockPyStmt<RuffExpr, ast::ExprName>,
{
    let CfgBlock {
        label,
        body,
        term,
        params,
        exc_edge,
    } = block;
    let mut builder = CoreStmtBuilder::new();
    for stmt in body {
        lower_semantic_stmt_into(
            &mut builder,
            convert_blockpy_stmt_expr(stmt.into_structured_stmt()),
        );
    }
    lower_semantic_term_into(&mut builder, convert_blockpy_term_expr(term));
    let fragment = builder.finish();
    CfgBlock {
        label,
        body: fragment.body,
        term: fragment
            .term
            .expect("semantic BlockPy block must lower to a core terminator"),
        params,
        exc_edge,
    }
}

pub(crate) fn simplify_blockpy_callable_def_exprs(
    callable_def: BlockPyFunction<RuffBlockPyPass>,
) -> BlockPyFunction<CoreBlockPyPassWithAwaitAndYield> {
    let BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        storage_layout,
        semantic,
    } = callable_def;
    let structured_blocks = blocks
        .into_iter()
        .map(lower_semantic_block)
        .map(make_eval_order_explicit_in_core_block)
        .collect::<Vec<_>>();
    BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks: lower_structured_blocks_to_bb_blocks(&structured_blocks),
        doc,
        storage_layout,
        semantic,
    }
}

#[cfg(test)]
mod test;

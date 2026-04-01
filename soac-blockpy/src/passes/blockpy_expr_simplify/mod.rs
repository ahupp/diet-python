use super::ast_to_ast::string_templates::lower_string_templates_in_expr;
use crate::block_py::{
    core_call_expr_with_meta, core_runtime_name_expr_with_meta,
    core_runtime_positional_call_expr_with_meta, operation, BlockPyAssign, BlockPyBranchTable,
    BlockPyDelete, BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmtFragment,
    BlockPyStmtFragmentBuilder, BlockPyTerm, CoreBlockPyAwait, CoreBlockPyCallArg,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreBlockPyYield,
    CoreBlockPyYieldFrom, CoreBytesLiteral, CoreNumberLiteral, CoreNumberLiteralValue,
    CoreStringLiteral, HasMeta, Meta, StructuredBlockPyStmt, WithMeta,
};
use crate::passes::ast_to_ast::expr_utils::make_tuple;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

type SemanticExpr = Expr;

fn core_builtin_name(id: &str) -> CoreBlockPyExprWithAwaitAndYield {
    core_runtime_name_expr_with_meta(id, Default::default(), Default::default())
}

fn is_synthetic_local_core_name(id: &str) -> bool {
    id.starts_with("_dp_")
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
    let mut segments: Vec<CoreBlockPyExprWithAwaitAndYield> = Vec::new();
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
                    segments.push(CoreBlockPyExprWithAwaitAndYield::from(py_expr!(
                        "__soac__.dict({tuple:expr})",
                        tuple = tuple
                    )));
                }
                segments.push(CoreBlockPyExprWithAwaitAndYield::from(py_expr!(
                    "__soac__.dict({mapping:expr})",
                    mapping = value
                )));
            }
        }
    }

    if !keyed_pairs.is_empty() {
        let tuple = make_tuple(keyed_pairs);
        segments.push(CoreBlockPyExprWithAwaitAndYield::from(py_expr!(
            "__soac__.dict({tuple:expr})",
            tuple = tuple
        )));
    }

    let expr = match segments.len() {
        0 => core_runtime_positional_call_expr_with_meta(
            "dict",
            ast::AtomicNodeIndex::default(),
            Default::default(),
            Vec::new(),
        ),
        _ => segments
            .into_iter()
            .reduce(|left, right| {
                core_operation_expr(
                    operation::BinOp::new(
                        operation::BinOpKind::Or,
                        Box::new(left),
                        Box::new(right),
                    )
                    .with_meta(Meta::synthetic()),
                )
            })
            .expect("dict segments are non-empty"),
    };
    expr
}

fn core_operation_expr(
    operation: impl Into<operation::OperationDetail<CoreBlockPyExprWithAwaitAndYield>>,
) -> CoreBlockPyExprWithAwaitAndYield {
    CoreBlockPyExprWithAwaitAndYield::Op(operation.into())
}

fn core_operation_expr_with_meta(
    detail: impl Into<operation::OperationDetail<CoreBlockPyExprWithAwaitAndYield>>,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExprWithAwaitAndYield {
    core_operation_expr(detail.into().with_meta(Meta::new(node_index, range)))
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
    let attr_expr =
        CoreBlockPyExprWithAwaitAndYield::Literal(CoreBlockPyLiteral::StringLiteral(
            CoreStringLiteral {
                node_index: node_index.clone(),
                range,
                value: attr,
            },
        ));
    core_operation_expr_with_meta(
        operation::GetAttr::new(Box::new(value), Box::new(attr_expr)),
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
        _ => {
            let kind = match op {
                ast::Operator::Add => unreachable!("handled above"),
                ast::Operator::Sub => operation::BinOpKind::Sub,
                ast::Operator::Mult => operation::BinOpKind::Mul,
                ast::Operator::MatMult => operation::BinOpKind::MatMul,
                ast::Operator::Div => operation::BinOpKind::TrueDiv,
                ast::Operator::Mod => operation::BinOpKind::Mod,
                ast::Operator::Pow => operation::BinOpKind::Pow,
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
        ast::CmpOp::IsNot => unary_op_expr_with_meta(
            node_index.clone(),
            range,
            operation::UnaryOpKind::Not,
            binop_expr_with_meta(node_index, range, operation::BinOpKind::Is, left, right),
        ),
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

fn non_operator_operation_from_helper_call(
    name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyExprWithAwaitAndYield>,
) -> Option<operation::OperationDetail<CoreBlockPyExprWithAwaitAndYield>> {
    let mut args = args.into_iter();
    let meta = Meta::new(node_index, range);
    let operation = match name {
        "store_global" => operation::StoreName::new(
            {
                let _globals = args.next()?;
                string_arg_from_core_expr(args.next()?)?
            },
            Box::new(args.next()?),
        )
        .with_meta(meta)
        .into(),
        "cell_ref" => operation::CellRefForName::new(string_arg_from_core_expr(args.next()?)?)
            .with_meta(meta)
            .into(),
        _ => return None,
    };
    if args.next().is_some() {
        return None;
    }
    Some(operation)
}

fn lower_core_call_expr_with_meta(
    func: Expr,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<Expr>,
    keywords: Vec<ast::Keyword>,
) -> CoreBlockPyExprWithAwaitAndYield {
    if keywords.is_empty() {
        if let Expr::Attribute(attr) = &func {
            if matches!(attr.value.as_ref(), Expr::Name(base) if base.id.as_str() == "__soac__")
                && attr.attr.id.as_str() == "make_function"
                && args.len() == 5
            {
                if let (Some(function_id), Some(kind)) = (
                    make_function_id_from_literal(&args[0]),
                    make_function_kind_from_literal(&args[1]),
                ) {
                    return core_operation_expr(
                        operation::MakeFunction::new(
                            function_id,
                            kind,
                            Box::new(CoreBlockPyExprWithAwaitAndYield::from(args[3].clone())),
                            Box::new(CoreBlockPyExprWithAwaitAndYield::from(args[4].clone())),
                        )
                        .with_meta(Meta::new(node_index, range)),
                    );
                }
            }
        }
        if let Expr::Attribute(attr) = &func {
            if matches!(attr.value.as_ref(), Expr::Name(base) if base.id.as_str() == "__soac__") {
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
                    if let Some(operation) = non_operator_operation_from_helper_call(
                        attr.attr.id.as_str(),
                        node_index.clone(),
                        range,
                        operation_args,
                    ) {
                        return core_operation_expr(operation);
                    }
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
                    segments.push(core_runtime_positional_call_expr_with_meta(
                        "tuple_values",
                        ast::AtomicNodeIndex::default(),
                        Default::default(),
                        std::mem::take(&mut values),
                    ));
                }
                segments.push(core_runtime_positional_call_expr_with_meta(
                    "tuple_from_iter",
                    node_index,
                    range,
                    vec![CoreBlockPyExprWithAwaitAndYield::from(*value)],
                ));
            }
            other => values.push(CoreBlockPyExprWithAwaitAndYield::from(other)),
        }
    }

    if !values.is_empty() {
        segments.push(core_runtime_positional_call_expr_with_meta(
            "tuple_values",
            ast::AtomicNodeIndex::default(),
            Default::default(),
            values,
        ));
    }

    segments.into_iter().reduce(add_op_expr).unwrap_or_else(|| {
        core_runtime_positional_call_expr_with_meta(
            "tuple_values",
            ast::AtomicNodeIndex::default(),
            Default::default(),
            Vec::new(),
        )
    })
}

impl From<Expr> for CoreBlockPyExprWithAwaitAndYield {
    fn from(value: Expr) -> Self {
        let mut value = value;
        lower_string_templates_in_expr(&mut value);
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
                    core_builtin_name("TRUE")
                } else {
                    core_builtin_name("FALSE")
                }
            }
            Expr::NoneLiteral(_) => core_builtin_name("NONE"),
            Expr::EllipsisLiteral(_) => core_builtin_name("ELLIPSIS"),
            Expr::Attribute(node) if matches!(node.ctx, ast::ExprContext::Load) => {
                if matches!(
                    node.value.as_ref(),
                    Expr::Name(base) if base.id.as_str() == "__soac__"
                ) {
                    return core_runtime_name_expr_with_meta(
                        node.attr.id.as_str(),
                        node.node_index,
                        node.range,
                    );
                }
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
                core_runtime_positional_call_expr_with_meta(
                    "list",
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
                core_runtime_positional_call_expr_with_meta(
                    "set",
                    node.node_index,
                    node.range,
                    vec![tuple],
                )
            }
            Expr::Slice(node) => Self::from(py_expr!(
                "__soac__.slice({lower:expr}, {upper:expr}, {step:expr})",
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
                if is_synthetic_local_core_name(node.id.as_str()) || node.id.as_str() == "__soac__"
                {
                    Self::Name(node)
                } else {
                    CoreBlockPyExprWithAwaitAndYield::Op(
                        operation::LoadName::new(node.id.to_string())
                            .with_meta(node.meta())
                            .into(),
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

#[cfg(test)]
mod test;

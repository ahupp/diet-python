use super::ast_to_ast::rewrite_expr::string::lower_string_templates_in_expr;
use super::core_eval_order::make_eval_order_explicit_in_core_block;
use crate::block_py::{
    core_call_expr_with_meta, core_positional_call_expr_with_meta,
    core_positional_intrinsic_expr_with_meta, intrinsics, BlockPyAssign, BlockPyBranchTable,
    BlockPyCfgFragment, BlockPyDelete, BlockPyFunction, BlockPyIf, BlockPyIfTerm, BlockPyRaise,
    BlockPyStmt, BlockPyStmtFragment, BlockPyStmtFragmentBuilder, BlockPyTerm, CfgBlock,
    CoreBlockPyAwait, CoreBlockPyCallArg, CoreBlockPyExprWithAwaitAndYield, CoreBlockPyKeywordArg,
    CoreBlockPyLiteral, CoreBlockPyYield, CoreBlockPyYieldFrom, CoreBytesLiteral,
    CoreNumberLiteral, CoreNumberLiteralValue, CoreStringLiteral,
};
use crate::passes::ast_to_ast::expr_utils::{make_binop, make_tuple, make_unaryop};
use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
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

fn add_intrinsic_expr_with_meta(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    left: CoreBlockPyExprWithAwaitAndYield,
    right: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    core_positional_intrinsic_expr_with_meta(
        &intrinsics::ADD_INTRINSIC,
        node_index,
        range,
        vec![left, right],
    )
}

fn add_intrinsic_expr(
    left: CoreBlockPyExprWithAwaitAndYield,
    right: CoreBlockPyExprWithAwaitAndYield,
) -> CoreBlockPyExprWithAwaitAndYield {
    core_positional_intrinsic_expr_with_meta(
        &intrinsics::ADD_INTRINSIC,
        ast::AtomicNodeIndex::default(),
        Default::default(),
        vec![left, right],
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

fn lower_core_call_expr_with_meta(
    func: Expr,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<Expr>,
    keywords: Vec<ast::Keyword>,
) -> CoreBlockPyExprWithAwaitAndYield {
    if keywords.is_empty() {
        if let Expr::Name(name) = &func {
            if let Some(intrinsic) =
                intrinsics::intrinsic_by_name_and_arity(name.id.as_str(), args.len())
            {
                let mut intrinsic_args = Vec::with_capacity(args.len());
                for arg in &args {
                    if matches!(arg, Expr::Starred(_)) {
                        return core_call_expr_with_meta(
                            CoreBlockPyExprWithAwaitAndYield::from(func),
                            node_index,
                            range,
                            lower_core_call_args(args),
                            Vec::new(),
                        );
                    }
                }
                for arg in args {
                    intrinsic_args.push(CoreBlockPyExprWithAwaitAndYield::from(arg));
                }
                return core_positional_intrinsic_expr_with_meta(
                    intrinsic,
                    node_index,
                    range,
                    intrinsic_args,
                );
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

    segments
        .into_iter()
        .reduce(add_intrinsic_expr)
        .unwrap_or_else(|| {
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
                Self::from(py_expr!(
                    "__dp_getattr({value:expr}, {attr:literal})",
                    value = *node.value,
                    attr = node.attr.id.as_str(),
                ))
            }
            Expr::Subscript(node) if matches!(node.ctx, ast::ExprContext::Load) => {
                Self::from(py_expr!(
                    "__dp_getitem({value:expr}, {slice:expr})",
                    value = *node.value,
                    slice = *node.slice,
                ))
            }
            Expr::UnaryOp(node) => {
                let func_name = match node.op {
                    ast::UnaryOp::Not => "not_",
                    ast::UnaryOp::Invert => "invert",
                    ast::UnaryOp::USub => "neg",
                    ast::UnaryOp::UAdd => "pos",
                };
                Self::from(make_unaryop(func_name, *node.operand))
            }
            Expr::BinOp(node) => match node.op {
                ast::Operator::Add => add_intrinsic_expr_with_meta(
                    node.node_index,
                    node.range,
                    Self::from(*node.left),
                    Self::from(*node.right),
                ),
                _ => {
                    let func_name = match node.op {
                        ast::Operator::Add => unreachable!("handled above"),
                        ast::Operator::Sub => "sub",
                        ast::Operator::Mult => "mul",
                        ast::Operator::MatMult => "matmul",
                        ast::Operator::Div => "truediv",
                        ast::Operator::Mod => "mod",
                        ast::Operator::Pow => "pow",
                        ast::Operator::LShift => "lshift",
                        ast::Operator::RShift => "rshift",
                        ast::Operator::BitOr => "or_",
                        ast::Operator::BitXor => "xor",
                        ast::Operator::BitAnd => "and_",
                        ast::Operator::FloorDiv => "floordiv",
                    };
                    Self::from(make_binop(func_name, *node.left, *node.right))
                }
            },
            Expr::Compare(node) if node.ops.len() == 1 && node.comparators.len() == 1 => {
                let left = *node.left;
                let right = node
                    .comparators
                    .into_vec()
                    .into_iter()
                    .next()
                    .expect("single compare comparator");
                match node
                    .ops
                    .into_vec()
                    .into_iter()
                    .next()
                    .expect("single compare op")
                {
                    ast::CmpOp::Eq => Self::from(make_binop("eq", left, right)),
                    ast::CmpOp::NotEq => Self::from(make_binop("ne", left, right)),
                    ast::CmpOp::Lt => Self::from(make_binop("lt", left, right)),
                    ast::CmpOp::LtE => Self::from(make_binop("le", left, right)),
                    ast::CmpOp::Gt => Self::from(make_binop("gt", left, right)),
                    ast::CmpOp::GtE => Self::from(make_binop("ge", left, right)),
                    ast::CmpOp::Is => Self::from(make_binop("is_", left, right)),
                    ast::CmpOp::IsNot => Self::from(make_binop("is_not", left, right)),
                    ast::CmpOp::In => Self::from(make_binop("contains", right, left)),
                    ast::CmpOp::NotIn => {
                        Self::from(make_unaryop("not_", make_binop("contains", right, left)))
                    }
                }
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
            Expr::Name(node) => Self::Name(node),
            other => panic!(
                "unexpected expr reached late core BlockPy boundary: {}",
                crate::ruff_ast_to_string(&other)
            ),
        }
    }
}

fn finish_expr_setup(
    builder: CoreStmtBuilder,
) -> Vec<BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>> {
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

fn lower_semantic_stmt_into(builder: &mut CoreStmtBuilder, stmt: BlockPyStmt<Expr>) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            let mut setup = CoreStmtBuilder::new();
            let value = lower_semantic_expr_into(&mut setup, &assign.value);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value,
            }));
        }
        BlockPyStmt::Expr(expr) => {
            let mut setup = CoreStmtBuilder::new();
            let expr = lower_semantic_expr_into(&mut setup, &expr);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(BlockPyStmt::Expr(expr));
        }
        BlockPyStmt::Delete(BlockPyDelete { target }) => {
            builder.push_stmt(BlockPyStmt::Delete(BlockPyDelete { target }));
        }
        BlockPyStmt::If(if_stmt) => {
            let mut setup = CoreStmtBuilder::new();
            let test = lower_semantic_expr_into(&mut setup, &if_stmt.test);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(BlockPyStmt::If(BlockPyIf {
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

fn lower_semantic_block(
    block: CfgBlock<BlockPyStmt<Expr>, BlockPyTerm<Expr>>,
) -> CfgBlock<
    BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
    BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
> {
    let CfgBlock {
        label,
        body,
        term,
        params,
        exc_edge,
    } = block;
    let fragment = lower_semantic_stmt_fragment(BlockPyCfgFragment {
        body,
        term: Some(term),
    });
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
        closure_layout,
        facts,
        semantic,
    } = callable_def;
    BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks: blocks
            .into_iter()
            .map(lower_semantic_block)
            .map(make_eval_order_explicit_in_core_block)
            .collect(),
        doc,
        closure_layout,
        facts,
        semantic,
    }
}

#[cfg(test)]
mod test;

use super::ast_to_ast::rewrite_expr::string::lower_string_templates_in_expr;
use super::core_eval_order::make_eval_order_explicit_in_core_block;
use crate::block_py::{
    core_call_expr_with_meta, BlockPyAssign, BlockPyBranchTable, BlockPyCfgFragment, BlockPyDelete,
    BlockPyFunction, BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmt, BlockPyStmtFragment,
    BlockPyStmtFragmentBuilder, BlockPyTerm, CfgBlock, CoreBlockPyAwait, CoreBlockPyCallArg,
    CoreBlockPyExpr, CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreBlockPyYield,
    CoreBlockPyYieldFrom, CoreBytesLiteral, CoreNumberLiteral, CoreNumberLiteralValue,
    CoreStringLiteral,
};
use crate::passes::ast_to_ast::expr_utils::{
    make_binop, make_tuple, make_tuple_splat, make_unaryop,
};
use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
use crate::passes::{CoreBlockPyPass, RuffBlockPyPass};
use crate::py_expr;
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr};

#[cfg(test)]
use crate::block_py::BlockPyModule;

type CoreStmtBuilder = BlockPyStmtFragmentBuilder<CoreBlockPyExpr>;
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

fn core_builtin_name(id: &str) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Name(ast::ExprName {
        id: id.into(),
        ctx: ast::ExprContext::Load,
        range: Default::default(),
        node_index: ast::AtomicNodeIndex::default(),
    })
}

pub(crate) trait PureCoreExprReducer {
    fn reduce_expr(&self, expr: &SemanticExpr) -> CoreBlockPyExpr;
}

struct DefaultCoreExprReducer;

impl PureCoreExprReducer for DefaultCoreExprReducer {
    fn reduce_expr(&self, expr: &SemanticExpr) -> CoreBlockPyExpr {
        let mut expr = expr.clone();
        lower_string_templates_in_expr(&mut expr);
        expr.into()
    }
}

fn reduce_core_blockpy_dict(items: Box<[ast::DictItem]>) -> CoreBlockPyExpr {
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
    CoreBlockPyExpr::from(expr)
}

impl From<Expr> for CoreBlockPyExpr {
    fn from(value: Expr) -> Self {
        match value {
            Expr::Call(node) => core_call_expr_with_meta(
                Self::from(*node.func),
                node.node_index,
                node.range,
                node.arguments
                    .args
                    .into_vec()
                    .into_iter()
                    .map(|arg| match arg {
                        Expr::Starred(starred) => {
                            CoreBlockPyCallArg::Starred(Self::from(*starred.value))
                        }
                        other => CoreBlockPyCallArg::Positional(Self::from(other)),
                    })
                    .collect(),
                node.arguments
                    .keywords
                    .into_vec()
                    .into_iter()
                    .map(|keyword| match keyword.arg {
                        Some(arg) => CoreBlockPyKeywordArg::Named {
                            arg,
                            value: Self::from(keyword.value),
                        },
                        None => CoreBlockPyKeywordArg::Starred(Self::from(keyword.value)),
                    })
                    .collect(),
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
            Expr::BinOp(node) => {
                let func_name = match node.op {
                    ast::Operator::Add => "add",
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
                    make_tuple_splat(node.elts)
                } else {
                    make_tuple(node.elts)
                };
                Self::from(tuple)
            }
            Expr::List(node) if matches!(node.ctx, ast::ExprContext::Load) => {
                let tuple = if node.elts.iter().any(Expr::is_starred_expr) {
                    make_tuple_splat(node.elts)
                } else {
                    make_tuple(node.elts)
                };
                Self::from(py_expr!("__dp_list({tuple:expr})", tuple = tuple))
            }
            Expr::Set(node) => {
                let tuple = if node.elts.iter().any(Expr::is_starred_expr) {
                    make_tuple_splat(node.elts)
                } else {
                    make_tuple(node.elts)
                };
                Self::from(py_expr!("__dp_set({tuple:expr})", tuple = tuple))
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

fn finish_expr_setup(builder: CoreStmtBuilder) -> Vec<BlockPyStmt<CoreBlockPyExpr>> {
    let fragment = builder.finish();
    assert!(
        fragment.term.is_none(),
        "semantic-to-core expression lowering produced an unexpected terminator",
    );
    fragment.body
}

fn lower_semantic_expr_into(builder: &mut CoreStmtBuilder, expr: &SemanticExpr) -> CoreBlockPyExpr {
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

#[cfg(test)]
fn lower_semantic_expr_without_setup(expr: &SemanticExpr) -> CoreBlockPyExpr {
    let mut setup = CoreStmtBuilder::new();
    let lowered = lower_semantic_expr_into(&mut setup, expr);
    assert!(
        finish_expr_setup(setup).is_empty(),
        "semantic-to-core metadata expression lowering unexpectedly emitted setup statements",
    );
    lowered
}

fn lower_semantic_stmt_fragment(
    fragment: CoreLikeStmtFragmentInput,
) -> BlockPyStmtFragment<CoreBlockPyExpr> {
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
) -> CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>> {
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
) -> BlockPyFunction<CoreBlockPyPass> {
    let BlockPyFunction {
        function_id,
        names,
        kind,
        params,
        blocks,
        doc,
        closure_layout,
        facts,
        try_regions,
    } = callable_def;
    BlockPyFunction {
        function_id,
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
        try_regions,
    }
}

#[cfg(test)]
pub(crate) fn simplify_blockpy_module_exprs(
    module: BlockPyModule<RuffBlockPyPass>,
) -> TestCoreBlockPyModule {
    module.map_callable_defs(simplify_blockpy_callable_def_exprs)
}

#[cfg(test)]
type TestCoreBlockPyModule = BlockPyModule<CoreBlockPyPass>;

#[cfg(test)]
mod tests {
    use super::simplify_blockpy_module_exprs;
    use crate::block_py::{
        CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyKeywordArg, CoreBlockPyLiteral,
    };
    use crate::passes::RuffBlockPyPass;
    use crate::py_expr;
    use crate::ruff_ast_to_string;
    use crate::{transform_str_to_ruff_with_options, Options};
    use ruff_python_ast::Expr;
    use ruff_python_parser::parse_expression;

    #[test]
    fn expr_simplify_preserves_control_flow_but_reduces_exprs() {
        let source = r#"
def f(x):
    if x:
        return 1
    return 2
"#;
        let blockpy = transform_str_to_ruff_with_options(source, Options::for_test())
            .unwrap()
            .get_pass::<crate::block_py::BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
            .cloned()
            .expect("expected lowered semantic BlockPy module");
        let core = simplify_blockpy_module_exprs(blockpy.clone());
        let semantic_rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
        let core_rendered = crate::block_py::pretty::blockpy_module_to_string(&core);

        assert!(semantic_rendered.contains("function f(x):"));
        assert!(core_rendered.contains("function f(x):"));
        assert!(semantic_rendered.contains("return 1"));
        assert!(core_rendered.contains("return 1"));
    }

    #[test]
    fn expr_simplify_recurses_bottom_up_for_operator_family() {
        let expr = Expr::from(py_expr!("-(x + 1)"));
        let lowered = super::lower_semantic_expr_without_setup(&expr);

        let CoreBlockPyExpr::Call(outer) = lowered else {
            panic!("expected call-shaped core expr");
        };
        assert!(matches!(
            &*outer.func,
            CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_neg"
        ));
        let [CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Intrinsic(inner))] = &outer.args[..]
        else {
            panic!("expected __dp_neg to receive one lowered intrinsic arg");
        };
        assert_eq!(inner.intrinsic.name(), "__dp_add");
    }

    #[test]
    fn core_blockpy_expr_uses_reduced_variants_for_simple_shapes() {
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("x")),
            CoreBlockPyExpr::Name(_)
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("1")),
            CoreBlockPyExpr::Literal(CoreBlockPyLiteral::NumberLiteral(_))
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("f(x)")),
            CoreBlockPyExpr::Call(_)
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("await f(x)")),
            CoreBlockPyExpr::Await(_)
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("yield x")),
            CoreBlockPyExpr::Yield(_)
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("yield from xs")),
            CoreBlockPyExpr::YieldFrom(_)
        ));
    }

    #[test]
    fn core_blockpy_call_supports_star_args_and_kwargs() {
        let CoreBlockPyExpr::Call(call) = CoreBlockPyExpr::from(py_expr!("f(x, *args, y=z, **kw)"))
        else {
            panic!("expected reduced call expr");
        };
        assert!(matches!(&*call.func, CoreBlockPyExpr::Name(name) if name.id.as_str() == "f"));
        assert_eq!(call.args.len(), 2);
        assert!(matches!(call.args[0], CoreBlockPyCallArg::Positional(_)));
        assert!(matches!(call.args[1], CoreBlockPyCallArg::Starred(_)));
        assert_eq!(call.keywords.len(), 2);
        assert!(matches!(
            &call.keywords[0],
            CoreBlockPyKeywordArg::Named { arg, .. } if arg.as_str() == "y"
        ));
        assert!(matches!(
            call.keywords[1],
            CoreBlockPyKeywordArg::Starred(_)
        ));
    }

    #[test]
    fn core_blockpy_expr_reduces_add_to_structured_intrinsic() {
        let parsed = *parse_expression("x + y").unwrap().into_syntax().body;
        let CoreBlockPyExpr::Intrinsic(call) = CoreBlockPyExpr::from(parsed) else {
            panic!("expected intrinsic-shaped reduced expr for x + y");
        };
        assert_eq!(call.intrinsic.name(), "__dp_add");
    }

    #[test]
    fn core_blockpy_expr_keeps_other_reduced_helper_families_as_named_calls() {
        for (expr, helper_name) in [
            ("obj.attr", "__dp_getattr"),
            ("obj[idx]", "__dp_getitem"),
            ("-x", "__dp_neg"),
            ("x < y", "__dp_lt"),
            ("(x, y)", "__dp_tuple"),
            ("[x, y]", "__dp_list"),
            ("{x, y}", "__dp_set"),
            ("{x: y}", "__dp_dict"),
        ] {
            let parsed = *parse_expression(expr).unwrap().into_syntax().body;
            let CoreBlockPyExpr::Call(call) = CoreBlockPyExpr::from(parsed) else {
                panic!("expected call-shaped reduced expr for {expr}");
            };
            assert!(
                matches!(&*call.func, CoreBlockPyExpr::Name(name) if name.id.as_str() == helper_name),
                "{call:?}",
            );
        }
    }

    #[test]
    fn core_blockpy_expr_reuses_shared_tuple_splat_intrinsic_shape() {
        let parsed = *parse_expression("(x, *xs, y)").unwrap().into_syntax().body;
        let CoreBlockPyExpr::Intrinsic(call) = CoreBlockPyExpr::from(parsed) else {
            panic!("expected intrinsic-shaped reduced tuple expr");
        };
        assert_eq!(call.intrinsic.name(), "__dp_add");
        let rendered = ruff_ast_to_string(&Expr::from(CoreBlockPyExpr::Intrinsic(call)));
        assert!(rendered.contains("__dp_tuple_from_iter(xs)"), "{rendered}");
    }

    #[test]
    fn core_blockpy_expr_reuses_shared_tuple_splat_for_list_and_set() {
        for (expr, intrinsic) in [("[x, *xs, y]", "__dp_list"), ("{x, *xs, y}", "__dp_set")] {
            let parsed = *parse_expression(expr).unwrap().into_syntax().body;
            let CoreBlockPyExpr::Call(call) = CoreBlockPyExpr::from(parsed) else {
                panic!("expected call-shaped reduced expr for {expr}");
            };
            assert!(matches!(
                &*call.func,
                CoreBlockPyExpr::Name(name) if name.id.as_str() == intrinsic
            ));
            let [CoreBlockPyCallArg::Positional(tupleish)] = &call.args[..] else {
                panic!("expected one positional arg for {expr}");
            };
            let rendered = ruff_ast_to_string(&tupleish.to_expr());
            assert!(rendered.contains("__dp_tuple_from_iter(xs)"), "{rendered}");
        }
    }

    #[test]
    fn helper_scoped_families_do_not_reach_core_blockpy_boundary() {
        for expr in [
            "(lambda x: x + 1)",
            "[x for x in xs]",
            "{x for x in xs}",
            "{x: y for x, y in pairs}",
            "(x for x in xs)",
        ] {
            let parsed = *parse_expression(expr).unwrap().into_syntax().body;
            let panic = std::panic::catch_unwind(|| CoreBlockPyExpr::from(parsed));
            assert!(
                panic.is_err(),
                "{expr} should be lowered before the core boundary"
            );
        }
    }

    #[test]
    #[should_panic(
        expected = "helper-scoped expr leaked past rewrite_ast_to_lowered_blockpy_module_plan"
    )]
    fn semantic_expr_simplify_panics_on_nested_helper_scoped_expr_leak() {
        let expr = Expr::from(py_expr!("f(lambda x: x)"));
        let _ = super::lower_semantic_expr_without_setup(&expr);
    }

    #[test]
    fn semantic_blockpy_keeps_function_defaults_out_of_blockpy_ir() {
        let source = r#"
def f(*, d={"metaclass": Meta}, **kw):
    return d
"#;
        let blockpy = transform_str_to_ruff_with_options(source, Options::for_test())
            .unwrap()
            .get_pass::<crate::block_py::BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
            .cloned()
            .expect("expected lowered semantic BlockPy module");
        let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);

        assert!(rendered.contains("function f(*, d, **kw):"), "{rendered}");
        assert!(!rendered.contains("function f(*, d={"), "{rendered}");
        assert!(rendered.contains("__dp_make_function("), "{rendered}");
    }
}

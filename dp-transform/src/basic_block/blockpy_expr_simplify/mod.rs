use super::{
    ast_to_ast::rewrite_expr::string::lower_string_templates_in_expr,
    block_py::{
        BlockPyBlock, BlockPyBlockMeta, BlockPyBranchTable, BlockPyCallableDef, BlockPyCfgFragment,
        BlockPyDelete, BlockPyIf, BlockPyIfTerm, BlockPyModule, BlockPyRaise, BlockPyStmt,
        BlockPyStmtFragment, BlockPyStmtFragmentBuilder, BlockPyTerm, CoreBlockPyAwait,
        CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyKeywordArg,
        CoreBlockPyLiteral, CoreBlockPyYield, CoreBlockPyYieldFrom,
    },
};
use crate::basic_block::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
use crate::basic_block::{
    ast_to_ast::expr_utils::{make_binop, make_tuple, make_tuple_splat, make_unaryop},
    block_py::BlockPyAssign,
};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

type CoreStmtBuilder = BlockPyStmtFragmentBuilder<CoreBlockPyExpr>;
type SemanticExpr = Expr;

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
            Expr::Call(node) => Self::Call(CoreBlockPyCall {
                node_index: node.node_index,
                range: node.range,
                func: Box::new(Self::from(*node.func)),
                args: node
                    .arguments
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
                keywords: node
                    .arguments
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
            }),
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
            Expr::StringLiteral(node) => Self::Literal(CoreBlockPyLiteral::StringLiteral(node)),
            Expr::BytesLiteral(node) => Self::Literal(CoreBlockPyLiteral::BytesLiteral(node)),
            Expr::NumberLiteral(node) => Self::Literal(CoreBlockPyLiteral::NumberLiteral(node)),
            Expr::BooleanLiteral(node) => Self::Literal(CoreBlockPyLiteral::BooleanLiteral(node)),
            Expr::NoneLiteral(node) => Self::Literal(CoreBlockPyLiteral::NoneLiteral(node)),
            Expr::EllipsisLiteral(node) => Self::Literal(CoreBlockPyLiteral::EllipsisLiteral(node)),
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
    let mut next_label_id = 0usize;
    let mut setup_builder = BlockPyStmtFragmentBuilder::<Expr>::new();
    let lowered_expr: Expr =
        lower_expr_into_with_setup(expr.clone(), &mut setup_builder, None, &mut next_label_id)
            .expect("semantic-to-core expression lowering should succeed");
    let setup_fragment = setup_builder.finish();
    let lowered_setup = lower_semantic_stmt_fragment(&setup_fragment);
    assert!(
        lowered_setup.term.is_none(),
        "semantic-to-core expression setup lowering unexpectedly emitted a terminator",
    );
    builder.extend(lowered_setup.body);
    DefaultCoreExprReducer.reduce_expr(&lowered_expr)
}

fn lower_semantic_expr_without_setup(expr: &SemanticExpr) -> CoreBlockPyExpr {
    let mut setup = CoreStmtBuilder::new();
    let lowered = lower_semantic_expr_into(&mut setup, expr);
    assert!(
        finish_expr_setup(setup).is_empty(),
        "semantic-to-core metadata expression lowering unexpectedly emitted setup statements",
    );
    lowered
}

pub(crate) fn simplify_param_defaults(defaults: &[Expr]) -> Vec<CoreBlockPyExpr> {
    defaults
        .iter()
        .map(lower_semantic_expr_without_setup)
        .collect()
}

fn lower_semantic_stmt_fragment(
    fragment: &CoreLikeStmtFragmentInput,
) -> BlockPyStmtFragment<CoreBlockPyExpr> {
    let mut builder = CoreStmtBuilder::new();
    for stmt in &fragment.body {
        lower_semantic_stmt_into(&mut builder, stmt);
    }
    if let Some(term) = &fragment.term {
        lower_semantic_term_into(&mut builder, term);
    }
    builder.finish()
}

type CoreLikeStmtFragmentInput = BlockPyStmtFragment<Expr>;

fn lower_semantic_stmt_into(builder: &mut CoreStmtBuilder, stmt: &BlockPyStmt<Expr>) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            let mut setup = CoreStmtBuilder::new();
            let value = lower_semantic_expr_into(&mut setup, &assign.value);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target.clone(),
                value,
            }));
        }
        BlockPyStmt::Expr(expr) => {
            let mut setup = CoreStmtBuilder::new();
            let expr = lower_semantic_expr_into(&mut setup, expr);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(BlockPyStmt::Expr(expr));
        }
        BlockPyStmt::Delete(BlockPyDelete { target }) => {
            builder.push_stmt(BlockPyStmt::Delete(BlockPyDelete {
                target: target.clone(),
            }));
        }
        BlockPyStmt::If(if_stmt) => {
            let mut setup = CoreStmtBuilder::new();
            let test = lower_semantic_expr_into(&mut setup, &if_stmt.test);
            builder.extend(finish_expr_setup(setup));
            builder.push_stmt(BlockPyStmt::If(BlockPyIf {
                test,
                body: lower_semantic_stmt_fragment(&if_stmt.body),
                orelse: lower_semantic_stmt_fragment(&if_stmt.orelse),
            }));
        }
    }
}

fn lower_semantic_term_into(builder: &mut CoreStmtBuilder, term: &BlockPyTerm<Expr>) {
    match term {
        BlockPyTerm::Jump(label) => builder.set_term(BlockPyTerm::Jump(label.clone())),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => {
            let mut setup = CoreStmtBuilder::new();
            let test = lower_semantic_expr_into(&mut setup, test);
            builder.extend(finish_expr_setup(setup));
            builder.set_term(BlockPyTerm::IfTerm(BlockPyIfTerm {
                test,
                then_label: then_label.clone(),
                else_label: else_label.clone(),
            }));
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => {
            let mut setup = CoreStmtBuilder::new();
            let index = lower_semantic_expr_into(&mut setup, index);
            builder.extend(finish_expr_setup(setup));
            builder.set_term(BlockPyTerm::BranchTable(BlockPyBranchTable {
                index,
                targets: targets.clone(),
                default_label: default_label.clone(),
            }));
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            let exc = exc.as_ref().map(|exc| {
                let mut setup = CoreStmtBuilder::new();
                let exc = lower_semantic_expr_into(&mut setup, exc);
                builder.extend(finish_expr_setup(setup));
                exc
            });
            builder.set_term(BlockPyTerm::Raise(BlockPyRaise { exc }));
        }
        BlockPyTerm::TryJump(try_jump) => builder.set_term(BlockPyTerm::TryJump(try_jump.clone())),
        BlockPyTerm::Return(value) => {
            let value = value.as_ref().map(|value| {
                let mut setup = CoreStmtBuilder::new();
                let value = lower_semantic_expr_into(&mut setup, value);
                builder.extend(finish_expr_setup(setup));
                value
            });
            builder.set_term(BlockPyTerm::Return(value));
        }
    }
}

fn lower_semantic_block(block: &BlockPyBlock<Expr>) -> BlockPyBlock<CoreBlockPyExpr> {
    let fragment = lower_semantic_stmt_fragment(&BlockPyCfgFragment {
        body: block.body.clone(),
        term: Some(block.term.clone()),
    });
    BlockPyBlock {
        label: block.label.clone(),
        body: fragment.body,
        term: fragment
            .term
            .expect("semantic BlockPy block must lower to a core terminator"),
        meta: BlockPyBlockMeta {
            exc_param: block.meta.exc_param.clone(),
        },
    }
}

pub(crate) fn simplify_blockpy_callable_def_exprs<X: Clone>(
    callable_def: &BlockPyCallableDef<Expr, BlockPyBlock<Expr>, X>,
) -> BlockPyCallableDef<CoreBlockPyExpr, BlockPyBlock<CoreBlockPyExpr>, X> {
    BlockPyCallableDef {
        function_id: callable_def.function_id,
        names: callable_def.names.clone(),
        kind: callable_def.kind,
        params: callable_def.params.clone(),
        param_defaults: simplify_param_defaults(&callable_def.param_defaults),
        blocks: callable_def
            .blocks
            .iter()
            .map(lower_semantic_block)
            .collect(),
        doc: callable_def.doc.clone(),
        closure_layout: callable_def.closure_layout.clone(),
        facts: callable_def.facts.clone(),
        try_regions: callable_def.try_regions.clone(),
        extra: callable_def.extra.clone(),
    }
}

pub(crate) fn simplify_blockpy_module_exprs(
    module: &BlockPyModule<Expr>,
) -> BlockPyModule<CoreBlockPyExpr> {
    BlockPyModule {
        callable_defs: module
            .callable_defs
            .iter()
            .map(simplify_blockpy_callable_def_exprs)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::simplify_blockpy_module_exprs;
    use crate::basic_block::block_py::pretty::blockpy_module_to_string;
    use crate::basic_block::block_py::{
        CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyKeywordArg, CoreBlockPyLiteral,
    };
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
            .get_pass::<crate::basic_block::block_py::BlockPyModule<Expr>>("semantic_blockpy")
            .cloned()
            .expect("expected lowered semantic BlockPy module");
        let core = simplify_blockpy_module_exprs(&blockpy);
        let semantic_rendered = blockpy_module_to_string(&blockpy);
        let core_rendered = blockpy_module_to_string(&core);

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
        let [CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Call(inner))] = &outer.args[..] else {
            panic!("expected __dp_neg to receive one lowered call arg");
        };
        assert!(matches!(
            &*inner.func,
            CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_add"
        ));
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
    fn core_blockpy_expr_reduces_local_expr_forms_to_intrinsic_calls() {
        for (expr, intrinsic) in [
            ("obj.attr", "__dp_getattr"),
            ("obj[idx]", "__dp_getitem"),
            ("-x", "__dp_neg"),
            ("x + y", "__dp_add"),
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
                matches!(&*call.func, CoreBlockPyExpr::Name(name) if name.id.as_str() == intrinsic),
                "{call:?}",
            );
        }
    }

    #[test]
    fn core_blockpy_expr_reuses_shared_tuple_splat_intrinsic_shape() {
        let parsed = *parse_expression("(x, *xs, y)").unwrap().into_syntax().body;
        let CoreBlockPyExpr::Call(call) = CoreBlockPyExpr::from(parsed) else {
            panic!("expected call-shaped reduced tuple expr");
        };
        assert!(matches!(
            &*call.func,
            CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_add"
        ));
        let rendered = ruff_ast_to_string(&Expr::from(CoreBlockPyExpr::Call(call)));
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
    fn core_blockpy_expr_simplifies_function_default_exprs() {
        let source = r#"
def f(*, d={"metaclass": Meta}, **kw):
    return d
"#;
        let blockpy = transform_str_to_ruff_with_options(source, Options::for_test())
            .unwrap()
            .get_pass::<crate::basic_block::block_py::BlockPyModule<Expr>>("semantic_blockpy")
            .cloned()
            .expect("expected lowered semantic BlockPy module");
        let core = simplify_blockpy_module_exprs(&blockpy);
        let rendered = blockpy_module_to_string(&core);

        assert!(rendered.contains("__dp_dict("), "{rendered}");
        assert!(!rendered.contains("{\"metaclass\": Meta}"), "{rendered}");
    }
}

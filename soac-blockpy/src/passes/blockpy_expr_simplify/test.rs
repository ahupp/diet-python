use super::*;

use crate::block_py::pretty::BlockPyDebugExprText;
use crate::block_py::{BinOpKind, OperationDetail, TernaryOpKind, UnaryOpKind};

fn lower_semantic_expr_without_setup(expr: &SemanticExpr) -> CoreBlockPyExprWithAwaitAndYield {
    let mut setup = CoreStmtBuilder::new();
    let lowered = lower_semantic_expr_into(&mut setup, expr);
    assert!(
        finish_expr_setup(setup).is_empty(),
        "semantic-to-core metadata expression lowering unexpectedly emitted setup statements",
    );
    lowered
}

use crate::block_py::{
    CoreBlockPyCallArg, CoreBlockPyExprWithAwaitAndYield, CoreBlockPyKeywordArg, CoreBlockPyLiteral,
};
use crate::lower_python_to_blockpy_for_testing;
use crate::py_expr;
use ruff_python_parser::parse_expression;

fn is_raw_load_name_expr(expr: &CoreBlockPyExprWithAwaitAndYield, expected: &str) -> bool {
    matches!(
        expr,
        CoreBlockPyExprWithAwaitAndYield::Op(operation)
            if matches!(operation.detail(), crate::block_py::OperationDetail::LoadName(op) if op.name.id.as_str() == expected)
    )
}

#[test]
fn expr_simplify_preserves_control_flow_but_reduces_exprs() {
    let source = r#"
def f(x):
    if x:
        return 1
    return 2
"#;
    let core = lower_python_to_blockpy_for_testing(source)
        .unwrap()
        .pass_tracker
        .pass_core_blockpy_with_await_and_yield()
        .cloned()
        .expect("expected lowered core BlockPy module");
    let core_rendered = crate::block_py::pretty::blockpy_module_to_string(&core);

    assert!(core_rendered.contains("function f(x):"));
    assert!(core_rendered.contains("return 1"));
}

#[test]
fn expr_simplify_recurses_bottom_up_for_operator_family() {
    let expr = Expr::from(py_expr!("-(x + 1)"));
    let lowered = lower_semantic_expr_without_setup(&expr);

    let CoreBlockPyExprWithAwaitAndYield::Op(outer) = lowered else {
        panic!("expected operation-shaped core expr");
    };
    assert!(matches!(
        outer.detail(),
        OperationDetail::UnaryOp(op) if op.kind == UnaryOpKind::Neg
    ));
    let OperationDetail::UnaryOp(op) = outer.detail() else {
        unreachable!("neg guard should ensure unary op");
    };
    let CoreBlockPyExprWithAwaitAndYield::Op(inner) = op.operand.as_ref() else {
        panic!("expected __dp_neg to receive one lowered op arg");
    };
    assert!(matches!(
        inner.detail(),
        OperationDetail::BinOp(op) if op.kind == BinOpKind::Add
    ));
}

#[test]
fn core_blockpy_expr_uses_reduced_variants_for_simple_shapes() {
    assert!(is_raw_load_name_expr(
        &CoreBlockPyExprWithAwaitAndYield::from(py_expr!("x")),
        "x"
    ));
    assert!(matches!(
        CoreBlockPyExprWithAwaitAndYield::from(py_expr!("1")),
        CoreBlockPyExprWithAwaitAndYield::Literal(CoreBlockPyLiteral::NumberLiteral(_))
    ));
    assert!(matches!(
        CoreBlockPyExprWithAwaitAndYield::from(py_expr!("f(x)")),
        CoreBlockPyExprWithAwaitAndYield::Call(_)
    ));
    assert!(matches!(
        CoreBlockPyExprWithAwaitAndYield::from(py_expr!("await f(x)")),
        CoreBlockPyExprWithAwaitAndYield::Await(_)
    ));
    assert!(matches!(
        CoreBlockPyExprWithAwaitAndYield::from(py_expr!("yield x")),
        CoreBlockPyExprWithAwaitAndYield::Yield(_)
    ));
    assert!(matches!(
        CoreBlockPyExprWithAwaitAndYield::from(py_expr!("yield from xs")),
        CoreBlockPyExprWithAwaitAndYield::YieldFrom(_)
    ));
}

#[test]
fn core_blockpy_call_supports_star_args_and_kwargs() {
    let CoreBlockPyExprWithAwaitAndYield::Call(call) =
        CoreBlockPyExprWithAwaitAndYield::from(py_expr!("f(x, *args, y=z, **kw)"))
    else {
        panic!("expected reduced call expr");
    };
    assert!(is_raw_load_name_expr(call.func.as_ref(), "f"));
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
    let CoreBlockPyExprWithAwaitAndYield::Op(call) = CoreBlockPyExprWithAwaitAndYield::from(parsed)
    else {
        panic!("expected operation-shaped reduced expr for x + y");
    };
    assert!(matches!(
        call.detail(),
        OperationDetail::BinOp(op) if op.kind == BinOpKind::Add
    ));
}

#[test]
fn core_blockpy_expr_reduces_operator_helper_families_to_intrinsics() {
    for expr in ["obj.attr", "obj[idx]", "-x", "x < y", "x in y", "x is y"] {
        let parsed = *parse_expression(expr).unwrap().into_syntax().body;
        let CoreBlockPyExprWithAwaitAndYield::Op(call) =
            CoreBlockPyExprWithAwaitAndYield::from(parsed)
        else {
            panic!("expected operation-shaped reduced expr for {expr}");
        };
        let matches_expected = match expr {
            "obj.attr" => matches!(call.detail(), OperationDetail::GetAttr(_)),
            "obj[idx]" => matches!(call.detail(), OperationDetail::GetItem(_)),
            "-x" => {
                matches!(call.detail(), OperationDetail::UnaryOp(op) if op.kind == UnaryOpKind::Neg)
            }
            "x < y" => {
                matches!(call.detail(), OperationDetail::BinOp(op) if op.kind == BinOpKind::Lt)
            }
            "x in y" => {
                matches!(call.detail(), OperationDetail::BinOp(op) if op.kind == BinOpKind::Contains)
            }
            "x is y" => {
                matches!(call.detail(), OperationDetail::BinOp(op) if op.kind == BinOpKind::Is)
            }
            _ => unreachable!(),
        };
        assert!(matches_expected, "{call:?}");
    }
}

#[test]
fn core_blockpy_expr_rewrites_ipow_helper_to_pow_operation() {
    let parsed = *parse_expression("__dp_ipow(x, y)")
        .unwrap()
        .into_syntax()
        .body;
    let CoreBlockPyExprWithAwaitAndYield::Op(call) = CoreBlockPyExprWithAwaitAndYield::from(parsed)
    else {
        panic!("expected operation-shaped reduced expr for __dp_ipow(x, y)");
    };
    assert!(matches!(
        call.detail(),
        OperationDetail::TernaryOp(op) if op.kind == TernaryOpKind::Pow
    ));
    let OperationDetail::TernaryOp(op) = call.detail() else {
        unreachable!("pow guard should ensure ternary op");
    };
    let _left = op.base.as_ref();
    let _right = op.exponent.as_ref();
    assert!(
        matches!(op.modulus.as_ref(), CoreBlockPyExprWithAwaitAndYield::Name(name) if name.id.as_str() == "__dp_NONE")
    );
}

#[test]
fn core_blockpy_expr_keeps_non_intrinsic_helper_families_as_named_calls() {
    for (expr, helper_name) in [
        ("(x, y)", "__dp_tuple"),
        ("[x, y]", "__dp_list"),
        ("{x, y}", "__dp_set"),
        ("{x: y}", "__dp_dict"),
    ] {
        let parsed = *parse_expression(expr).unwrap().into_syntax().body;
        let CoreBlockPyExprWithAwaitAndYield::Call(call) =
            CoreBlockPyExprWithAwaitAndYield::from(parsed)
        else {
            panic!("expected call-shaped reduced expr for {expr}");
        };
        assert!(
            matches!(&*call.func, CoreBlockPyExprWithAwaitAndYield::Name(name) if name.id.as_str() == helper_name),
            "{call:?}",
        );
    }
}

#[test]
fn core_blockpy_expr_reuses_shared_tuple_splat_intrinsic_shape() {
    let parsed = *parse_expression("(x, *xs, y)").unwrap().into_syntax().body;
    let CoreBlockPyExprWithAwaitAndYield::Op(call) = CoreBlockPyExprWithAwaitAndYield::from(parsed)
    else {
        panic!("expected operation-shaped reduced tuple expr");
    };
    assert!(matches!(
        call.detail(),
        OperationDetail::BinOp(op) if op.kind == BinOpKind::Add
    ));
    let rendered = CoreBlockPyExprWithAwaitAndYield::Op(call).debug_expr_text();
    assert!(rendered.contains("__dp_tuple_from_iter(xs)"), "{rendered}");
}

#[test]
fn core_blockpy_expr_reuses_shared_tuple_splat_for_list_and_set() {
    for (expr, intrinsic) in [("[x, *xs, y]", "__dp_list"), ("{x, *xs, y}", "__dp_set")] {
        let parsed = *parse_expression(expr).unwrap().into_syntax().body;
        let CoreBlockPyExprWithAwaitAndYield::Call(call) =
            CoreBlockPyExprWithAwaitAndYield::from(parsed)
        else {
            panic!("expected call-shaped reduced expr for {expr}");
        };
        assert!(matches!(
            &*call.func,
            CoreBlockPyExprWithAwaitAndYield::Name(name) if name.id.as_str() == intrinsic
        ));
        let [CoreBlockPyCallArg::Positional(tupleish)] = &call.args[..] else {
            panic!("expected one positional arg for {expr}");
        };
        let rendered = tupleish.debug_expr_text();
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
        let panic = std::panic::catch_unwind(|| CoreBlockPyExprWithAwaitAndYield::from(parsed));
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
    let _ = lower_semantic_expr_without_setup(&expr);
}

#[test]
fn core_blockpy_keeps_function_defaults_out_of_blockpy_ir() {
    let source = r#"
def f(*, d={"metaclass": Meta}, **kw):
    return d
"#;
    let blockpy = lower_python_to_blockpy_for_testing(source)
        .unwrap()
        .pass_tracker
        .pass_core_blockpy_with_await_and_yield()
        .cloned()
        .expect("expected lowered core BlockPy module");
    let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);

    assert!(rendered.contains("function f(*, d, **kw):"), "{rendered}");
    assert!(!rendered.contains("function f(*, d={"), "{rendered}");
    assert!(rendered.contains("MakeFunction("), "{rendered}");
}

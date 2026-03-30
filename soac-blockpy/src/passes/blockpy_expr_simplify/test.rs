use super::*;

use crate::block_py::BlockPyModule;

fn lower_semantic_expr_without_setup(expr: &SemanticExpr) -> CoreBlockPyExprWithAwaitAndYield {
    let mut setup = CoreStmtBuilder::new();
    let lowered = lower_semantic_expr_into(&mut setup, expr);
    assert!(
        finish_expr_setup(setup).is_empty(),
        "semantic-to-core metadata expression lowering unexpectedly emitted setup statements",
    );
    lowered
}

pub(crate) fn simplify_blockpy_module_exprs(
    module: BlockPyModule<RuffBlockPyPass>,
) -> TestCoreBlockPyModule {
    module.map_callable_defs(simplify_blockpy_callable_def_exprs)
}

type TestCoreBlockPyModule = BlockPyModule<CoreBlockPyPassWithAwaitAndYield>;

use crate::block_py::{
    CoreBlockPyCallArg, CoreBlockPyExprWithAwaitAndYield, CoreBlockPyKeywordArg, CoreBlockPyLiteral,
};
use crate::lower_python_to_blockpy_recorded;
use crate::passes::RuffBlockPyPass;
use crate::py_expr;
use crate::ruff_ast_to_string;
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
    let blockpy = lower_python_to_blockpy_recorded(source)
        .unwrap()
        .pass_tracker
        .get::<crate::block_py::BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
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
    let lowered = lower_semantic_expr_without_setup(&expr);

    let CoreBlockPyExprWithAwaitAndYield::Op(outer) = lowered else {
        panic!("expected operation-shaped core expr");
    };
    assert_eq!(outer.helper_name(), "__dp_neg");
    let outer_args = (*outer).clone().into_call_args();
    let [CoreBlockPyExprWithAwaitAndYield::Op(inner)] = &outer_args[..] else {
        panic!("expected __dp_neg to receive one lowered op arg");
    };
    assert_eq!(inner.helper_name(), "__dp_add");
}

#[test]
fn core_blockpy_expr_uses_reduced_variants_for_simple_shapes() {
    assert!(matches!(
        CoreBlockPyExprWithAwaitAndYield::from(py_expr!("x")),
        CoreBlockPyExprWithAwaitAndYield::Name(_)
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
    assert!(
        matches!(&*call.func, CoreBlockPyExprWithAwaitAndYield::Name(name) if name.id.as_str() == "f")
    );
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
    assert_eq!(call.helper_name(), "__dp_add");
}

#[test]
fn core_blockpy_expr_reduces_operator_helper_families_to_intrinsics() {
    for (expr, helper_name) in [
        ("obj.attr", "__dp_getattr"),
        ("obj[idx]", "__dp_getitem"),
        ("-x", "__dp_neg"),
        ("x < y", "__dp_lt"),
        ("x in y", "__dp_contains"),
        ("x is y", "__dp_is_"),
    ] {
        let parsed = *parse_expression(expr).unwrap().into_syntax().body;
        let CoreBlockPyExprWithAwaitAndYield::Op(call) =
            CoreBlockPyExprWithAwaitAndYield::from(parsed)
        else {
            panic!("expected operation-shaped reduced expr for {expr}");
        };
        assert_eq!(call.helper_name(), helper_name, "{call:?}");
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
    assert_eq!(call.helper_name(), "__dp_pow");
    assert_eq!(call.call_args().len(), 3);
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
    assert_eq!(call.helper_name(), "__dp_add");
    let rendered = ruff_ast_to_string(&Expr::from(CoreBlockPyExprWithAwaitAndYield::Op(call)));
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
fn semantic_blockpy_keeps_function_defaults_out_of_blockpy_ir() {
    let source = r#"
def f(*, d={"metaclass": Meta}, **kw):
    return d
"#;
    let blockpy = lower_python_to_blockpy_recorded(source)
        .unwrap()
        .pass_tracker
        .get::<crate::block_py::BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
        .cloned()
        .expect("expected lowered semantic BlockPy module");
    let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);

    assert!(rendered.contains("function f(*, d, **kw):"), "{rendered}");
    assert!(!rendered.contains("function f(*, d={"), "{rendered}");
    assert!(rendered.contains("__dp_make_function("), "{rendered}");
}

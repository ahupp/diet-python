use super::rewrite_exprs;

#[test]
fn rewrite_exprs_applies_decorators_inside_out() {
    let decorated = rewrite_exprs(
        vec![crate::py_expr!("d1"), crate::py_expr!("d2")],
        crate::py_expr!("f"),
    );
    assert_eq!(crate::ruff_ast_to_string(&decorated).trim(), "d1(d2(f))");
}

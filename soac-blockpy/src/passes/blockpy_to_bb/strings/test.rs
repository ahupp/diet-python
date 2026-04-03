use super::normalize_bb_module_strings;
use crate::{
    block_py::{
        BlockPyExprLike, BlockPyLiteral, BlockPyNameLike, BlockPyTerm, CodegenBlockPyExpr,
        CoreBlockPyExpr, InstrExprNode, LocatedCoreBlockPyExpr,
    },
    lower_python_to_blockpy_for_testing,
    passes::lower_try_jump_exception_flow,
};

fn tracked_name_binding_module(
    source: &str,
) -> crate::block_py::BlockPyModule<crate::passes::ResolvedStorageBlockPyPass> {
    lower_python_to_blockpy_for_testing(source)
        .expect("transform should succeed")
        .pass_tracker
        .pass_name_binding()
        .expect("bb module should be available")
        .clone()
}

fn expr_contains_literal(expr: &CodegenBlockPyExpr) -> bool {
    match expr {
        CodegenBlockPyExpr::Literal(_) => true,
        _ => {
            let mut saw_literal = false;
            expr.walk_child_exprs(&mut |arg| {
                if expr_contains_literal(arg) {
                    saw_literal = true;
                }
            });
            saw_literal
        }
    }
}

fn module_constants_contain_string(exprs: &[LocatedCoreBlockPyExpr]) -> bool {
    exprs.iter().any(|expr| {
        matches!(
            expr,
            CoreBlockPyExpr::Literal(literal)
                if matches!(literal.as_literal(), BlockPyLiteral::StringLiteral(_))
        )
    })
}

fn collect_helper_like_names_in_expr(out: &mut Vec<String>, expr: &CodegenBlockPyExpr) {
    match expr {
        CodegenBlockPyExpr::Literal(_) => {}
        CodegenBlockPyExpr::GetAttr(operation) => {
            out.push("__dp_getattr".to_string());
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::SetAttr(operation) => {
            out.push("__dp_setattr".to_string());
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::GetItem(operation) => {
            out.push("__dp_getitem".to_string());
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::SetItem(operation) => {
            out.push("__dp_setitem".to_string());
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::Call(operation) => {
            if let CodegenBlockPyExpr::Load(op) = &*operation.func {
                out.push(op.name.id_str().to_string());
            }
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::BinOp(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::UnaryOp(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::Load(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::Store(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::Del(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::MakeCell(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::CellRefForName(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::CellRef(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::MakeFunction(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::DelItem(operation) => {
            operation.visit_exprs(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
    }
}

#[test]
fn keeps_string_literals_in_module_constants_and_out_of_executable_codegen() {
    let source = r#"
def f():
    x = __dp_store_global(globals(), "classify", __dp_ret("ok"))
    return x
"#;
    let bb_module = tracked_name_binding_module(source);
    let prepared = lower_try_jump_exception_flow(&bb_module);
    let normalized = normalize_bb_module_strings(&prepared);

    assert!(
        module_constants_contain_string(&normalized.module_constants),
        "expected normalized module constants to retain string literals"
    );

    for function in normalized.callable_defs {
        for block in &function.blocks {
            for stmt in &block.body {
                assert!(
                    !expr_contains_literal(stmt),
                    "expr stmt should not retain executable literals: {stmt:?}"
                );
            }
            match &block.term {
                BlockPyTerm::Jump(_) => {}
                BlockPyTerm::IfTerm(if_term) => assert!(
                    !expr_contains_literal(&if_term.test),
                    "if test should not retain executable literals: {:?}",
                    if_term.test
                ),
                BlockPyTerm::BranchTable(branch) => assert!(
                    !expr_contains_literal(&branch.index),
                    "branch index should not retain executable literals: {:?}",
                    branch.index
                ),
                BlockPyTerm::Raise(raise_stmt) => {
                    if let Some(exc) = &raise_stmt.exc {
                        assert!(
                            !expr_contains_literal(exc),
                            "raise value should not retain executable literals: {exc:?}"
                        );
                    }
                }
                BlockPyTerm::Return(value) => assert!(
                    !expr_contains_literal(value),
                    "return value should not retain executable literals: {value:?}"
                ),
            }
        }
    }
}

#[test]
fn preserves_structured_intrinsics_for_attr_and_item_helpers() {
    let source = r#"
def f(obj, mapping, key, value):
    a = obj.x
    obj.x = value
    b = mapping[key]
    mapping[key] = value
    return a, b
"#;
    let bb_module = tracked_name_binding_module(source);
    let prepared = lower_try_jump_exception_flow(&bb_module);
    let normalized = normalize_bb_module_strings(&prepared);

    let mut helper_names = Vec::new();
    for function in normalized.callable_defs {
        for block in &function.blocks {
            for stmt in &block.body {
                collect_helper_like_names_in_expr(&mut helper_names, stmt);
            }
        }
    }

    assert!(
        helper_names.iter().any(|name| name == "__dp_getattr"),
        "{helper_names:?}"
    );
    assert!(
        helper_names.iter().any(|name| name == "__dp_setattr"),
        "{helper_names:?}"
    );
    assert!(
        helper_names.iter().any(|name| name == "__dp_getitem"),
        "{helper_names:?}"
    );
    assert!(
        helper_names.iter().any(|name| name == "__dp_setitem"),
        "{helper_names:?}"
    );
    assert!(
        !helper_names.iter().any(|name| name == "PyObject_GetAttr"),
        "{helper_names:?}"
    );
    assert!(
        !helper_names.iter().any(|name| name == "PyObject_SetAttr"),
        "{helper_names:?}"
    );
    assert!(
        !helper_names.iter().any(|name| name == "PyObject_GetItem"),
        "{helper_names:?}"
    );
    assert!(
        !helper_names.iter().any(|name| name == "PyObject_SetItem"),
        "{helper_names:?}"
    );
}

#[test]
fn preserves_surrogate_escaped_string_literals_in_module_constants() {
    let source = "def f():\n    return \"\\udca7\" \"b\"\n";
    let bb_module = tracked_name_binding_module(source);
    let prepared = lower_try_jump_exception_flow(&bb_module);
    let normalized = normalize_bb_module_strings(&prepared);

    assert!(
        module_constants_contain_string(&normalized.module_constants),
        "expected surrogate-escaped string to remain in module constants"
    );
}

use super::normalize_bb_module_strings;
use crate::{
    block_py::{
        BlockPyNameLike, BlockPyStmt, BlockPyTerm, CodegenBlockPyExpr, CodegenBlockPyLiteral,
        CodegenExprOp,
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
        CodegenBlockPyExpr::Name(_) => false,
        CodegenBlockPyExpr::Literal(_) => true,
        CodegenBlockPyExpr::Op(operation) => {
            let mut saw_literal = false;
            operation.walk_args(&mut |arg| {
                if expr_contains_literal(arg) {
                    saw_literal = true;
                }
            });
            saw_literal
        }
    }
}

fn module_constants_contain_string(exprs: &[CodegenBlockPyExpr]) -> bool {
    exprs.iter().any(|expr| {
        matches!(
            expr,
            CodegenBlockPyExpr::Literal(CodegenBlockPyLiteral::StringLiteral(_))
        )
    })
}

fn collect_helper_like_names_in_expr(out: &mut Vec<String>, expr: &CodegenBlockPyExpr) {
    match expr {
        CodegenBlockPyExpr::Name(_) | CodegenBlockPyExpr::Literal(_) => {}
        CodegenBlockPyExpr::Op(operation) => {
            match operation {
                CodegenExprOp::GetAttr(_) => out.push("__dp_getattr".to_string()),
                CodegenExprOp::SetAttr(_) => out.push("__dp_setattr".to_string()),
                CodegenExprOp::GetItem(_) => out.push("__dp_getitem".to_string()),
                CodegenExprOp::SetItem(_) => out.push("__dp_setitem".to_string()),
                CodegenExprOp::Call(call) => {
                    if let CodegenBlockPyExpr::Name(name) = &*call.func {
                        out.push(name.id_str().to_string());
                    }
                }
                _ => {}
            }
            operation.walk_args(&mut |arg| collect_helper_like_names_in_expr(out, arg));
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
                match stmt {
                    BlockPyStmt::Assign(assign) => assert!(
                        !expr_contains_literal(&assign.value),
                        "assign value should not retain executable literals: {:?}",
                        assign.value
                    ),
                    BlockPyStmt::Expr(expr) => assert!(
                        !expr_contains_literal(expr),
                        "expr stmt should not retain executable literals: {expr:?}"
                    ),
                    BlockPyStmt::Delete(_) => {}
                }
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
                match stmt {
                    BlockPyStmt::Assign(assign) => {
                        collect_helper_like_names_in_expr(&mut helper_names, &assign.value);
                    }
                    BlockPyStmt::Expr(expr) => {
                        collect_helper_like_names_in_expr(&mut helper_names, expr);
                    }
                    BlockPyStmt::Delete(_) => {}
                }
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

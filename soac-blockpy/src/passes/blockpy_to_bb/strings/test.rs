use super::normalize_bb_module_strings;
use crate::{
    block_py::{
        BlockPyNameLike, BlockPyStmt, BlockPyTerm, CodegenBlockPyExpr, CodegenBlockPyLiteral,
        LocatedName, OperationDetail,
    },
    lower_python_to_blockpy_for_testing,
    passes::lower_try_jump_exception_flow,
};
use std::cell::Cell;

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

struct ExprShapeProbe {
    saw_make_string: Cell<bool>,
    saw_make_string_bytes: Cell<bool>,
}

impl ExprShapeProbe {
    fn new() -> Self {
        Self {
            saw_make_string: Cell::new(false),
            saw_make_string_bytes: Cell::new(false),
        }
    }
}

fn probe_bb_exprs(probe: &mut ExprShapeProbe, expr: &CodegenBlockPyExpr) {
    match expr {
        CodegenBlockPyExpr::Name(_) => {}
        CodegenBlockPyExpr::Literal(literal) => {
            if matches!(literal, CodegenBlockPyLiteral::BytesLiteral(_)) {
                probe.saw_make_string_bytes.set(true);
            }
        }
        CodegenBlockPyExpr::Op(operation) => {
            if let crate::block_py::OperationDetail::MakeString(op) = operation.detail() {
                probe.saw_make_string.set(true);
                if !op.bytes.is_empty() {
                    probe.saw_make_string_bytes.set(true);
                }
            }
            operation.walk_args(&mut |arg| probe_bb_exprs(probe, arg));
        }
        CodegenBlockPyExpr::Call(call) => {
            probe_bb_exprs(probe, &call.func);
            for arg in &call.args {
                match arg {
                    crate::block_py::CoreBlockPyCallArg::Positional(value)
                    | crate::block_py::CoreBlockPyCallArg::Starred(value) => {
                        probe_bb_exprs(probe, value);
                    }
                }
            }
            for kw in &call.keywords {
                match kw {
                    crate::block_py::CoreBlockPyKeywordArg::Named { value, .. }
                    | crate::block_py::CoreBlockPyKeywordArg::Starred(value) => {
                        probe_bb_exprs(probe, value);
                    }
                }
            }
        }
    }
}

fn collect_helper_like_names_in_expr(out: &mut Vec<String>, expr: &CodegenBlockPyExpr) {
    match expr {
        CodegenBlockPyExpr::Name(_) | CodegenBlockPyExpr::Literal(_) => {}
        CodegenBlockPyExpr::Op(operation) => {
            match operation.detail() {
                OperationDetail::GetAttr(_) => out.push("__dp_getattr".to_string()),
                OperationDetail::SetAttr(_) => out.push("__dp_setattr".to_string()),
                OperationDetail::GetItem(_) => out.push("__dp_getitem".to_string()),
                OperationDetail::SetItem(_) => out.push("__dp_setitem".to_string()),
                _ => {}
            }
            operation.walk_args(&mut |arg| collect_helper_like_names_in_expr(out, arg));
        }
        CodegenBlockPyExpr::Call(call) => {
            if let CodegenBlockPyExpr::Name(name) = &*call.func {
                out.push(name.id_str().to_string());
            }
            collect_helper_like_names_in_expr(out, &call.func);
            for arg in &call.args {
                match arg {
                    crate::block_py::CoreBlockPyCallArg::Positional(value)
                    | crate::block_py::CoreBlockPyCallArg::Starred(value) => {
                        collect_helper_like_names_in_expr(out, value);
                    }
                }
            }
            for kw in &call.keywords {
                match kw {
                    crate::block_py::CoreBlockPyKeywordArg::Named { value, .. }
                    | crate::block_py::CoreBlockPyKeywordArg::Starred(value) => {
                        collect_helper_like_names_in_expr(out, value);
                    }
                }
            }
        }
    }
}

fn probe_bb_term_exprs(probe: &mut ExprShapeProbe, term: &BlockPyTerm<CodegenBlockPyExpr>) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => probe_bb_exprs(probe, &if_term.test),
        BlockPyTerm::BranchTable(branch) => probe_bb_exprs(probe, &branch.index),
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_ref() {
                probe_bb_exprs(probe, exc);
            }
        }
        BlockPyTerm::Return(value) => probe_bb_exprs(probe, value),
    }
}

fn probe_bb_stmt_exprs(
    probe: &mut ExprShapeProbe,
    stmt: &BlockPyStmt<CodegenBlockPyExpr, LocatedName>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => probe_bb_exprs(probe, &assign.value),
        BlockPyStmt::Expr(expr) => probe_bb_exprs(probe, expr),
        BlockPyStmt::Delete(_) => {}
    }
}

#[test]
fn lowers_attributes_and_string_literals_for_codegen() {
    let source = r#"
def f():
    x = __dp_store_global(globals(), "classify", __dp_ret("ok"))
    return x
"#;
    let bb_module = tracked_name_binding_module(source);
    let prepared = lower_try_jump_exception_flow(&bb_module);
    let normalized = normalize_bb_module_strings(&prepared);

    let mut probe = ExprShapeProbe::new();
    for function in normalized.callable_defs {
        for block in &function.blocks {
            for op in &block.body {
                probe_bb_stmt_exprs(&mut probe, &op);
            }
            probe_bb_term_exprs(&mut probe, &block.term);
        }
    }

    assert!(
        probe.saw_make_string.get(),
        "a MakeString operation should be present"
    );
    assert!(
        probe.saw_make_string_bytes.get(),
        "MakeString should carry utf-8 byte payloads"
    );
}

#[test]
fn preserves_structured_intrinsics_for_attr_and_item_helpers() {
    let source = r#"
def f(obj, mapping, key, value):
    a = __dp_getattr(obj, "x")
    __dp_setattr(obj, "x", value)
    b = __dp_getitem(mapping, key)
    __dp_setitem(mapping, key, value)
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
fn lowers_surrogate_escaped_string_literals_for_codegen() {
    let source = "def f():\n    return \"\\udca7\" \"b\"\n";
    let bb_module = tracked_name_binding_module(source);
    let prepared = lower_try_jump_exception_flow(&bb_module);
    let normalized = normalize_bb_module_strings(&prepared);

    let mut probe = ExprShapeProbe::new();
    for function in normalized.callable_defs {
        for block in &function.blocks {
            for op in &block.body {
                probe_bb_stmt_exprs(&mut probe, &op);
            }
            probe_bb_term_exprs(&mut probe, &block.term);
        }
    }

    assert!(probe.saw_make_string.get(), "expected MakeString operation");
}

use super::normalize_bb_module_strings;
use crate::{
    block_py::{BbStmt, BlockPyNameLike, BlockPyTerm, CoreBlockPyExpr},
    passes::lower_try_jump_exception_flow,
    transform_str_to_bb_ir_with_options, Options,
};
use std::cell::Cell;

struct ExprShapeProbe {
    saw_string_literal: Cell<bool>,
    saw_bytes_literal: Cell<bool>,
    saw_str_bytes_call: Cell<bool>,
    saw_decode_literal_call: Cell<bool>,
}

impl ExprShapeProbe {
    fn new() -> Self {
        Self {
            saw_string_literal: Cell::new(false),
            saw_bytes_literal: Cell::new(false),
            saw_str_bytes_call: Cell::new(false),
            saw_decode_literal_call: Cell::new(false),
        }
    }
}

fn probe_bb_exprs<N: BlockPyNameLike>(
    probe: &mut ExprShapeProbe,
    expr: &crate::block_py::CoreBlockPyExpr<N>,
) {
    match expr {
        crate::block_py::CoreBlockPyExpr::Name(_) => {}
        crate::block_py::CoreBlockPyExpr::Literal(literal) => match literal {
            crate::block_py::CoreBlockPyLiteral::StringLiteral(_) => {
                probe.saw_string_literal.set(true);
            }
            crate::block_py::CoreBlockPyLiteral::BytesLiteral(_) => {
                probe.saw_bytes_literal.set(true);
            }
            _ => {}
        },
        crate::block_py::CoreBlockPyExpr::Op(operation) => {
            operation.walk_args(&mut |arg| probe_bb_exprs(probe, arg));
        }
        crate::block_py::CoreBlockPyExpr::Call(call) => {
            if let crate::block_py::CoreBlockPyExpr::Name(name) = call.func.as_ref() {
                if name.id_str() == "str"
                    && call.args.len() == 1
                    && call.keywords.is_empty()
                    && matches!(
                        call.args[0],
                        crate::block_py::CoreBlockPyCallArg::Positional(
                            crate::block_py::CoreBlockPyExpr::Literal(
                                crate::block_py::CoreBlockPyLiteral::BytesLiteral(_)
                            )
                        )
                    )
                {
                    probe.saw_str_bytes_call.set(true);
                }
                if name.id_str() == "__dp_decode_literal_bytes"
                    || name.id_str() == "__dp_decode_literal_source_bytes"
                {
                    probe.saw_decode_literal_call.set(true);
                }
            }
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
        crate::block_py::CoreBlockPyExpr::Intrinsic(call) => {
            for arg in &call.args {
                probe_bb_exprs(probe, arg);
            }
        }
    }
}

fn probe_bb_term_exprs<N: BlockPyNameLike>(
    probe: &mut ExprShapeProbe,
    term: &BlockPyTerm<CoreBlockPyExpr<N>>,
) {
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

fn probe_bb_stmt_exprs<N: BlockPyNameLike>(
    probe: &mut ExprShapeProbe,
    stmt: &BbStmt<CoreBlockPyExpr<N>, N>,
) {
    match stmt {
        BbStmt::Assign(assign) => probe_bb_exprs(probe, &assign.value),
        BbStmt::Expr(expr) => probe_bb_exprs(probe, expr),
        BbStmt::Delete(_) => {}
    }
}

#[test]
fn lowers_attributes_and_string_literals_for_codegen() {
    let source = r#"
def f():
    x = __dp_store_global(globals(), "classify", __dp_ret("ok"))
    return x
"#;
    let options = Options::for_test();
    let bb_module = transform_str_to_bb_ir_with_options(source, options)
        .expect("transform should succeed")
        .expect("bb module should be available");
    let prepared = lower_try_jump_exception_flow(&bb_module).expect("bb lowering should succeed");
    let normalized = normalize_bb_module_strings(&prepared, source);

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
        !probe.saw_string_literal.get(),
        "string literals should be lowered"
    );
    assert!(
        probe.saw_bytes_literal.get(),
        "bytes literals should remain"
    );
    assert!(
        probe.saw_str_bytes_call.get() || probe.saw_decode_literal_call.get(),
        "a lowered string decode call should be present"
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
    let options = Options::for_test();
    let bb_module = transform_str_to_bb_ir_with_options(source, options)
        .expect("transform should succeed")
        .expect("bb module should be available");
    let prepared = lower_try_jump_exception_flow(&bb_module).expect("bb lowering should succeed");
    let normalized = normalize_bb_module_strings(&prepared, source);

    let mut text = String::new();
    for function in normalized.callable_defs {
        for block in &function.blocks {
            text.push_str(&crate::block_py::pretty::bb_stmts_text(&block.body));
        }
    }

    assert!(text.contains("__dp_getattr"), "{text}");
    assert!(text.contains("__dp_setattr"), "{text}");
    assert!(text.contains("__dp_getitem"), "{text}");
    assert!(text.contains("__dp_setitem"), "{text}");
    assert!(!text.contains("PyObject_GetAttr"), "{text}");
    assert!(!text.contains("PyObject_SetAttr"), "{text}");
    assert!(!text.contains("PyObject_GetItem"), "{text}");
    assert!(!text.contains("PyObject_SetItem"), "{text}");
}

#[test]
fn lowers_surrogate_escaped_string_literals_for_codegen() {
    let source = "def f():\n    return \"\\udca7\" \"b\"\n";
    let options = Options::for_test();
    let bb_module = transform_str_to_bb_ir_with_options(source, options)
        .expect("transform should succeed")
        .expect("bb module should be available");
    let prepared = lower_try_jump_exception_flow(&bb_module).expect("bb lowering should succeed");
    let normalized = normalize_bb_module_strings(&prepared, source);

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
        probe.saw_decode_literal_call.get(),
        "expected surrogate decode call"
    );
    assert!(
        !probe.saw_string_literal.get(),
        "string literal should have been lowered"
    );
}

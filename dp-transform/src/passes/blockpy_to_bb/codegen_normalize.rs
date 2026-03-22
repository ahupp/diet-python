use crate::block_py::{
    BbStmt, BlockPyModule, BlockPyRaise, BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg,
    CoreBlockPyExpr, CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreBytesLiteral, IntrinsicCall,
};
use crate::passes::trace::{instrument_bb_module_for_trace, parse_trace_env};
use crate::passes::PreparedBbBlockPyPass;
use ruff_python_ast::{self as ast, ExprName};
use ruff_text_size::TextRange;

pub fn normalize_bb_module_for_codegen(
    module: &BlockPyModule<PreparedBbBlockPyPass>,
) -> BlockPyModule<PreparedBbBlockPyPass> {
    let mut normalized = module.clone();
    if let Some(config) = parse_trace_env() {
        instrument_bb_module_for_trace(&mut normalized, &config);
    }
    let mut rewriter = CodegenExprNormalizer;
    for function in &mut normalized.callable_defs {
        for block in &mut function.blocks {
            for op in &mut block.body {
                match op {
                    BbStmt::Assign(assign) => rewrite_bb_expr(&mut rewriter, &mut assign.value),
                    BbStmt::Expr(expr) => rewrite_bb_expr(&mut rewriter, expr),
                    BbStmt::Delete(_) => {}
                }
            }
            rewrite_term_exprs(&mut rewriter, &mut block.term);
        }
    }
    normalized
}

fn rewrite_term_exprs(
    rewriter: &mut CodegenExprNormalizer,
    term: &mut BlockPyTerm<CoreBlockPyExpr>,
) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => rewrite_bb_expr(rewriter, &mut if_term.test),
        BlockPyTerm::BranchTable(branch) => rewrite_bb_expr(rewriter, &mut branch.index),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc.as_mut() {
                rewrite_bb_expr(rewriter, exc);
            }
        }
        BlockPyTerm::Return(value) => rewrite_bb_expr(rewriter, value),
    }
}

fn rewrite_bb_expr(rewriter: &mut CodegenExprNormalizer, expr: &mut CoreBlockPyExpr) {
    rewriter.rewrite_expr(expr);
}

struct CodegenExprNormalizer;

impl CodegenExprNormalizer {
    fn rewrite_expr(&mut self, expr: &mut CoreBlockPyExpr) {
        match expr {
            CoreBlockPyExpr::Call(call) => {
                self.rewrite_expr(call.func.as_mut());
                rewrite_call_parts(self, &mut call.args, &mut call.keywords);
            }
            CoreBlockPyExpr::Intrinsic(IntrinsicCall { args, keywords, .. }) => {
                rewrite_call_parts(self, args, keywords);
            }
            CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => {}
        }

        match expr {
            CoreBlockPyExpr::Call(call)
                if call.keywords.is_empty()
                    && matches!(call.func.as_ref(), CoreBlockPyExpr::Name(_)) =>
            {
                let func_name = match call.func.as_ref() {
                    CoreBlockPyExpr::Name(name) => name.id.as_str(),
                    _ => unreachable!(),
                };
                let args = call.args.clone();
                let call_meta = (call.node_index.clone(), call.range);
                let replacement = match (func_name, args.as_slice()) {
                    (
                        "__dp_getattr",
                        [CoreBlockPyCallArg::Positional(obj), CoreBlockPyCallArg::Positional(attr)],
                    ) => Some(helper_call_expr_with_meta(
                        "PyObject_GetAttr",
                        vec![obj.clone(), attr.clone()],
                        call_meta,
                    )),
                    (
                        "__dp_setattr",
                        [CoreBlockPyCallArg::Positional(obj), CoreBlockPyCallArg::Positional(attr), CoreBlockPyCallArg::Positional(value)],
                    ) => Some(helper_call_expr_with_meta(
                        "PyObject_SetAttr",
                        vec![obj.clone(), attr.clone(), value.clone()],
                        call_meta,
                    )),
                    (
                        "__dp_getitem",
                        [CoreBlockPyCallArg::Positional(obj), CoreBlockPyCallArg::Positional(key)],
                    ) => Some(helper_call_expr_with_meta(
                        "PyObject_GetItem",
                        vec![obj.clone(), key.clone()],
                        call_meta,
                    )),
                    (
                        "__dp_setitem",
                        [CoreBlockPyCallArg::Positional(obj), CoreBlockPyCallArg::Positional(key), CoreBlockPyCallArg::Positional(value)],
                    ) => Some(helper_call_expr_with_meta(
                        "PyObject_SetItem",
                        vec![obj.clone(), key.clone(), value.clone()],
                        call_meta,
                    )),
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    *expr = replacement;
                }
            }
            CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(node)) => {
                *expr = str_bytes_call_expr(node.value.as_bytes());
            }
            _ => {}
        }
    }
}

fn rewrite_call_parts(
    rewriter: &mut CodegenExprNormalizer,
    args: &mut [CoreBlockPyCallArg<CoreBlockPyExpr>],
    keywords: &mut [CoreBlockPyKeywordArg<CoreBlockPyExpr>],
) {
    for arg in args {
        match arg {
            CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                rewriter.rewrite_expr(value);
            }
        }
    }
    for keyword in keywords {
        match keyword {
            CoreBlockPyKeywordArg::Named { value, .. } | CoreBlockPyKeywordArg::Starred(value) => {
                rewriter.rewrite_expr(value)
            }
        }
    }
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

fn load_name(id: &str) -> ExprName {
    ExprName {
        id: id.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    }
}

fn bytes_literal_expr(bytes: &[u8]) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Literal(CoreBlockPyLiteral::BytesLiteral(CoreBytesLiteral {
        range: compat_range(),
        node_index: compat_node_index(),
        value: bytes.to_vec(),
    }))
}

fn helper_call_expr_with_meta(
    helper_name: &str,
    args: Vec<CoreBlockPyExpr>,
    (node_index, range): (ast::AtomicNodeIndex, TextRange),
) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Call(CoreBlockPyCall {
        node_index,
        range,
        func: Box::new(CoreBlockPyExpr::Name(load_name(helper_name))),
        args: args
            .into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        keywords: Vec::<CoreBlockPyKeywordArg<CoreBlockPyExpr>>::new(),
    })
}

fn helper_call_expr(helper_name: &str, args: Vec<CoreBlockPyExpr>) -> CoreBlockPyExpr {
    helper_call_expr_with_meta(helper_name, args, (compat_node_index(), compat_range()))
}

fn str_bytes_call_expr(bytes: &[u8]) -> CoreBlockPyExpr {
    helper_call_expr("str", vec![bytes_literal_expr(bytes)])
}

#[cfg(test)]
mod tests {
    use super::normalize_bb_module_for_codegen;
    use crate::{
        block_py::{BbStmt, BlockPyTerm, CoreBlockPyExpr},
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

    fn probe_bb_exprs(probe: &mut ExprShapeProbe, expr: &crate::block_py::CoreBlockPyExpr) {
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
            crate::block_py::CoreBlockPyExpr::Call(call) => {
                if let crate::block_py::CoreBlockPyExpr::Name(name) = call.func.as_ref() {
                    if name.id.as_str() == "str"
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
                    if name.id.as_str() == "__dp_decode_literal_bytes" {
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

    fn probe_bb_term_exprs(probe: &mut ExprShapeProbe, term: &BlockPyTerm<CoreBlockPyExpr>) {
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

    fn probe_bb_stmt_exprs(probe: &mut ExprShapeProbe, stmt: &BbStmt) {
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
        let prepared =
            lower_try_jump_exception_flow(&bb_module).expect("bb lowering should succeed");
        let normalized = normalize_bb_module_for_codegen(&prepared);

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
    fn rewrites_intrinsics_to_python_capi_names() {
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
        let prepared =
            lower_try_jump_exception_flow(&bb_module).expect("bb lowering should succeed");
        let normalized = normalize_bb_module_for_codegen(&prepared);

        let mut text = String::new();
        for function in normalized.callable_defs {
            for block in &function.blocks {
                text.push_str(&crate::block_py::pretty::bb_stmts_text(&block.body));
            }
        }

        assert!(text.contains("PyObject_GetAttr"), "{text}");
        assert!(text.contains("PyObject_SetAttr"), "{text}");
        assert!(text.contains("PyObject_GetItem"), "{text}");
        assert!(text.contains("PyObject_SetItem"), "{text}");
        assert!(!text.contains("__dp_getattr"), "{text}");
        assert!(!text.contains("__dp_setattr"), "{text}");
        assert!(!text.contains("__dp_getitem"), "{text}");
        assert!(!text.contains("__dp_setitem"), "{text}");
    }
}

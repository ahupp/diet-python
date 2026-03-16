use crate::basic_block::bb_ir;
use crate::basic_block::block_py::{
    BlockPyStmt, CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExprWithoutAwaitOrYield,
    CoreBlockPyKeywordArg, CoreBlockPyLiteral,
};
use ruff_python_ast::str::Quote;
use ruff_python_ast::{self as ast, BytesLiteral, BytesLiteralFlags, ExprName};
use ruff_text_size::TextRange;

use super::codegen_trace::{instrument_bb_module_for_trace, parse_bb_trace_env};

pub fn normalize_bb_module_for_codegen(module: &bb_ir::BbModule) -> bb_ir::BbModule {
    let mut normalized = module.clone();
    if let Some(config) = parse_bb_trace_env() {
        instrument_bb_module_for_trace(&mut normalized, &config);
    }
    let mut rewriter = CodegenExprNormalizer;
    for function in &mut normalized.callable_defs {
        for block in &mut function.blocks {
            for op in &mut block.body {
                match op {
                    BlockPyStmt::Assign(assign) => {
                        rewrite_bb_expr(&mut rewriter, &mut assign.value)
                    }
                    BlockPyStmt::Expr(expr) => rewrite_bb_expr(&mut rewriter, expr),
                    BlockPyStmt::Delete(_) => {}
                    BlockPyStmt::If(_) => panic!("structured BlockPy If is not allowed in BbBlock"),
                }
            }
            rewrite_term_exprs(&mut rewriter, &mut block.term);
        }
    }
    normalized
}

fn rewrite_term_exprs(rewriter: &mut CodegenExprNormalizer, term: &mut bb_ir::BbTerm) {
    match term {
        bb_ir::BbTerm::Jump(_) => {}
        bb_ir::BbTerm::BrIf { test, .. } => rewrite_bb_expr(rewriter, test),
        bb_ir::BbTerm::BrTable { index, .. } => rewrite_bb_expr(rewriter, index),
        bb_ir::BbTerm::Raise { exc, cause } => {
            if let Some(exc) = exc.as_mut() {
                rewrite_bb_expr(rewriter, exc);
            }
            if let Some(cause) = cause.as_mut() {
                rewrite_bb_expr(rewriter, cause);
            }
        }
        bb_ir::BbTerm::Ret(value) => {
            if let Some(value) = value.as_mut() {
                rewrite_bb_expr(rewriter, value);
            }
        }
    }
}

fn rewrite_bb_expr(
    rewriter: &mut CodegenExprNormalizer,
    expr: &mut CoreBlockPyExprWithoutAwaitOrYield,
) {
    rewriter.rewrite_expr(expr);
}

struct CodegenExprNormalizer;

impl CodegenExprNormalizer {
    fn rewrite_expr(&mut self, expr: &mut CoreBlockPyExprWithoutAwaitOrYield) {
        if let CoreBlockPyExprWithoutAwaitOrYield::Call(call) = expr {
            self.rewrite_expr(call.func.as_mut());
            for arg in &mut call.args {
                match arg {
                    CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                        self.rewrite_expr(value);
                    }
                }
            }
            for keyword in &mut call.keywords {
                match keyword {
                    CoreBlockPyKeywordArg::Named { value, .. }
                    | CoreBlockPyKeywordArg::Starred(value) => self.rewrite_expr(value),
                }
            }
        }

        match expr {
            CoreBlockPyExprWithoutAwaitOrYield::Call(call)
                if call.keywords.is_empty()
                    && matches!(
                        call.func.as_ref(),
                        CoreBlockPyExprWithoutAwaitOrYield::Name(_)
                    ) =>
            {
                let func_name = match call.func.as_ref() {
                    CoreBlockPyExprWithoutAwaitOrYield::Name(name) => name.id.as_str(),
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
            CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::StringLiteral(
                node,
            )) => {
                *expr = str_bytes_call_expr(node.value.to_str().as_bytes());
            }
            CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::BooleanLiteral(
                boolean,
            )) => {
                *expr = if boolean.value {
                    helper_name_expr("__dp_TRUE")
                } else {
                    helper_name_expr("__dp_FALSE")
                };
            }
            CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::NoneLiteral(_)) => {
                *expr = helper_name_expr("__dp_NONE");
            }
            CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::EllipsisLiteral(_)) => {
                *expr = helper_name_expr("__dp_Ellipsis");
            }
            _ => {}
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

fn bytes_literal_expr(bytes: &[u8]) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::BytesLiteral(
        ast::ExprBytesLiteral {
            range: compat_range(),
            node_index: compat_node_index(),
            value: ast::BytesLiteralValue::single(BytesLiteral {
                range: compat_range(),
                node_index: compat_node_index(),
                value: bytes.into(),
                flags: BytesLiteralFlags::empty().with_quote_style(Quote::Double),
            }),
        },
    ))
}

fn helper_call_expr_with_meta(
    helper_name: &str,
    args: Vec<CoreBlockPyExprWithoutAwaitOrYield>,
    (node_index, range): (ast::AtomicNodeIndex, TextRange),
) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Call(CoreBlockPyCall {
        node_index,
        range,
        func: Box::new(CoreBlockPyExprWithoutAwaitOrYield::Name(load_name(
            helper_name,
        ))),
        args: args
            .into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        keywords: Vec::<CoreBlockPyKeywordArg<CoreBlockPyExprWithoutAwaitOrYield>>::new(),
    })
}

fn helper_call_expr(
    helper_name: &str,
    args: Vec<CoreBlockPyExprWithoutAwaitOrYield>,
) -> CoreBlockPyExprWithoutAwaitOrYield {
    helper_call_expr_with_meta(helper_name, args, (compat_node_index(), compat_range()))
}

fn str_bytes_call_expr(bytes: &[u8]) -> CoreBlockPyExprWithoutAwaitOrYield {
    helper_call_expr("str", vec![bytes_literal_expr(bytes)])
}

fn helper_name_expr(name: &str) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Name(load_name(name))
}

#[cfg(test)]
mod tests {
    use super::normalize_bb_module_for_codegen;
    use crate::{
        basic_block::{bb_ir, block_py::BlockPyStmt},
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

    fn probe_bb_exprs(
        probe: &mut ExprShapeProbe,
        expr: &crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield,
    ) {
        match expr {
            crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield::Name(_) => {}
            crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield::Literal(literal) => {
                match literal {
                    crate::basic_block::block_py::CoreBlockPyLiteral::StringLiteral(_) => {
                        probe.saw_string_literal.set(true);
                    }
                    crate::basic_block::block_py::CoreBlockPyLiteral::BytesLiteral(_) => {
                        probe.saw_bytes_literal.set(true);
                    }
                    _ => {}
                }
            }
            crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield::Call(call) => {
                if let crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield::Name(
                    name,
                ) = call.func.as_ref()
                {
                    if name.id.as_str() == "str"
                        && call.args.len() == 1
                        && call.keywords.is_empty()
                        && matches!(
                            call.args[0],
                            crate::basic_block::block_py::CoreBlockPyCallArg::Positional(
                                crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield::Literal(
                                    crate::basic_block::block_py::CoreBlockPyLiteral::BytesLiteral(_)
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
                        crate::basic_block::block_py::CoreBlockPyCallArg::Positional(value)
                        | crate::basic_block::block_py::CoreBlockPyCallArg::Starred(value) => {
                            probe_bb_exprs(probe, value);
                        }
                    }
                }
                for kw in &call.keywords {
                    match kw {
                        crate::basic_block::block_py::CoreBlockPyKeywordArg::Named {
                            value,
                            ..
                        }
                        | crate::basic_block::block_py::CoreBlockPyKeywordArg::Starred(value) => {
                            probe_bb_exprs(probe, value);
                        }
                    }
                }
            }
        }
    }

    fn probe_bb_term_exprs(probe: &mut ExprShapeProbe, term: &bb_ir::BbTerm) {
        match term {
            bb_ir::BbTerm::Jump(_) => {}
            bb_ir::BbTerm::BrIf { test, .. } => probe_bb_exprs(probe, test),
            bb_ir::BbTerm::BrTable { index, .. } => probe_bb_exprs(probe, index),
            bb_ir::BbTerm::Raise { exc, cause } => {
                if let Some(exc) = exc {
                    probe_bb_exprs(probe, exc);
                }
                if let Some(cause) = cause {
                    probe_bb_exprs(probe, cause);
                }
            }
            bb_ir::BbTerm::Ret(value) => {
                if let Some(value) = value {
                    probe_bb_exprs(probe, value);
                }
            }
        }
    }

    fn probe_bb_stmt_exprs(probe: &mut ExprShapeProbe, stmt: &bb_ir::BbStmt) {
        match stmt {
            BlockPyStmt::Assign(assign) => probe_bb_exprs(probe, &assign.value),
            BlockPyStmt::Expr(expr) => probe_bb_exprs(probe, expr),
            BlockPyStmt::Delete(_) => {}
            BlockPyStmt::If(_) => panic!("structured BlockPy If is not allowed in BbBlock"),
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
        let normalized = normalize_bb_module_for_codegen(&bb_module);

        let mut probe = ExprShapeProbe::new();
        for function in normalized.callable_defs {
            for block in function.cfg.blocks {
                for op in block.body {
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
        let normalized = normalize_bb_module_for_codegen(&bb_module);

        let mut text = String::new();
        for function in normalized.callable_defs {
            for block in function.cfg.blocks {
                text.push_str(&bb_ir::bb_stmts_text(&block.body));
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

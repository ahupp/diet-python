use crate::basic_block::bb_ir;
use crate::basic_block::codegen_trace::{instrument_bb_module_for_trace, parse_bb_trace_env};
use crate::py_expr;
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr, ExprContext};
use ruff_python_parser::parse_expression;

pub fn normalize_bb_module_for_codegen(module: &bb_ir::BbModule) -> bb_ir::BbModule {
    let mut normalized = module.clone();
    if let Some(config) = parse_bb_trace_env() {
        instrument_bb_module_for_trace(&mut normalized, &config);
    }
    let mut rewriter = CodegenExprNormalizer;
    for function in &mut normalized.functions {
        for block in &mut function.blocks {
            for op in &mut block.ops {
                match op {
                    bb_ir::BbOp::Assign(assign) => {
                        rewrite_bb_expr(&mut rewriter, &mut assign.value)
                    }
                    bb_ir::BbOp::Expr(expr) => rewrite_bb_expr(&mut rewriter, &mut expr.value),
                    bb_ir::BbOp::Delete(delete) => {
                        for target in &mut delete.targets {
                            rewrite_bb_expr(&mut rewriter, target);
                        }
                    }
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
        bb_ir::BbTerm::TryJump { .. } => {}
        bb_ir::BbTerm::Ret(value) => {
            if let Some(value) = value.as_mut() {
                rewrite_bb_expr(rewriter, value);
            }
        }
    }
}

fn rewrite_bb_expr(rewriter: &mut CodegenExprNormalizer, expr: &mut bb_ir::BbExpr) {
    let mut raw = expr.to_expr();
    rewriter.visit_expr(&mut raw);
    *expr = bb_ir::BbExpr::from_expr(raw);
}

struct CodegenExprNormalizer;

impl Transformer for CodegenExprNormalizer {
    fn visit_expr(&mut self, expr: &mut Expr) {
        walk_expr(self, expr);
        match expr {
            Expr::Call(call)
                if call.arguments.keywords.is_empty()
                    && matches!(call.func.as_ref(), Expr::Name(_)) =>
            {
                let func_name = if let Expr::Name(name) = call.func.as_ref() {
                    name.id.as_str()
                } else {
                    ""
                };
                let args = call.arguments.args.clone();
                match (func_name, args.len()) {
                    ("__dp_getattr", 2) => {
                        *expr = py_expr!(
                            "PyObject_GetAttr({obj:expr}, {attr:expr})",
                            obj = args[0].clone(),
                            attr = args[1].clone(),
                        );
                    }
                    ("__dp_setattr", 3) => {
                        *expr = py_expr!(
                            "PyObject_SetAttr({obj:expr}, {attr:expr}, {value:expr})",
                            obj = args[0].clone(),
                            attr = args[1].clone(),
                            value = args[2].clone(),
                        );
                    }
                    ("__dp_getitem", 2) => {
                        *expr = py_expr!(
                            "PyObject_GetItem({obj:expr}, {key:expr})",
                            obj = args[0].clone(),
                            key = args[1].clone(),
                        );
                    }
                    ("__dp_setitem", 3) => {
                        *expr = py_expr!(
                            "PyObject_SetItem({obj:expr}, {key:expr}, {value:expr})",
                            obj = args[0].clone(),
                            key = args[1].clone(),
                            value = args[2].clone(),
                        );
                    }
                    _ => {}
                }
            }
            Expr::Attribute(ast::ExprAttribute {
                value, attr, ctx, ..
            }) if matches!(ctx, ExprContext::Load) => {
                let value_expr = *value.clone();
                *expr = py_expr!(
                    "PyObject_GetAttr({value:expr}, {attr:literal})",
                    value = value_expr,
                    attr = attr.to_string().as_str(),
                );
            }
            Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
                *expr = string_to_str_bytes_expr(value.to_string().as_str());
            }
            _ => {}
        }
    }
}

fn string_to_str_bytes_expr(value: &str) -> Expr {
    let mut source = String::from("str(b\"");
    source.push_str(&escape_bytes_for_double_quoted_literal(value.as_bytes()));
    source.push_str("\")");
    let parsed = parse_expression(&source).unwrap_or_else(|err| {
        panic!("failed to build codegen string literal expression from {source:?}: {err}")
    });
    *parsed.into_syntax().body
}

fn escape_bytes_for_double_quoted_literal(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 4);
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\x{:02x}", byte)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::normalize_bb_module_for_codegen;
    use crate::{
        basic_block::bb_ir, transform_str_to_bb_ir_with_options, transformer::Transformer, Options,
    };
    use ruff_python_ast::{Expr, Stmt};
    use std::cell::Cell;

    struct ExprShapeProbe {
        saw_attribute: Cell<bool>,
        saw_string_literal: Cell<bool>,
        saw_bytes_literal: Cell<bool>,
        saw_str_bytes_call: Cell<bool>,
        saw_decode_literal_call: Cell<bool>,
    }

    impl ExprShapeProbe {
        fn new() -> Self {
            Self {
                saw_attribute: Cell::new(false),
                saw_string_literal: Cell::new(false),
                saw_bytes_literal: Cell::new(false),
                saw_str_bytes_call: Cell::new(false),
                saw_decode_literal_call: Cell::new(false),
            }
        }
    }

    impl Transformer for ExprShapeProbe {
        fn visit_expr(&mut self, expr: &mut Expr) {
            match expr {
                Expr::Attribute(_) => self.saw_attribute.set(true),
                Expr::StringLiteral(_) => self.saw_string_literal.set(true),
                Expr::BytesLiteral(_) => self.saw_bytes_literal.set(true),
                Expr::Call(call) => {
                    if call.arguments.keywords.is_empty()
                        && call.arguments.args.len() == 1
                        && matches!(call.func.as_ref(), Expr::Name(name) if name.id.as_str() == "str")
                        && matches!(call.arguments.args[0], Expr::BytesLiteral(_))
                    {
                        self.saw_str_bytes_call.set(true);
                    }
                    if call.arguments.keywords.is_empty()
                        && call.arguments.args.len() == 1
                        && matches!(call.func.as_ref(), Expr::Name(name) if name.id.as_str() == "__dp_decode_literal_bytes")
                    {
                        self.saw_decode_literal_call.set(true);
                    }
                }
                _ => {}
            }
            crate::transformer::walk_expr(self, expr);
        }
    }

    fn probe_stmt_exprs(probe: &mut ExprShapeProbe, stmt: &mut Stmt) {
        probe.visit_stmt(stmt);
    }

    #[test]
    fn lowers_attributes_and_string_literals_for_codegen() {
        let source = r#"
def f():
    x = __dp_store_global(globals(), "classify", __dp_ret("ok"))
    return x
"#;
        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let normalized = normalize_bb_module_for_codegen(&bb_module);

        let mut probe = ExprShapeProbe::new();
        for function in normalized.functions {
            for mut block in function.blocks {
                for op in block.ops {
                    let mut stmt = op.to_stmt();
                    probe_stmt_exprs(&mut probe, &mut stmt);
                }
                match &mut block.term {
                    crate::basic_block::bb_ir::BbTerm::BrIf { test, .. } => {
                        let mut expr = test.to_expr();
                        probe.visit_expr(&mut expr);
                    }
                    crate::basic_block::bb_ir::BbTerm::BrTable { index, .. } => {
                        let mut expr = index.to_expr();
                        probe.visit_expr(&mut expr);
                    }
                    crate::basic_block::bb_ir::BbTerm::Raise { exc, cause } => {
                        if let Some(exc) = exc.as_mut() {
                            let mut expr = exc.to_expr();
                            probe.visit_expr(&mut expr);
                        }
                        if let Some(cause) = cause.as_mut() {
                            let mut expr = cause.to_expr();
                            probe.visit_expr(&mut expr);
                        }
                    }
                    crate::basic_block::bb_ir::BbTerm::Ret(value) => {
                        if let Some(value) = value.as_mut() {
                            let mut expr = value.to_expr();
                            probe.visit_expr(&mut expr);
                        }
                    }
                    crate::basic_block::bb_ir::BbTerm::Jump(_)
                    | crate::basic_block::bb_ir::BbTerm::TryJump { .. } => {}
                }
            }
        }

        assert!(!probe.saw_attribute.get(), "attributes should be lowered");
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
        let options = Options {
            inject_import: false,
            ..Options::for_test()
        };
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let normalized = normalize_bb_module_for_codegen(&bb_module);

        let mut text = String::new();
        for function in normalized.functions {
            for block in function.blocks {
                text.push_str(&crate::ruff_ast_to_string(&bb_ir::bb_ops_to_stmts(
                    &block.ops,
                )));
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

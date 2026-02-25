use crate::basic_block::bb_ir;
use crate::py_expr;
use crate::transformer::{Transformer, walk_expr};
use ruff_python_ast::{self as ast, Expr, ExprContext};
use ruff_python_parser::parse_expression;

pub fn normalize_bb_module_for_codegen(module: &bb_ir::BbModule) -> bb_ir::BbModule {
    let mut normalized = module.clone();
    let mut rewriter = CodegenExprNormalizer;
    for function in &mut normalized.functions {
        for block in &mut function.blocks {
            let mut rewritten_ops = Vec::with_capacity(block.ops.len());
            for op in &block.ops {
                let mut stmt = op.to_stmt();
                rewriter.visit_stmt(&mut stmt);
                if let Some(op) = bb_ir::BbOp::from_stmt(stmt) {
                    rewritten_ops.push(op);
                }
            }
            block.ops = rewritten_ops;
            rewrite_term_exprs(&mut rewriter, &mut block.term);
        }
    }
    normalized
}

fn rewrite_term_exprs(rewriter: &mut CodegenExprNormalizer, term: &mut bb_ir::BbTerm) {
    match term {
        bb_ir::BbTerm::Jump(_) => {}
        bb_ir::BbTerm::BrIf { test, .. } => rewriter.visit_expr(test),
        bb_ir::BbTerm::BrTable { index, .. } => rewriter.visit_expr(index),
        bb_ir::BbTerm::Raise { exc, cause } => {
            if let Some(exc) = exc.as_mut() {
                rewriter.visit_expr(exc);
            }
            if let Some(cause) = cause.as_mut() {
                rewriter.visit_expr(cause);
            }
        }
        bb_ir::BbTerm::TryJump { .. } => {}
        bb_ir::BbTerm::Ret(value) => {
            if let Some(value) = value.as_mut() {
                rewriter.visit_expr(value);
            }
        }
    }
}

struct CodegenExprNormalizer;

impl Transformer for CodegenExprNormalizer {
    fn visit_expr(&mut self, expr: &mut Expr) {
        walk_expr(self, expr);
        match expr {
            Expr::Attribute(ast::ExprAttribute {
                value, attr, ctx, ..
            }) if matches!(ctx, ExprContext::Load) => {
                let value_expr = *value.clone();
                let attr_name = attr.to_string();
                let attr_expr = string_to_str_bytes_expr(attr_name.as_str());
                *expr = py_expr!(
                    "__dp_getattr({value:expr}, {attr:expr})",
                    value = value_expr,
                    attr = attr_expr,
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
    use crate::{Options, transform_str_to_bb_ir_with_options, transformer::Transformer};
    use ruff_python_ast::{Expr, Stmt};
    use std::cell::Cell;

    struct ExprShapeProbe {
        saw_attribute: Cell<bool>,
        saw_string_literal: Cell<bool>,
        saw_bytes_literal: Cell<bool>,
        saw_str_bytes_call: Cell<bool>,
    }

    impl ExprShapeProbe {
        fn new() -> Self {
            Self {
                saw_attribute: Cell::new(false),
                saw_string_literal: Cell::new(false),
                saw_bytes_literal: Cell::new(false),
                saw_str_bytes_call: Cell::new(false),
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
    x = __dp__.store_global(globals(), "classify", __dp__.ret("ok"))
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
                    crate::basic_block::bb_ir::BbTerm::BrIf { test, .. } => probe.visit_expr(test),
                    crate::basic_block::bb_ir::BbTerm::BrTable { index, .. } => {
                        probe.visit_expr(index)
                    }
                    crate::basic_block::bb_ir::BbTerm::Raise { exc, cause } => {
                        if let Some(exc) = exc.as_mut() {
                            probe.visit_expr(exc);
                        }
                        if let Some(cause) = cause.as_mut() {
                            probe.visit_expr(cause);
                        }
                    }
                    crate::basic_block::bb_ir::BbTerm::Ret(value) => {
                        if let Some(value) = value.as_mut() {
                            probe.visit_expr(value);
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
        assert!(probe.saw_bytes_literal.get(), "bytes literals should remain");
        assert!(
            probe.saw_str_bytes_call.get(),
            "str(b\"...\") calls should be present"
        );
    }
}

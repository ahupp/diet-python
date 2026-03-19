use crate::{
    basic_block::ast_to_ast::body::{suite_mut, suite_ref, Suite},
    basic_block::ast_to_ast::context::Context,
    basic_block::ast_to_ast::rewrite_expr::string,
    transformer::{walk_expr, Transformer},
};
use ruff_python_ast::{self as ast, Expr, Stmt};

pub(crate) struct Flattener;

impl Flattener {
    fn visit_body(&mut self, body: &mut Suite) {
        let mut i = 0;
        while i < body.len() {
            self.visit_stmt(&mut body[i]);
            if let Stmt::If(ast::StmtIf {
                test,
                body: inner,
                elif_else_clauses,
                ..
            }) = &mut body[i]
            {
                if elif_else_clauses.is_empty()
                    && matches!(
                        test.as_ref(),
                        Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })
                    )
                {
                    let replacement = std::mem::take(inner);
                    body.splice(i..=i, replacement);
                    continue;
                }
            }
            i += 1;
        }
    }
}

fn remove_placeholder_pass(body: &mut Suite) {
    if body.len() == 1 {
        if let Stmt::Pass(ast::StmtPass { range, .. }) = &body[0] {
            if range.is_empty() {
                body.clear();
            }
        }
    }
}

impl Transformer for Flattener {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::If(ast::StmtIf {
                body,
                elif_else_clauses,
                ..
            }) => {
                self.visit_body(suite_mut(body));
                remove_placeholder_pass(suite_mut(body));
                for clause in elif_else_clauses.iter_mut() {
                    self.visit_body(suite_mut(&mut clause.body));
                    remove_placeholder_pass(suite_mut(&mut clause.body));
                }
            }
            Stmt::For(ast::StmtFor {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_body(suite_mut(inner));
                remove_placeholder_pass(suite_mut(inner));
                self.visit_body(suite_mut(orelse));
                remove_placeholder_pass(suite_mut(orelse));
            }
            Stmt::While(ast::StmtWhile {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_body(suite_mut(inner));
                remove_placeholder_pass(suite_mut(inner));
                self.visit_body(suite_mut(orelse));
                remove_placeholder_pass(suite_mut(orelse));
            }
            Stmt::Try(ast::StmtTry {
                body: inner,
                handlers,
                orelse,
                finalbody,
                ..
            }) => {
                self.visit_body(suite_mut(inner));
                remove_placeholder_pass(suite_mut(inner));
                for handler in handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                        body,
                        ..
                    }) = handler;
                    self.visit_body(suite_mut(body));
                    remove_placeholder_pass(suite_mut(body));
                }
                self.visit_body(suite_mut(orelse));
                remove_placeholder_pass(suite_mut(orelse));
                self.visit_body(suite_mut(finalbody));
                remove_placeholder_pass(suite_mut(finalbody));
            }
            Stmt::FunctionDef(ast::StmtFunctionDef { body: inner, .. }) => {
                self.visit_body(suite_mut(inner));
                remove_placeholder_pass(suite_mut(inner));
            }
            _ => {}
        }
    }
}

pub fn flatten(stmts: &mut Suite) {
    let mut flattener = Flattener;
    (&mut flattener).visit_body(stmts);
}

fn is_docstring_stmt(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    )
}

struct SurrogateStringLiteralLowerer<'a> {
    context: &'a Context,
}

fn decoded_literal_interpolation(
    range: ruff_text_size::TextRange,
    node_index: ast::AtomicNodeIndex,
    source: &str,
) -> ast::InterpolatedStringElement {
    ast::InterpolatedStringElement::Interpolation(ast::InterpolatedElement {
        range,
        node_index,
        expression: Box::new(string::decode_literal_source_bytes_expr(source)),
        debug_text: None,
        conversion: ast::ConversionFlag::None,
        format_spec: None,
    })
}

fn merged_string_literal_expr(node: &ast::ExprStringLiteral) -> ast::ExprStringLiteral {
    if !node.value.is_implicit_concatenated() {
        return node.clone();
    }
    ast::ExprStringLiteral {
        range: node.range,
        node_index: node.node_index.clone(),
        value: ast::StringLiteralValue::single(ast::StringLiteral {
            range: node.range,
            node_index: node.node_index.clone(),
            value: node.value.to_str().into(),
            flags: node.value.first_literal_flags(),
        }),
    }
}

fn merged_bytes_literal_expr(node: &ast::ExprBytesLiteral) -> ast::ExprBytesLiteral {
    if !node.value.is_implicit_concatenated() {
        return node.clone();
    }
    let flags = node
        .value
        .iter()
        .next()
        .expect("bytes literal should have at least one part")
        .flags;
    ast::ExprBytesLiteral {
        range: node.range,
        node_index: node.node_index.clone(),
        value: ast::BytesLiteralValue::single(ast::BytesLiteral {
            range: node.range,
            node_index: node.node_index.clone(),
            value: node.value.bytes().collect::<Vec<_>>().into_boxed_slice(),
            flags,
        }),
    }
}

fn materialize_nested_fstring_sources(fstring: &mut ast::FString, context: &Context) {
    let is_raw = fstring.flags.prefix().is_raw();
    for element in fstring.elements.iter_mut() {
        match element {
            ast::InterpolatedStringElement::Literal(lit) => {
                if is_raw {
                    continue;
                }
                let Some(src) = context.source_slice(lit.range) else {
                    continue;
                };
                if !string::has_surrogate_escape(src) {
                    continue;
                }
                let quoted = string::quote_fstring_literal(src);
                *element =
                    decoded_literal_interpolation(lit.range, lit.node_index.clone(), &quoted);
            }
            ast::InterpolatedStringElement::Interpolation(_) => {}
        }
    }
}

fn materialize_fstring_sources(node: &mut ast::ExprFString, context: &Context) {
    for part in node.value.iter_mut() {
        match part {
            ast::FStringPart::Literal(lit) => {
                if lit.flags.prefix().is_raw() {
                    continue;
                }
                let Some(src) = context.source_slice(lit.range) else {
                    continue;
                };
                if !string::has_surrogate_escape(src) {
                    continue;
                }
                *part = ast::FStringPart::FString(ast::FString {
                    range: lit.range,
                    node_index: lit.node_index.clone(),
                    elements: vec![decoded_literal_interpolation(
                        lit.range,
                        lit.node_index.clone(),
                        &format!("({src})"),
                    )]
                    .into(),
                    flags: ast::FStringFlags::empty(),
                });
            }
            ast::FStringPart::FString(fstring) => {
                materialize_nested_fstring_sources(fstring, context);
            }
        }
    }
}

impl Transformer for &mut SurrogateStringLiteralLowerer<'_> {
    fn visit_body(&mut self, body: &mut Suite) {
        for (index, stmt) in body.iter_mut().enumerate() {
            if index == 0 && is_docstring_stmt(stmt) {
                continue;
            }
            self.visit_stmt(stmt);
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        walk_expr(self, expr);
        match expr {
            Expr::StringLiteral(node) => {
                *node = merged_string_literal_expr(node);
                let Some(src) = self.context.source_slice(node.range) else {
                    return;
                };
                if string::has_surrogate_escape(src) {
                    let wrapped = format!("({src})");
                    *expr = string::decode_literal_source_bytes_expr(wrapped.as_str());
                }
            }
            Expr::BytesLiteral(node) => {
                *node = merged_bytes_literal_expr(node);
            }
            Expr::FString(node) => {
                materialize_fstring_sources(node, self.context);
            }
            _ => {}
        }
    }
}

pub fn lower_surrogate_string_literals(context: &Context, stmts: &mut Suite) {
    let mut lowerer = SurrogateStringLiteralLowerer { context };
    (&mut lowerer).visit_body(stmts);
}

#[cfg(test)]
mod tests {
    use super::lower_surrogate_string_literals;
    use crate::basic_block::ast_to_ast::body::{suite_mut, suite_ref};
    use crate::basic_block::ast_to_ast::context::Context;
    use crate::basic_block::ast_to_ast::rewrite_expr::string::lower_string_templates_in_expr;
    use crate::basic_block::ast_to_ast::Options;
    use crate::ruff_ast_to_string;
    use ruff_python_ast::{self as ast, Expr, Stmt};
    use ruff_python_parser::parse_module;

    fn lower_module(source: &str) -> ast::ModModule {
        let mut module = parse_module(source).unwrap().into_syntax();
        let context = Context::new(Options::for_test(), source);
        lower_surrogate_string_literals(&context, suite_mut(&mut module.body));
        module
    }

    fn first_assign_value(module: &ast::ModModule) -> &Expr {
        let Stmt::Assign(assign) = &suite_ref(&module.body)[0] else {
            panic!("expected first statement to be an assignment");
        };
        assign.value.as_ref()
    }

    #[test]
    fn lower_surrogate_string_literals_merges_implicit_string_literals() {
        let module = lower_module("x = \"a\" \"b\"\n");
        let Expr::StringLiteral(node) = first_assign_value(&module) else {
            panic!("expected merged string literal");
        };
        assert!(!node.value.is_implicit_concatenated());
        assert_eq!(node.value.to_str(), "ab");
    }

    #[test]
    fn lower_surrogate_string_literals_merges_implicit_bytes_literals() {
        let module = lower_module("x = b\"a\" b\"b\"\n");
        let Expr::BytesLiteral(node) = first_assign_value(&module) else {
            panic!("expected merged bytes literal");
        };
        assert!(!node.value.is_implicit_concatenated());
        assert_eq!(node.value.bytes().collect::<Vec<_>>(), b"ab");
    }

    #[test]
    fn lower_surrogate_string_literals_still_decodes_surrogate_escapes_after_merge() {
        let module = lower_module("x = \"\\udca7\" \"b\"\n");
        let rendered = ruff_ast_to_string(suite_ref(&module.body));
        assert!(
            rendered.contains("__dp_decode_literal_source_bytes"),
            "{rendered}"
        );
    }

    #[test]
    fn lower_surrogate_string_literals_keeps_fstring_debug_output_correct() {
        let mut module = lower_module("x = f\"{value=}\"\n");
        let Stmt::Assign(assign) = &mut suite_mut(&mut module.body)[0] else {
            panic!("expected first statement to be an assignment");
        };
        lower_string_templates_in_expr(assign.value.as_mut());
        let rendered = ruff_ast_to_string(suite_ref(&module.body));
        assert!(rendered.contains("value="), "{rendered}");
        assert!(rendered.contains("__dp_repr(value)"), "{rendered}");
    }

    #[test]
    fn lower_surrogate_string_literals_keeps_tstring_expr_text_available() {
        let mut module = lower_module("x = t\"{value}\"\n");
        let Stmt::Assign(assign) = &mut suite_mut(&mut module.body)[0] else {
            panic!("expected first statement to be an assignment");
        };
        lower_string_templates_in_expr(assign.value.as_mut());
        let rendered = ruff_ast_to_string(suite_ref(&module.body));
        assert!(
            rendered.contains("__dp_templatelib_Interpolation(value, \"value\""),
            "{rendered}"
        );
    }

    #[test]
    fn lower_surrogate_string_literals_materializes_fstring_literal_surrogates() {
        let mut module = lower_module("x = f\"\\udca7\"\n");
        let Stmt::Assign(assign) = &mut suite_mut(&mut module.body)[0] else {
            panic!("expected first statement to be an assignment");
        };
        lower_string_templates_in_expr(assign.value.as_mut());
        let rendered = ruff_ast_to_string(suite_ref(&module.body));
        assert!(
            rendered.contains("__dp_decode_literal_source_bytes"),
            "{rendered}"
        );
    }
}

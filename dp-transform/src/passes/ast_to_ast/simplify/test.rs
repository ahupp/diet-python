use super::lower_surrogate_string_literals;
use crate::passes::ast_to_ast::body::{suite_mut, suite_ref};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::rewrite_expr::string::lower_string_templates_in_expr;
use crate::passes::ast_to_ast::Options;
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

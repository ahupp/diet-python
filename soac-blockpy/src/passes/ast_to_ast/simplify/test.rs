use crate::passes::ast_to_ast::rewrite_expr::string::lower_string_templates_in_expr;
use crate::ruff_ast_to_string;
use ruff_python_ast::{self as ast, Stmt};
use ruff_python_parser::parse_module;

fn parse_assign_module(source: &str) -> ast::ModModule {
    parse_module(source).unwrap().into_syntax()
}

#[test]
fn lower_string_templates_keeps_fstring_debug_output_correct() {
    let mut module = parse_assign_module("x = f\"{value=}\"\n");
    let Stmt::Assign(assign) = &mut module.body[0] else {
        panic!("expected first statement to be an assignment");
    };
    lower_string_templates_in_expr(assign.value.as_mut());
    let rendered = ruff_ast_to_string(&module.body);
    assert!(rendered.contains("value="), "{rendered}");
    assert!(rendered.contains("__dp_repr(value)"), "{rendered}");
}

#[test]
fn lower_string_templates_keeps_tstring_expr_text_available() {
    let mut module = parse_assign_module("x = t\"{value}\"\n");
    let Stmt::Assign(assign) = &mut module.body[0] else {
        panic!("expected first statement to be an assignment");
    };
    lower_string_templates_in_expr(assign.value.as_mut());
    let rendered = ruff_ast_to_string(&module.body);
    assert!(
        rendered.contains("__dp_templatelib_Interpolation(value, \"value\""),
        "{rendered}"
    );
}

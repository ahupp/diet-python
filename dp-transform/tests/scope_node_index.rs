use dp_transform::analyze_module_scope;
use ruff_python_ast::{self as ast, Stmt};
use ruff_python_parser::parse_module;

fn parse_module_body(source: &str) -> Vec<Stmt> {
    parse_module(source)
        .expect("parse failure")
        .into_syntax()
        .body
}

fn find_function<'a>(body: &'a [Stmt], name: &str) -> &'a ast::StmtFunctionDef {
    for stmt in body {
        if let Stmt::FunctionDef(func_def) = stmt {
            if func_def.name.id.as_str() == name {
                return func_def;
            }
        }
    }
    panic!("function {name} not found");
}

#[test]
fn analyze_module_scope_assigns_node_indices() {
    let mut body = parse_module_body("def f():\n    return 1\n");
    let module_scope = analyze_module_scope(&mut body);
    let func_def = find_function(&body, "f");
    let func_scope = module_scope.lookup_child_scope(func_def);
    assert!(func_scope.is_some());
}

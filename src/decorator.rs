use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Stmt};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub struct DecoratorRewriter {
    count: Cell<usize>,
}

impl DecoratorRewriter {
    pub fn new() -> Self {
        Self {
            count: Cell::new(0),
        }
    }

    fn next_tmp(&self) -> String {
        let id = self.count.get() + 1;
        self.count.set(id);
        format!("_dp_dec_{}", id)
    }

    fn rewrite(&self, decorators: Vec<ast::Decorator>, name: &str, item: Stmt) -> Stmt {
        let mut assigns = Vec::new();
        let mut names = Vec::new();

        for decorator in decorators {
            let tmp = self.next_tmp();
            let assign = crate::py_stmt!(
                "{name:id} = {decorator:expr}",
                name = tmp.as_str(),
                decorator = decorator.expression,
            );
            assigns.push(assign);
            names.push(tmp);
        }

        let mut call_expr = crate::py_expr!("{name:id}", name = name);
        for decorator in names.iter().rev() {
            call_expr = crate::py_expr!(
                "{decorator:id}({expr:expr})",
                decorator = decorator.as_str(),
                expr = call_expr,
            );
        }
        let call_stmt = crate::py_stmt!("{name:id} = {expr:expr}", name = name, expr = call_expr,);

        let mut body = assigns;
        body.push(item);
        body.push(call_stmt);
        crate::py_stmt!("{body:stmt}", body = body)
    }
}

impl Transformer for DecoratorRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef {
                decorator_list,
                name,
                ..
            }) => {
                if !decorator_list.is_empty() {
                    let decorators = std::mem::take(decorator_list);
                    let func_name = name.id.clone();
                    let func_def = stmt.clone();
                    *stmt = self.rewrite(decorators, func_name.as_str(), func_def);
                }
            }
            Stmt::ClassDef(ast::StmtClassDef {
                decorator_list,
                name,
                ..
            }) => {
                if !decorator_list.is_empty() {
                    let decorators = std::mem::take(decorator_list);
                    let class_name = name.id.clone();
                    let class_def = stmt.clone();
                    *stmt = self.rewrite(decorators, class_name.as_str(), class_def);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = DecoratorRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_function_decorators() {
        let input = r#"@dec2(5)
@dec1
def foo():
    pass
"#;
        let expected = r#"_dp_dec_1 = dec2(5)
_dp_dec_2 = dec1
def foo():
    pass
foo = _dp_dec_1(_dp_dec_2(foo))
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_class_decorators() {
        let input = r#"@dec
class C:
    pass
"#;
        let expected = r#"_dp_dec_1 = dec
class C:
    pass
C = _dp_dec_1(C)
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_multiple_class_decorators() {
        let input = r#"@dec2(5)
@dec1
class C:
    pass
"#;
        let expected = r#"_dp_dec_1 = dec2(5)
_dp_dec_2 = dec1
class C:
    pass
C = _dp_dec_1(_dp_dec_2(C))
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
    }
}

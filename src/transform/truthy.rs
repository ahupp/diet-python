use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};

pub struct TruthyRewriter;

impl TruthyRewriter {
    pub fn new() -> Self {
        Self
    }

    fn wrap_test(&self, test: &mut Expr) {
        let original = test.clone();
        *test = crate::py_expr!("__dp__.truth({expr:expr})", expr = original);
    }
}

impl Transformer for TruthyRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        match stmt {
            Stmt::If(ast::StmtIf {
                test,
                elif_else_clauses,
                ..
            }) => {
                self.wrap_test(test);
                for clause in elif_else_clauses {
                    if let Some(test) = &mut clause.test {
                        self.wrap_test(test);
                    }
                }
            }
            Stmt::While(ast::StmtWhile { test, .. }) => {
                self.wrap_test(test);
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
        let rewriter = TruthyRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_if_condition() {
        let output = rewrite("if a: pass\nelse: pass");
        let expected = "if __dp__.truth(a):\n    pass\nelse:\n    pass";
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_elif_and_else() {
        let output = rewrite("if a: pass\nelif b: pass\nelse: pass");
        let expected =
            "if __dp__.truth(a):\n    pass\nelif __dp__.truth(b):\n    pass\nelse:\n    pass";
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_while_condition() {
        let output = rewrite("while a: pass");
        let expected = "while __dp__.truth(a):\n    pass";
        assert_flatten_eq!(output, expected);
    }
}

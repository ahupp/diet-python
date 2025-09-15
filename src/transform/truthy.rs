use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::py_expr;

pub struct TruthyRewriter;

impl TruthyRewriter {
    pub fn new() -> Self {
        Self
    }

    fn wrap_test(&self, test: &mut Expr) {
        let original = test.clone();
        *test = py_expr!(
            "
__dp__.truth({expr:expr})
",
            expr = original,
        );
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
    use crate::test_util::{assert_transform_eq_ex, TransformPhase};

    #[test]
    fn rewrites_if_condition() {
        let input = r#"
if a:
    pass
else:
    pass
"#;
        let expected = r#"
if __dp__.truth(a):
    pass
else:
    pass
"#;
        assert_transform_eq_ex(input, expected, TransformPhase::Full);
    }

    #[test]
    fn rewrites_elif_and_else() {
        let input = r#"
if a:
    pass
elif b:
    pass
else:
    pass
"#;
        let expected = r#"
if __dp__.truth(a):
    pass
elif __dp__.truth(b):
    pass
else:
    pass
"#;
        assert_transform_eq_ex(input, expected, TransformPhase::Full);
    }

    #[test]
    fn rewrites_while_condition() {
        let input = r#"
while a:
    pass
"#;
        let expected = r#"
while __dp__.truth(a):
    pass
"#;
        assert_transform_eq_ex(input, expected, TransformPhase::Full);
    }
}

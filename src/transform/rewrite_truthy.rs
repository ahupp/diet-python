use crate::body_transform::{walk_stmt, Transformer};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, Stmt};

pub(crate) fn rewrite(body: &mut Vec<Stmt>) {
    let mut transformer = TruthyRewriter;
    transformer.visit_body(body);
}

struct TruthyRewriter;

impl Transformer for TruthyRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        match stmt {
            Stmt::If(ast::StmtIf {
                test,
                elif_else_clauses,
                ..
            }) => {
                wrap_truthy_expr(test);
                for clause in elif_else_clauses {
                    if let Some(test) = &mut clause.test {
                        wrap_truthy_expr(test);
                    }
                }
            }
            Stmt::While(ast::StmtWhile { test, .. }) => {
                wrap_truthy_expr(test);
            }
            _ => {}
        }
    }
}

fn wrap_truthy_expr(expr: &mut Expr) {
    if is_truth_call(expr) {
        return;
    }

    let original = expr.clone();
    *expr = py_expr!(
        "
__dp__.truth({expr:expr})
",
        expr = original,
    );
}

fn is_truth_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) if arguments.args.len() == 1 && arguments.keywords.is_empty() => match func.as_ref() {
            Expr::Attribute(ast::ExprAttribute { value, attr, .. }) if attr.as_str() == "truth" => {
                matches!(
                    value.as_ref(),
                    Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "__dp__"
                )
            }
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq_truthy;

    crate::transform_fixture_test!("tests_rewrite_truthy.txt");

    #[test]
    fn rewrites_truthy_if_condition() {
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
        assert_transform_eq_truthy(input, expected);
    }

    #[test]
    fn rewrites_truthy_elif_and_else() {
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
        assert_transform_eq_truthy(input, expected);
    }

    #[test]
    fn rewrites_truthy_while_condition() {
        let input = r#"
while a:
    pass
"#;
        let expected = r#"
while __dp__.truth(a):
    pass
"#;
        assert_transform_eq_truthy(input, expected);
    }
}

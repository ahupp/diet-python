use super::{rewrite_with_pass, ExprRewritePass, LoweredExpr};
use crate::passes::ast_to_ast::context::Context;
use crate::py_expr;
use ruff_python_ast::Expr;
use ruff_python_parser::parse_module;

struct RenameXExprPass;

impl ExprRewritePass for RenameXExprPass {
    fn lower_expr(&self, _context: &Context, expr: Expr) -> LoweredExpr {
        match expr {
            Expr::Name(name) if name.id.as_str() == "x" => {
                LoweredExpr::modified(py_expr!("renamed"), Vec::new())
            }
            other => LoweredExpr::unmodified(other),
        }
    }
}

#[test]
fn rewrite_with_expr_pass_only_traverses_stmt_bodies() {
    let source = r#"
def f():
    return x
"#;
    let mut module = parse_module(source).unwrap().into_syntax().body;
    let context = Context::new(source);

    rewrite_with_pass(&context, None, Some(&RenameXExprPass), &mut module);

    let rendered = crate::ruff_ast_to_string(&module);
    assert!(rendered.contains("return renamed"), "{rendered}");
}

use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Stmt};

pub struct ForLoopRewriter {
    iter_count: Cell<usize>,
}

impl ForLoopRewriter {
    pub fn new() -> Self {
        Self {
            iter_count: Cell::new(0),
        }
    }
}

impl Transformer for ForLoopRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        if let Stmt::For(ast::StmtFor {
            target,
            iter: iter_expr,
            body,
            orelse,
            is_async,
            ..
        }) = stmt
        {
            if *is_async {
                return;
            }

            let id = self.iter_count.get() + 1;
            self.iter_count.set(id);
            let iter_name = format!("_dp_iter_{}", id);

            let body_stmts = std::mem::take(body);

            let mut except_body = std::mem::take(orelse);
            except_body.push(crate::py_stmt!("break"));

            let wrapper = crate::py_stmt!(
                "
{iter_name:id} = iter({iter:expr})
while True:
    try:
        {target:expr} = next({iter_name:id})
    except StopIteration:
        {except_body:stmt}
    {body:stmt}
",
                iter_name = iter_name.as_str(),
                iter = *iter_expr.clone(),
                target = *target.clone(),
                except_body = except_body,
                body = body_stmts,
            );

            *stmt = wrapper;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::flatten;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_codegen::{Generator as Codegen, Stylist};
    use ruff_python_parser::parse_module;

    fn rewrite_for(source: &str) -> String {
        let parsed = parse_module(source).expect("parse error");
        let tokens = parsed.tokens().clone();
        let mut module = parsed.into_syntax();

        let rewriter = ForLoopRewriter::new();
        walk_body(&rewriter, &mut module.body);
        if let [Stmt::If(ast::StmtIf { body, .. })] = module.body.as_mut_slice() {
            flatten(body);
        }

        let stylist = Stylist::from_tokens(&tokens, source);
        let mut output = String::new();
        for stmt in &module.body {
            let snippet = Codegen::from(&stylist).stmt(stmt);
            output.push_str(&snippet);
            output.push_str(stylist.line_ending().as_str());
        }
        output
    }

    #[test]
    fn rewrites_for_loop_with_else() {
        let input = r#"
for a in b:
    if a % 2 == 0:
        c(a)
    else:
        break
else:
    c(0)
"#;
        let expected = r#"
if True:
    _dp_iter_1 = iter(b)
    while True:
        try:
            a = next(_dp_iter_1)
        except StopIteration:
            c(0)
            break
        if a % 2 == 0:
            c(a)
        else:
            break
"#;
        let output = rewrite_for(input);
        assert_eq!(output.trim(), expected.trim());
    }
}

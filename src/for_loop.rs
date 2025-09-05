use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::TextRange;

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
            except_body.push(crate::py_stmt!("break").into_iter().next().unwrap());

            let inner = crate::py_stmt!(
                "{iter_name:id} = iter({iter:expr})\nwhile True:\n    try:\n        {target:expr} = next({iter_name:id})\n    except StopIteration:\n        {except_body:stmt}\n    {body:stmt}",
                iter_name = iter_name.as_str(),
                iter = *iter_expr.clone(),
                target = *target.clone(),
                except_body = except_body,
                body = body_stmts,
            );

            let wrapper = Stmt::If(ast::StmtIf {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                test: Box::new(crate::py_expr!("True")),
                body: inner,
                elif_else_clauses: Vec::new(),
            });

            *stmt = wrapper;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn flatten(body: &mut Vec<Stmt>) {
        let mut i = 0;
        while i < body.len() {
            match &mut body[i] {
                Stmt::If(ast::StmtIf {
                    test,
                    body: inner,
                    elif_else_clauses,
                    ..
                }) => {
                    flatten(inner);
                    for clause in elif_else_clauses.iter_mut() {
                        flatten(&mut clause.body);
                    }
                    if elif_else_clauses.is_empty()
                        && matches!(
                            test.as_ref(),
                            ast::Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })
                        )
                    {
                        let replacement = std::mem::take(inner);
                        body.splice(i..=i, replacement);
                        continue;
                    }
                }
                Stmt::While(ast::StmtWhile { body: inner, orelse, .. }) => {
                    flatten(inner);
                    flatten(orelse);
                }
                Stmt::Try(ast::StmtTry {
                    body: inner,
                    handlers,
                    orelse,
                    finalbody,
                    ..
                }) => {
                    flatten(inner);
                    for handler in handlers.iter_mut() {
                        let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler { body, .. }) = handler;
                        flatten(body);
                    }
                    flatten(orelse);
                    flatten(finalbody);
                }
                _ => {}
            }
            i += 1;
        }
    }

    #[test]
    fn rewrites_for_loop_with_else() {
        let input = concat!(
            "for a in b:\n",
            "    if a % 2 == 0:\n",
            "        c(a)\n",
            "    else:\n",
            "        break\n",
            "else:\n",
            "    c(0)",
        );
        let expected = concat!(
            "if True:\n",
            "    _dp_iter_1 = iter(b)\n",
            "    while True:\n",
            "        try:\n",
            "            a = next(_dp_iter_1)\n",
            "        except StopIteration:\n",
            "            c(0)\n",
            "            break\n",
            "        if a % 2 == 0:\n",
            "            c(a)\n",
            "        else:\n",
            "            break",
        );
        let output = rewrite_for(input);
        assert_eq!(output.trim_end(), expected.trim_end());
    }
}

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
            iter,
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

            let iter_expr = crate::py_expr!("{name:id}", name = iter_name.as_str());
            let iter_call = crate::py_expr!("iter({iter:expr})", iter = *iter.clone());

            let assign_iter = Stmt::Assign(ast::StmtAssign {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                targets: vec![iter_expr.clone()],
                value: Box::new(iter_call),
            });

            let next_call = crate::py_expr!("next({iter:expr})", iter = iter_expr.clone());
            let assign_next = Stmt::Assign(ast::StmtAssign {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                targets: vec![(*target.clone())],
                value: Box::new(next_call),
            });

            let body_stmts = std::mem::take(body);

            let try_body = vec![assign_next];

            let mut except_body = std::mem::take(orelse);
            except_body.push(Stmt::Break(ast::StmtBreak {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
            }));

            let handler = ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                type_: Some(Box::new(crate::py_expr!("StopIteration"))),
                name: None,
                body: except_body,
            });

            let try_stmt = Stmt::Try(ast::StmtTry {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                body: try_body,
                handlers: vec![handler],
                orelse: Vec::new(),
                finalbody: Vec::new(),
                is_star: false,
            });

            let mut while_body = vec![try_stmt];
            while_body.extend(body_stmts);

            let while_stmt = Stmt::While(ast::StmtWhile {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                test: Box::new(crate::py_expr!("True")),
                body: while_body,
                orelse: Vec::new(),
            });

            let wrapper = Stmt::If(ast::StmtIf {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                test: Box::new(crate::py_expr!("True")),
                body: vec![assign_iter, while_stmt],
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

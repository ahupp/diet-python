use crate::basic_block::ast_to_ast::ast_rewrite::Rewrite;

use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::TextRange;

pub fn expand_if_chain(mut if_stmt: ast::StmtIf) -> Rewrite {
    if !if_stmt
        .elif_else_clauses
        .iter()
        .any(|clause| clause.test.is_some())
    {
        return Rewrite::Unmodified(if_stmt.into());
    }
    let mut else_body: Option<ast::StmtBody> = None;

    for clause in if_stmt.elif_else_clauses.into_iter().rev() {
        match clause.test {
            Some(test) => {
                let mut nested_if = ast::StmtIf {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    test: Box::new(test),
                    body: clause.body,
                    elif_else_clauses: Vec::new(),
                };

                if let Some(body) = else_body.take() {
                    nested_if.elif_else_clauses.push(ast::ElifElseClause {
                        test: None,
                        body,
                        range: TextRange::default(),
                        node_index: ast::AtomicNodeIndex::default(),
                    });
                }

                else_body = Some(ast::StmtBody {
                    body: vec![Box::new(Stmt::If(nested_if))],
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                });
            }
            None => {
                else_body = Some(clause.body);
            }
        }
    }

    if let Some(body) = else_body {
        if_stmt.elif_else_clauses = vec![ast::ElifElseClause {
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
            test: None,
            body,
        }];
    } else {
        if_stmt.elif_else_clauses = Vec::new();
    }

    Rewrite::Walk(if_stmt.into())
}

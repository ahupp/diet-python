use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::body_transform::Transformer;


pub fn strip_generated_passes(stmts: &mut Vec<Stmt>) {
    struct StripGeneratedPasses;

    impl Transformer for StripGeneratedPasses {
        fn visit_body(&mut self, body: &mut Vec<Stmt>) {
            crate::body_transform::walk_body(self, body);
            let mut updated = Vec::with_capacity(body.len());
            for stmt in body.drain(..) {
                match stmt {
                    Stmt::If(mut if_stmt) => {
                        if if_stmt.body.is_empty() {
                            if_stmt.body.push(Stmt::Pass(ast::StmtPass {
                                node_index: Default::default(),
                                range: Default::default(),
                            }));
                        }
                        for clause in if_stmt.elif_else_clauses.iter_mut() {
                            if clause.body.is_empty() {
                                clause.body.push(Stmt::Pass(ast::StmtPass {
                                    node_index: Default::default(),
                                    range: Default::default(),
                                }));
                            }
                        }
                        if_stmt.elif_else_clauses.retain(|clause| {
                            !(clause.body.len() == 1 && matches!(clause.body[0], Stmt::Pass(_)))
                        });

                        if if_stmt.body.len() == 1
                            && matches!(if_stmt.body[0], Stmt::Pass(_))
                            && if_stmt.elif_else_clauses.is_empty()
                        {
                            updated.extend(crate::py_stmt!("{expr:expr}", expr = if_stmt.test));
                            continue;
                        }

                        updated.push(Stmt::If(if_stmt));
                        continue;
                    }
                    Stmt::Expr(ast::StmtExpr { ref value, .. })
                        if matches!(
                            value.as_ref(),
                            Expr::Name(ast::ExprName { id, .. })
                                if id.as_str().starts_with("_dp_")
                        ) =>
                    {
                        continue;
                    }
                    other => {
                        updated.push(other);
                        continue;
                    }
                }
            }

            if updated.len() > 1 {
                updated.retain(|stmt| !matches!(stmt, Stmt::Pass(_)));

                if updated.is_empty() {
                    updated.extend(crate::py_stmt!("pass"));
                }
            }

            *body = updated;
        }
    }

    let mut stripper = StripGeneratedPasses;
    stripper.visit_body(stmts);
}

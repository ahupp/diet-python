use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::body_transform::Transformer;


pub(crate) struct Flattener;

impl Flattener {
    fn visit_stmts(&mut self, body: &mut Vec<Stmt>) {
        let mut i = 0;
        while i < body.len() {
            self.visit_stmt(&mut body[i]);
            if let Stmt::If(ast::StmtIf {
                test,
                body: inner,
                elif_else_clauses,
                ..
            }) = &mut body[i]
            {
                if elif_else_clauses.is_empty()
                    && matches!(
                        test.as_ref(),
                        Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })
                    )
                {
                    let replacement = std::mem::take(inner);
                    body.splice(i..=i, replacement);
                    continue;
                }
            }
            i += 1;
        }
    }
}

fn remove_placeholder_pass(stmts: &mut Vec<Stmt>) {
    if stmts.len() == 1 {
        if let Stmt::Pass(ast::StmtPass { range, .. }) = &stmts[0] {
            if range.is_empty() {
                stmts.clear();
            }
        }
    }
}

impl Transformer for Flattener {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::If(ast::StmtIf {
                body,
                elif_else_clauses,
                ..
            }) => {
                self.visit_stmts(body);
                remove_placeholder_pass(body);
                for clause in elif_else_clauses.iter_mut() {
                    self.visit_stmts(&mut clause.body);
                    remove_placeholder_pass(&mut clause.body);
                }
            }
            Stmt::For(ast::StmtFor {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_stmts(inner);
                remove_placeholder_pass(inner);
                self.visit_stmts(orelse);
                remove_placeholder_pass(orelse);
            }
            Stmt::While(ast::StmtWhile {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_stmts(inner);
                remove_placeholder_pass(inner);
                self.visit_stmts(orelse);
                remove_placeholder_pass(orelse);
            }
            Stmt::Try(ast::StmtTry {
                body: inner,
                handlers,
                orelse,
                finalbody,
                ..
            }) => {
                self.visit_stmts(inner);
                remove_placeholder_pass(inner);
                for handler in handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                        body,
                        ..
                    }) = handler;
                    self.visit_stmts(body);
                    remove_placeholder_pass(body);
                }
                self.visit_stmts(orelse);
                remove_placeholder_pass(orelse);
                self.visit_stmts(finalbody);
                remove_placeholder_pass(finalbody);
            }
            Stmt::FunctionDef(ast::StmtFunctionDef { body: inner, .. }) => {
                self.visit_stmts(inner);
                remove_placeholder_pass(inner);
            }
            _ => {}
        }
    }
}


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


pub fn flatten(stmts: &mut Vec<Stmt>) {
    let mut flattener = Flattener;
    flattener.visit_stmts(stmts);
}

pub fn strip_generated_passes(stmts: &mut Vec<Stmt>) {
    flatten(stmts);

    let mut stripper = StripGeneratedPasses;
    stripper.visit_body(stmts);
}

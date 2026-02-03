use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use crate::{transform::context::Context, transformer::{Transformer, walk_body}};


pub(crate) struct Flattener;

impl Flattener {
    fn visit_body(&mut self, body: &mut StmtBody) {
        let body = &mut body.body;
        let mut i = 0;
        while i < body.len() {
            self.visit_stmt(body[i].as_mut());
            if let Stmt::BodyStmt(ast::StmtBody { body: inner, .. }) = body[i].as_mut() {
                let replacement = std::mem::take(inner);
                body.splice(i..=i, replacement);
                continue;
            }
            if let Stmt::If(ast::StmtIf {
                test,
                body: inner,
                elif_else_clauses,
                ..
            }) = body[i].as_mut()
            {
                if elif_else_clauses.is_empty()
                    && matches!(
                        test.as_ref(),
                        Expr::BooleanLiteral(ast::ExprBooleanLiteral { value: true, .. })
                    )
                {
                    let replacement = std::mem::take(&mut inner.body);
                    body.splice(i..=i, replacement);
                    continue;
                }
            }
            i += 1;
        }
    }
}

fn remove_placeholder_pass(body: &mut StmtBody) {
    let body = &mut body.body;
    if body.len() == 1 {
        if let Stmt::Pass(ast::StmtPass { range, .. }) = body[0].as_ref() {
            if range.is_empty() {
                body.clear();
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
                self.visit_body(body);
                remove_placeholder_pass(body);
                for clause in elif_else_clauses.iter_mut() {
                    self.visit_body(&mut clause.body);
                    remove_placeholder_pass(&mut clause.body);
                }
            }
            Stmt::For(ast::StmtFor {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_body(inner);
                remove_placeholder_pass(inner);
                self.visit_body(orelse);
                remove_placeholder_pass(orelse);
            }
            Stmt::While(ast::StmtWhile {
                body: inner,
                orelse,
                ..
            }) => {
                self.visit_body(inner);
                remove_placeholder_pass(inner);
                self.visit_body(orelse);
                remove_placeholder_pass(orelse);
            }
            Stmt::Try(ast::StmtTry {
                body: inner,
                handlers,
                orelse,
                finalbody,
                ..
            }) => {
                self.visit_body(inner);
                remove_placeholder_pass(inner);
                for handler in handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                        body,
                        ..
                    }) = handler;
                    self.visit_body(body);
                    remove_placeholder_pass(body);
                }
                self.visit_body(orelse);
                remove_placeholder_pass(orelse);
                self.visit_body(finalbody);
                remove_placeholder_pass(finalbody);
            }
            Stmt::FunctionDef(ast::StmtFunctionDef { body: inner, .. }) => {
                self.visit_body(inner);
                remove_placeholder_pass(inner);
            }
            _ => {}
        }
    }
}


struct StripGeneratedPasses;

impl Transformer for &mut StripGeneratedPasses {
    fn visit_body(&mut self, body: &mut StmtBody) {
        walk_body(self, body);
        let mut updated = Vec::with_capacity(body.body.len());
        for stmt in body.body.drain(..) {
            let stmt = *stmt;
            match stmt {
                Stmt::If(mut if_stmt) => {
                    if if_stmt.body.body.is_empty() {
                        if_stmt
                            .body
                            .body
                            .push(Box::new(Stmt::Pass(ast::StmtPass {
                                node_index: Default::default(),
                                range: Default::default(),
                            })));
                    }
                    for clause in if_stmt.elif_else_clauses.iter_mut() {
                        if clause.body.body.is_empty() {
                            clause
                                .body
                                .body
                                .push(Box::new(Stmt::Pass(ast::StmtPass {
                                    node_index: Default::default(),
                                    range: Default::default(),
                                })));
                        }
                    }
                    if_stmt.elif_else_clauses.retain(|clause| {
                        clause
                            .body
                            .body
                            .len()
                            .ne(&1)
                            || !matches!(clause.body.body[0].as_ref(), Stmt::Pass(_))
                    });

                    let is_empty_if = if_stmt
                        .body
                        .body
                        .len()
                        .eq(&1)
                        && matches!(if_stmt.body.body[0].as_ref(), Stmt::Pass(_))
                        && if_stmt.elif_else_clauses.is_empty();
                    if is_empty_if {
                        updated.push(crate::py_stmt!("__dp__.truth({expr:expr})", expr = if_stmt.test));
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
                updated.push(crate::py_stmt!("pass"));
            }
        }

        body.body = updated.into_iter().map(Box::new).collect();
    }
}


pub fn flatten(stmts: &mut StmtBody) {
    let mut flattener = Flattener;
    (&mut flattener).visit_body(stmts);
}

pub fn strip_generated_passes(_context: &Context, stmts: &mut StmtBody) {
    flatten(stmts);

    let mut stripper = StripGeneratedPasses;
    (&mut stripper).visit_body(stmts);
}

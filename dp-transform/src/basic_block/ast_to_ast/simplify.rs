use crate::{
    basic_block::ast_to_ast::context::Context,
    basic_block::ast_to_ast::rewrite_expr::string,
    transformer::{walk_expr, Transformer},
};
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};

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

pub fn flatten(stmts: &mut StmtBody) {
    let mut flattener = Flattener;
    (&mut flattener).visit_body(stmts);
}

fn is_docstring_stmt(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    )
}

struct SurrogateStringLiteralLowerer<'a> {
    context: &'a Context,
}

impl Transformer for &mut SurrogateStringLiteralLowerer<'_> {
    fn visit_body(&mut self, body: &mut StmtBody) {
        for (index, stmt) in body.body.iter_mut().enumerate() {
            if index == 0 && is_docstring_stmt(stmt) {
                continue;
            }
            self.visit_stmt(stmt);
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        walk_expr(self, expr);
        let Expr::StringLiteral(ast::ExprStringLiteral { range, .. }) = expr else {
            return;
        };
        let Some(src) = self.context.source_slice(*range) else {
            return;
        };
        if string::has_surrogate_escape(src) {
            let wrapped = format!("({src})");
            *expr = string::decode_literal_source_bytes_expr(wrapped.as_str());
        }
    }
}

pub fn lower_surrogate_string_literals(context: &Context, stmts: &mut StmtBody) {
    let mut lowerer = SurrogateStringLiteralLowerer { context };
    (&mut lowerer).visit_body(stmts);
}

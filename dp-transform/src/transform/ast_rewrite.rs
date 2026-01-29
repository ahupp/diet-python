use std::{backtrace::Backtrace, mem::take};

use log::{Level, log_enabled, trace};
use ruff_python_ast::{Expr, Stmt, StmtBody, self as ast};
use ruff_text_size::{Ranged, TextRange};

use crate::{ruff_ast_to_string, template::{empty_body, into_body}, transform::context::Context};
use crate::transformer::{Transformer, walk_expr, walk_stmt};


pub enum Rewrite {
    Unmodified(Stmt),
    Walk(Stmt),
}

pub struct LoweredExpr {
    pub stmt: Stmt,
    pub expr: Expr,
    pub modified: bool,
}

#[derive(Default)]
pub struct BodyBuilder {
    pub modified: bool,
    pub body: Vec<Box<Stmt>>,
}

impl BodyBuilder {


    pub fn into_stmt(self) -> Stmt {
        Stmt::BodyStmt(ast::StmtBody { body: self.body, range: TextRange::default(), node_index: ast::AtomicNodeIndex::default() })
    }

    pub fn push(&mut self, expr: LoweredExpr) -> Expr {
        extend_body(&mut self.body, Box::new(expr.stmt));
        self.modified |= expr.modified;
        expr.expr
    }
}

struct FlattenBodyTransformer;

fn extend_body(new_body: &mut Vec<Box<Stmt>>, mut stmt: Box<Stmt>) {

    match stmt.as_mut() {
        Stmt::BodyStmt(ast::StmtBody { body, ..}) => {
            for stmt in take(body).into_iter() {
                extend_body(new_body, stmt);
            }
        }
        _ => {
            new_body.push(stmt);
            return;
        }
    }
}


impl Transformer for FlattenBodyTransformer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::BodyStmt(ast::StmtBody { body, range, node_index }) => {
                let mut new_body = Vec::new();
                for stmt in take(body).into_iter() {
                    extend_body(&mut new_body, stmt);
                }
                *stmt = Stmt::BodyStmt(ast::StmtBody { body: new_body, range: *range, node_index: node_index.clone() });
            }
            _ => {}
        }
    }
}

pub fn push_stmt(stmt: &mut Stmt, new_stmt: Stmt) -> bool {
   
    let this_stmt = std::mem::replace( stmt, empty_body().into());
    *stmt = into_body(vec![this_stmt, new_stmt]).into();
    FlattenBodyTransformer.visit_stmt(stmt);
    match &stmt {
        Stmt::BodyStmt(ast::StmtBody { body, .. }) => {
            !body.is_empty()
        }
        _ => false
    }
}

impl LoweredExpr {

    pub fn modified(expr: Expr, stmt: impl Into<Stmt>) -> Self {
        trace!("LoweredExpr::modified {}",  Backtrace::capture());
        Self {
            stmt: stmt.into(),
            expr,
            modified: true,
        }
    }

    pub fn unmodified(expr: Expr) -> Self {
        Self {
            stmt: empty_body().into(),
            expr,
            modified: false,
        }
    }
}

pub fn rewrite_once_with_pass<'a, P: RewritePass>(
    context: &'a Context,
    pass: &'a P,
    body: &mut StmtBody,
) -> bool {
    let pass_name = std::any::type_name::<P>();
    let mut rloop = RewriteLoop {
        context,
        pass,
        pass_name,
        buf: Vec::new(),
        modified: false,
        suppress_lowering: 0,
    };
    rloop.visit_body(body);
    assert!(rloop.buf.is_empty());
    rloop.modified
}

pub fn rewrite_with_pass<'a, P: RewritePass>(
    context: &'a Context,
    pass: &'a P,
    body: &mut StmtBody,
) {
    let pass_name = std::any::type_name::<P>();
    let mut iteration = 0usize;
    loop {
        iteration += 1;
        if log_enabled!(Level::Trace) {
            trace!(
                "rewrite_with_pass iteration {} start: {}",
                iteration,
                pass_name,
            );
        }
        let modified = rewrite_once_with_pass(context, pass, body);

        if log_enabled!(Level::Trace) {
            trace!(
                "rewrite_with_pass iteration {} end: {} modified={}",
                iteration,
                pass_name,
                modified
            );
        }
        if !modified {
            break;
        }
    }
}

struct RewriteLoop<'a, P: RewritePass> {
    buf: Vec<Stmt>,
    context: &'a Context,
    pass: &'a P,
    pass_name: &'static str,
    modified: bool,
    suppress_lowering: usize,
}


impl<'a, P: RewritePass> RewriteLoop<'a, P> {
    fn flush_buffered(&mut self, mut stmt: Stmt, output: &mut Vec<Stmt>) {
        walk_stmt(self, &mut stmt);

        let buffered = take(&mut self.buf);

        for stmt in buffered.into_iter() {
            self.flush_buffered(stmt, output);
        }
        output.push(stmt);
    }

    fn process_statements(&mut self, initial: Vec<Stmt>) -> Vec<Stmt> {
        let mut output = Vec::new();
        assert!(self.buf.is_empty());

        for stmt in initial.into_iter() {
            let mut before = None;
            if log_enabled!(Level::Trace) {
                before = Some(crate::ruff_ast_to_string(&stmt));
            }
            let res = self.pass.lower_stmt(self.context, stmt);
            match res {
                Rewrite::Unmodified(stmt) => {
                    self.flush_buffered(stmt, &mut output);
                }
                Rewrite::Walk(stmt) => {
                    if log_enabled!(Level::Trace) {
                        trace!(
                            "rewrite (pass={}) before: \n{} after: \n{}",
                            self.pass_name,
                            before.unwrap_or_default(),
                            crate::ruff_ast_to_string(&stmt).trim_end()
                        );
                    }
                    self.modified = true;
                    self.flush_buffered(stmt, &mut output);
                }                
            }
        }

        output
    }

}

impl<'a, P: RewritePass> Transformer for RewriteLoop<'a, P> {
    fn visit_body(&mut self, body: &mut StmtBody) {

        let saved_buf = take(&mut self.buf);
        let stmts = take(&mut body.body)
            .into_iter()
            .map(|stmt| *stmt)
            .collect::<Vec<_>>();
        body.body = self
            .process_statements(stmts)
            .into_iter()
            .map(Box::new)
            .collect();
        self.buf = saved_buf;
    }

    fn visit_expr(&mut self, expr_input: &mut Expr) {
        if self.suppress_lowering > 0 {
            walk_expr(self, expr_input);
            return;
        }
        if matches!(expr_input, Expr::Lambda(_) | Expr::Generator(_)) {
            self.suppress_lowering += 1;
            walk_expr(self, expr_input);
            self.suppress_lowering -= 1;
            return;
        }

        let original_range = expr_input.range();
        let mut lowered: LoweredExpr;
        let mut current = expr_input.clone();
        let mut modified_any = false;
        let mut iteration = 0usize;
        loop {
            iteration += 1;
            let mut log_input = None;
            if log_enabled!(Level::Trace) {
                log_input = Some(ruff_ast_to_string(&current).trim_end().to_string());
            }
            lowered = self.pass.lower_expr(self.context, current);

            let LoweredExpr { stmt, expr, modified } = lowered;
            if log_enabled!(Level::Trace) {
                trace!(
                    "lower_expr iteration={} modified={} pass={} \ninput: {}\noutput: \n{}\nstmt: \n{}",
                    iteration,
                    modified,
                    self.pass_name,
                    log_input.unwrap_or_default(),
                    ruff_ast_to_string(&expr).trim_end(),
                    ruff_ast_to_string(&stmt).trim_end(),
                );
            }
            self.buf.push(stmt);
            
            current = expr;

            apply_expr_range(&mut current, original_range);
            if !modified {
                break;
            }
            modified_any = true;
            self.modified = true;
        }

        if !modified_any {
            if log_enabled!(Level::Trace) {
                trace!(
                    "walk_expr (pass={}): {}",
                    self.pass_name,
                    ruff_ast_to_string(&current)
                    .trim_end()
                );
            }

            walk_expr(self, &mut current);
        }
        *expr_input = current;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        let rewritten = self.process_statements(vec![stmt.clone()]);
        *stmt = into_body(rewritten);
    }
}


pub trait RewritePass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite;
    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr;
}


fn apply_expr_range(expr: &mut Expr, range: TextRange) {
    match expr {
        Expr::BoolOp(node) => node.range = range,
        Expr::Named(node) => node.range = range,
        Expr::BinOp(node) => node.range = range,
        Expr::UnaryOp(node) => node.range = range,
        Expr::Lambda(node) => node.range = range,
        Expr::If(node) => node.range = range,
        Expr::Dict(node) => node.range = range,
        Expr::Set(node) => node.range = range,
        Expr::ListComp(node) => node.range = range,
        Expr::SetComp(node) => node.range = range,
        Expr::DictComp(node) => node.range = range,
        Expr::Generator(node) => node.range = range,
        Expr::Await(node) => node.range = range,
        Expr::Yield(node) => node.range = range,
        Expr::YieldFrom(node) => node.range = range,
        Expr::Compare(node) => node.range = range,
        Expr::Call(node) => node.range = range,
        Expr::FString(node) => node.range = range,
        Expr::TString(node) => node.range = range,
        Expr::StringLiteral(node) => node.range = range,
        Expr::BytesLiteral(node) => node.range = range,
        Expr::NumberLiteral(node) => node.range = range,
        Expr::BooleanLiteral(node) => node.range = range,
        Expr::NoneLiteral(node) => node.range = range,
        Expr::EllipsisLiteral(node) => node.range = range,
        Expr::Attribute(node) => node.range = range,
        Expr::Subscript(node) => node.range = range,
        Expr::Starred(node) => node.range = range,
        Expr::Name(node) => node.range = range,
        Expr::List(node) => node.range = range,
        Expr::Tuple(node) => node.range = range,
        Expr::Slice(node) => node.range = range,
        Expr::IpyEscapeCommand(node) => node.range = range,
    }
}

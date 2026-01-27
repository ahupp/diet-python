use std::{mem::take};

use log::{Level, log_enabled, trace};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::{
    body_transform::{Transformer, walk_expr, walk_stmt},
    py_stmt,
    ruff_ast_to_string,
    transform::context::Context,
};


pub enum Rewrite {
    Unmodified(Stmt),
    Walk(Vec<Stmt>),
}

pub struct LoweredExpr {
    pub stmts: Vec<Stmt>,
    pub expr: Expr,
    pub modified: bool,
}


impl LoweredExpr {

    pub fn modified(expr: Expr, stmts: Vec<Stmt>) -> Self {
        Self {
            stmts,
            expr,
            modified: true,
        }
    }

    pub fn unmodified(expr: Expr) -> Self {
        Self {
            stmts: Vec::new(),
            expr,
            modified: false,
        }
    }

    pub fn extend(self, other: Vec<Stmt>) -> Self {
        let Self { mut stmts, expr, modified } = self;
        stmts.extend(other);
        Self {
            stmts,
            expr,
            modified,
        }
    }
}

pub fn rewrite_once_with_pass<'a, P: RewritePass>(
    context: &'a Context,
    pass: &'a P,
    stmts: &mut Vec<Stmt>,
) -> bool {
    let pass_name = std::any::type_name::<P>();
    let mut rloop = RewriteLoop {
        context,
        pass,
        pass_name,
        buf: Vec::new(),
        modified: false,
    };
    rloop.visit_body(stmts);
    assert!(rloop.buf.is_empty());
    rloop.modified
}

pub fn rewrite_with_pass<'a, P: RewritePass>(
    context: &'a Context,
    pass: &'a P,
    stmts: &mut Vec<Stmt>,
) {
    let pass_name = std::any::type_name::<P>();
    let mut iteration = 0usize;
    loop {
        iteration += 1;
        if log_enabled!(Level::Trace) {
            trace!(
                "rewrite_with_pass iteration {} start: {} stmts_len={}",
                iteration,
                pass_name,
                stmts.len()
            );
        }
        let modified = rewrite_once_with_pass(context, pass, stmts);

        if log_enabled!(Level::Trace) {
            trace!(
                "rewrite_with_pass iteration {} end: {} stmts_len={} modified={}",
                iteration,
                pass_name,
                stmts.len(),
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
                Rewrite::Walk(stmts) => {
                    if log_enabled!(Level::Trace) {
                        trace!(
                            "rewrite (pass={}) before: \n{} after: \n{}",
                            self.pass_name,
                            before.unwrap_or_default(),
                            crate::ruff_ast_to_string(&stmts).trim_end()
                        );
                    }
                    self.modified = true;
                    for stmt in stmts {
                        self.flush_buffered(stmt, &mut output);
                    }
                }                
            }
        }

        output
    }

}

impl<'a, P: RewritePass> Transformer for RewriteLoop<'a, P> {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let saved_buf = take(&mut self.buf);
        let stmts = take(body);
        *body = self.process_statements(stmts);
        self.buf = saved_buf;
    }

    fn visit_expr(&mut self, expr_input: &mut Expr) {

        let original_range = expr_input.range();
        let mut lowered: LoweredExpr;
        let mut current = expr_input.clone();
        let mut modified_any = false;
        let mut iteration = 0usize;
        loop {
            iteration += 1;
            if log_enabled!(Level::Trace) {
                trace!(
                    "lower_expr iteration {} pass={} input: {}",
                    iteration,
                    self.pass_name,
                    ruff_ast_to_string(&current).trim_end()
                );
            }
            lowered = self.pass.lower_expr(self.context, current);
            if log_enabled!(Level::Trace) {
                trace!(
                    "lower_expr iteration {} pass={} output: {}",
                    iteration,
                    self.pass_name,
                    ruff_ast_to_string(&lowered.expr).trim_end()
                );
            }

            let LoweredExpr { stmts, expr, modified } = lowered;
            if log_enabled!(Level::Trace) {
                trace!(
                    "lower_expr iteration {} pass={} modified={}",
                    iteration,
                    self.pass_name,
                    modified
                );
            }
            self.buf.extend(stmts);
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
        let mut rewritten = self.process_statements(vec![stmt.clone()]);
        match rewritten.len() {
            0 => *stmt = py_stmt!("pass")[0].clone(),
            1 => *stmt = rewritten.remove(0),
            _ => {
                *stmt = rewritten.remove(0);
                self.buf.extend(rewritten);
            }
        }
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

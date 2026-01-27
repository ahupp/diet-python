use std::{collections::VecDeque, mem::take};

use log::{Level, log_enabled, trace};
use ruff_python_ast::{self as ast, Expr, Stmt, HasNodeIndex};
use ruff_text_size::{Ranged, TextRange};

use crate::{
    body_transform::{Transformer, walk_expr, walk_stmt},
    py_stmt,
    ruff_ast_to_string,
    transform::context::Context,
};


pub enum Rewrite {
    Walk(Vec<Stmt>),
    Visit(Vec<Stmt>),
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

pub fn rewrite_with_pass<'a, P: RewritePass>(
    context: &'a Context,
    pass: &'a P,
    stmts: &mut Vec<Stmt>,
) {
    let pass_name = std::any::type_name::<P>();
    if log_enabled!(Level::Trace) {
        trace!("rewrite_with_pass start: {} stmts_len={}", pass_name, stmts.len());
    }
    let mut rloop = RewriteLoop {
        context,
        pass,
        pass_name,
        buf: Vec::new(),
    };
    rloop.visit_body(stmts);
    if log_enabled!(Level::Trace) {
        trace!(
            "rewrite_with_pass end: {} stmts_len={}",
            pass_name,
            stmts.len()
        );
    }
}

struct RewriteLoop<'a, P: RewritePass> {
    buf: Vec<Stmt>,
    context: &'a Context,
    pass: &'a P,
    pass_name: &'static str,
}


impl<'a, P: RewritePass> RewriteLoop<'a, P> {
    fn process_statements(&mut self, initial: Vec<Stmt>) -> Vec<Stmt> {
        enum WorkItem {
            Process(Stmt),
            Emit(Stmt),
        }

        let mut buf_stack = take(&mut self.buf);
        buf_stack.extend(initial);

        let mut output = Vec::new();
        let mut worklist: VecDeque<WorkItem> =
            buf_stack.into_iter().map(WorkItem::Process).collect();

        let mut steps = 0usize;
        while let Some(item) = worklist.pop_front() {
            steps += 1;
            if log_enabled!(Level::Trace) {
                trace!(
                    "rewrite loop step {} pass={} worklist_len={}",
                    steps,
                    self.pass_name,
                    worklist.len()
                );
            }
            match item {
                WorkItem::Process(stmt) => {
                    let original_range = stmt.range();
                    if log_enabled!(Level::Trace) {
                        trace!(
                            "rewrite input (pass={} node_index={:?}): {}",
                            self.pass_name,
                            stmt.node_index(),
                            crate::ruff_ast_to_string(&stmt),
                        );
                    }

                    let res = self.pass.lower_stmt(self.context, stmt);

                    match res {
                        Rewrite::Visit(stmts) => {
                            if log_enabled!(Level::Trace) {
                                trace!(
                                    "rewrite output (Visit, pass={}): {}",
                                    self.pass_name,
                                    crate::ruff_ast_to_string(&stmts).trim_end()
                                );
                            }
                            let mut items: Vec<WorkItem> =
                                take(&mut self.buf).into_iter().map(WorkItem::Process).collect();
                            items.extend(stmts.into_iter().map(WorkItem::Process));
                            for item in items.into_iter().rev() {
                                worklist.push_front(item);
                            }
                        }
                        Rewrite::Walk(stmts) => {
                            if log_enabled!(Level::Trace) {
                                trace!(
                                    "rewrite output (Walk, pass={}): {}",
                                    self.pass_name,
                                    crate::ruff_ast_to_string(&stmts).trim_end()
                                );
                            }
                            let mut items: Vec<WorkItem> =
                                take(&mut self.buf).into_iter().map(WorkItem::Process).collect();
                            for mut stmt in stmts {
                                walk_stmt(self, &mut stmt);

                                items.extend(
                                    take(&mut self.buf)
                                        .into_iter()
                                        .map(WorkItem::Process),
                                );
                                items.push(WorkItem::Emit(stmt));
                            }
                            for item in items.into_iter().rev() {
                                worklist.push_front(item);
                            }
                        }
                    };
                }
                WorkItem::Emit(stmt) => {
                    output.push(stmt);
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
                let es = Stmt::Expr(ast::StmtExpr {
                    value: Box::new(current.clone()),
                    range: original_range,
                    node_index: ast::AtomicNodeIndex::default(),
                });
                trace!(
                    "lower_expr iteration {} pass={} input: {}",
                    iteration,
                    self.pass_name,
                    ruff_ast_to_string(&es).trim_end()
                );
            }
            lowered = self.pass.lower_expr(self.context, current);
            if log_enabled!(Level::Trace) {
                let es = Stmt::Expr(ast::StmtExpr {
                    value: Box::new(lowered.expr.clone()),
                    range: original_range,
                    node_index: ast::AtomicNodeIndex::default(),
                });
                trace!(
                    "lower_expr iteration {} pass={} output: {}",
                    iteration,
                    self.pass_name,
                    ruff_ast_to_string(&es).trim_end()
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
            if !matches!(
                current,
                Expr::Lambda(_)
                    | Expr::Generator(_)
                    | Expr::ListComp(_)
                    | Expr::SetComp(_)
                    | Expr::DictComp(_)
            ) {
                walk_expr(self, &mut current);
            }
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
    fn should_walk(&self, _stmt: &Stmt) -> bool {
        true
    }
}


fn apply_stmt_range(stmts: &mut [Stmt], range: TextRange) {
    for stmt in stmts {
        match stmt {
            Stmt::FunctionDef(node) => node.range = range,
            Stmt::ClassDef(node) => node.range = range,
            Stmt::Return(node) => node.range = range,
            Stmt::Delete(node) => node.range = range,
            Stmt::TypeAlias(node) => node.range = range,
            Stmt::Assign(node) => node.range = range,
            Stmt::AugAssign(node) => node.range = range,
            Stmt::AnnAssign(node) => node.range = range,
            Stmt::For(node) => node.range = range,
            Stmt::While(node) => node.range = range,
            Stmt::If(node) => node.range = range,
            Stmt::With(node) => node.range = range,
            Stmt::Match(node) => node.range = range,
            Stmt::Raise(node) => node.range = range,
            Stmt::Try(node) => node.range = range,
            Stmt::Assert(node) => node.range = range,
            Stmt::Import(node) => node.range = range,
            Stmt::ImportFrom(node) => node.range = range,
            Stmt::Global(node) => node.range = range,
            Stmt::Nonlocal(node) => node.range = range,
            Stmt::Expr(node) => node.range = range,
            Stmt::Pass(node) => node.range = range,
            Stmt::Break(node) => node.range = range,
            Stmt::Continue(node) => node.range = range,
            Stmt::IpyEscapeCommand(node) => node.range = range,
        }
    }
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

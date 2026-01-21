use super::{
    context::{Context, ScopeInfo, ScopeKind},
    rewrite_import, Options,
};
use crate::{ruff_ast_to_string, template::is_simple, transform::{rewrite_expr::lower_expr, rewrite_stmt}};

use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    transform::class_def,
};
use crate::{py_expr, py_stmt};
use log::{Level, log_enabled, trace};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::{Ranged, TextRange};
use std::collections::VecDeque;
use std::mem::take;

// TODO: rename RewriteContext, fold Context into it
pub struct ExprRewriter {
    ctx: Context,
    options: Options,
    buf: Vec<Stmt>,
    qualname_stack: Vec<(ScopeKind, String)>,
    stage: TransformStage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformStage {
    Stage1,
    Stage2,
}

pub(crate) enum Rewrite {
    Walk(Vec<Stmt>),
    Visit(Vec<Stmt>),
}

pub(crate) struct LoweredExpr {
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


impl ExprRewriter {
    pub fn new(ctx: Context) -> Self {
        Self {
            options: ctx.options,
            ctx,
            buf: Vec::new(),
            qualname_stack: Vec::new(),
            stage: TransformStage::Stage1,
        }
    }

    pub(super) fn context(&self) -> &Context {
        &self.ctx
    }

    pub fn set_stage(&mut self, stage: TransformStage) {
        self.stage = stage;
    }

    pub fn stage(&self) -> TransformStage {
        self.stage
    }


    pub(crate) fn rewrite_block(&mut self, body: Vec<Stmt>) -> Vec<Stmt> {
        self.process_statements(body)
    }

    pub(crate) fn fresh(&self, name: &str) -> String {
        self.ctx.fresh(name)
    }

    pub(crate) fn tmpify(&self, name: &str, expr: Expr) -> LoweredExpr {
        let tmp = self.ctx.fresh(name);
        let assign = py_stmt!(
            "{tmp:id} = {expr:expr}",
            tmp = tmp.as_str(),
            expr = expr
        );
        LoweredExpr::modified(py_expr!("{tmp:id}", tmp = tmp.as_str()), assign)
    }

    pub(crate) fn with_scope<F, R>(&mut self, scope: ScopeInfo, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.ctx.push_scope(scope);
        let result = f(self);
        self.ctx.pop_scope();
        result
    }

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

        while let Some(item) = worklist.pop_front() {
            match item {
                WorkItem::Process(stmt) => {
                    let original_range = stmt.range();
                    if log_enabled!(Level::Trace) {
                        trace!(
                            "rewrite input: {}",
                            crate::ruff_ast_to_string(std::slice::from_ref(&stmt)).trim_end()
                        );
                    }

                    let res = self.lower_stmt(stmt);

                    match res {
                        Rewrite::Visit(stmts) => {
                            if log_enabled!(Level::Trace) {
                                trace!(
                                    "rewrite output (Visit): {}",
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
                                    "rewrite output (Walk): {}",
                                    crate::ruff_ast_to_string(&stmts).trim_end()
                                );
                            }
                            let mut items: Vec<WorkItem> =
                                take(&mut self.buf).into_iter().map(WorkItem::Process).collect();
                            for mut stmt in stmts {
                                if !(self.stage == TransformStage::Stage1
                                    && matches!(stmt, Stmt::ClassDef(_)))
                                {
                                    walk_stmt(self, &mut stmt);
                                }
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

    pub(crate) fn maybe_placeholder_lowered(&mut self, expr: Expr) -> LoweredExpr {

        if is_simple(&expr) && !matches!(&expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_)) {
            return LoweredExpr::unmodified(expr);
        }

        self.tmpify("tmp", expr)
    }

    fn rewrite_function_def(&mut self, mut func_def: ast::StmtFunctionDef) -> Rewrite {
        let func_name = func_def.name.id.to_string();
        let scope = self.context().analyze_function_scope(&func_def);
   
        self.qualname_stack
            .push((ScopeKind::Function, func_name.clone()));
        func_def.body = self.with_scope(scope, |rewriter| {
            rewriter.rewrite_block(take(&mut func_def.body))
        });
        self.qualname_stack.pop();

        let decorators = take(&mut func_def.decorator_list);
        rewrite_stmt::decorator::rewrite(decorators, func_name.as_str(), vec![Stmt::FunctionDef(func_def)], self)
    }

    fn lower_stmt(&mut self, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::FunctionDef(func_def) => self.rewrite_function_def(func_def),
            Stmt::With(with) => rewrite_stmt::with::rewrite(with, self),
            Stmt::While(while_stmt) => rewrite_stmt::loop_::rewrite_while(while_stmt, self),
            Stmt::For(for_stmt) => rewrite_stmt::loop_::rewrite_for(for_stmt, self),
            Stmt::Assert(assert) => rewrite_stmt::assert::rewrite(assert),
            Stmt::ClassDef(class_def) if self.stage == TransformStage::Stage1 => {
                Rewrite::Walk(vec![Stmt::ClassDef(class_def)])
            }
            Stmt::ClassDef(class_def) => class_def::rewrite(class_def, self),
            Stmt::Try(try_stmt) => rewrite_stmt::exception::rewrite_try(try_stmt, &self.ctx),
            Stmt::If(if_stmt)
                if if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| clause.test.is_some()) =>
            {
                Rewrite::Visit(vec![expand_if_chain(if_stmt).into()])
            }
            Stmt::Match(match_stmt) => rewrite_stmt::match_case::rewrite(match_stmt, &self.ctx),
            Stmt::Import(import) => rewrite_import::rewrite(import, &self.options),
            Stmt::ImportFrom(import_from) => {
                rewrite_import::rewrite_from(import_from.clone(), &self.ctx, &self.options)
            }

            Stmt::AnnAssign(ann_assign) => rewrite_stmt::assign_del::rewrite_ann_assign(self, ann_assign),
            Stmt::Assign(assign) => rewrite_stmt::assign_del::rewrite_assign(self, assign),
            Stmt::AugAssign(aug) => rewrite_stmt::assign_del::rewrite_aug_assign(self, aug),
            Stmt::Delete(del) => rewrite_stmt::assign_del::rewrite_delete(self, del),
            Stmt::Raise(raise) => rewrite_stmt::exception::rewrite_raise(raise),
            other => Rewrite::Walk(vec![other]),
        }
    }
}

fn expand_if_chain(mut if_stmt: ast::StmtIf) -> ast::StmtIf {
    let mut else_body: Option<Vec<Stmt>> = None;

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

                else_body = Some(vec![Stmt::If(nested_if)]);
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

    if_stmt
}

impl Transformer for ExprRewriter {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let saved_buf = take(&mut self.buf);
        let stmts = take(body);
        *body = self.process_statements(stmts);
        self.buf = saved_buf;
    }

    fn visit_expr(&mut self, expr_input: &mut Expr) {
        if self.stage == TransformStage::Stage1
            && matches!(expr_input, Expr::Lambda(_) | Expr::Generator(_))
        {
            return;
        }

        let original_range = expr_input.range();
        let mut lowered: LoweredExpr;
        let mut current = expr_input.clone();
        let mut modified_any = false;
        loop {
            lowered = lower_expr(self, current);
            if log_enabled!(Level::Trace) {
                let es = Stmt::Expr(ast::StmtExpr {
                    value: Box::new(lowered.expr.clone()),
                    range: original_range,
                    node_index: ast::AtomicNodeIndex::default(),
                });
                trace!("lower_expr input: {}", ruff_ast_to_string(&[es]).trim_end());
            }

            let LoweredExpr { stmts, expr, modified } = lowered;
            self.buf.extend(stmts);
            current = expr;

            apply_expr_range(&mut current, original_range);
            if !modified {
                break;
            }
            modified_any = true;
        }

        if !modified_any {
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

use std::cell::Cell;

use ruff_python_ast::{self as ast, Expr, ModModule, Stmt};

use super::unnest_expr::{unnest_expr, unnest_exprs};
use super::Options;

pub struct Namer {
    pub counter: Cell<usize>,
}

impl Namer {
    pub fn new() -> Self {
        Self {
            counter: Cell::new(0),
        }
    }

    pub fn fresh(&self, prefix: &str) -> String {
        let id = self.counter.get();
        self.counter.set(id + 1);
        format!("{prefix}_{id}")
    }
}

pub struct Context {
    pub namer: Namer,
    pub options: Options,
}

pub fn lower_module(ctx: &Context, module: ModModule) -> ModModule {
    ModModule {
        body: lower_stmts(ctx, module.body),
        ..module
    }
}

pub fn lower_stmts(ctx: &Context, stmts: Vec<Stmt>) -> Vec<Stmt> {
    let mut result = Vec::new();
    for stmt in stmts {
        result.extend(lower_stmt(ctx, stmt));
    }
    result
}

fn unnest_expr_prepend(ctx: &Context, prepend: &mut Vec<Stmt>, expr: Expr) -> Expr {
    let (expr, mut stmts) = unnest_expr(ctx, expr);
    prepend.append(&mut stmts);
    expr
}

fn unnest_exprs_prepend(ctx: &Context, prepend: &mut Vec<Stmt>, exprs: Vec<Expr>) -> Vec<Expr> {
    let (exprs, mut stmts) = unnest_exprs(ctx, exprs);
    prepend.append(&mut stmts);
    exprs
}

pub fn walk_stmt(ctx: &Context, stmt: Stmt) -> Vec<Stmt> {
    let mut prepend = vec![];
    let stmt = match stmt {
        ast::Stmt::FunctionDef(mut s) => {
            s.body = lower_stmts(ctx, s.body);
            ast::Stmt::FunctionDef(s)
        }
        ast::Stmt::ClassDef(mut s) => {
            s.body = lower_stmts(ctx, s.body);
            ast::Stmt::ClassDef(s)
        }
        ast::Stmt::Return(mut s) => {
            if let Some(value) = s.value.take() {
                let value = unnest_expr_prepend(ctx, &mut prepend, *value);
                s.value = Some(Box::new(value));
            }
            ast::Stmt::Return(s)
        }
        ast::Stmt::Delete(mut s) => {
            s.targets = unnest_exprs_prepend(ctx, &mut prepend, s.targets);
            ast::Stmt::Delete(s)
        }
        ast::Stmt::TypeAlias(mut s) => {
            let name = unnest_expr_prepend(ctx, &mut prepend, *s.name);
            s.name = Box::new(name);
            s.value = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.value));
            ast::Stmt::TypeAlias(s)
        }
        ast::Stmt::Assign(mut s) => {
            let (value, mut stmts) = unnest_expr(ctx, *s.value);
            prepend.append(&mut stmts);
            s.value = Box::new(value);
            s.targets = unnest_exprs_prepend(ctx, &mut prepend, s.targets);
            ast::Stmt::Assign(s)
        }
        ast::Stmt::AugAssign(mut s) => {
            s.value = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.value));
            s.target = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.target));
            ast::Stmt::AugAssign(s)
        }
        ast::Stmt::AnnAssign(mut s) => {
            if let Some(value) = s.value.take() {
                let value = unnest_expr_prepend(ctx, &mut prepend, *value);
                s.value = Some(Box::new(value));
            }
            s.annotation = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.annotation));
            s.target = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.target));
            ast::Stmt::AnnAssign(s)
        }
        ast::Stmt::For(mut s) => {
            s.iter = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.iter));
            s.target = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.target));
            s.body = lower_stmts(ctx, s.body);
            s.orelse = lower_stmts(ctx, s.orelse);
            ast::Stmt::For(s)
        }
        ast::Stmt::While(mut s) => {
            s.test = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.test));
            s.body = lower_stmts(ctx, s.body);
            s.orelse = lower_stmts(ctx, s.orelse);
            ast::Stmt::While(s)
        }
        ast::Stmt::If(mut s) => {
            s.test = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.test));
            s.body = lower_stmts(ctx, s.body);
            for clause in &mut s.elif_else_clauses {
                if let Some(test) = clause.test.take() {
                    clause.test = Some(unnest_expr_prepend(ctx, &mut prepend, test));
                }
                clause.body = lower_stmts(ctx, std::mem::take(&mut clause.body));
            }
            ast::Stmt::If(s)
        }
        ast::Stmt::With(mut s) => {
            for item in &mut s.items {
                let context_expr = item.context_expr.clone();
                item.context_expr = unnest_expr_prepend(ctx, &mut prepend, context_expr);
                if let Some(vars) = item.optional_vars.take() {
                    item.optional_vars = Some(Box::new(unnest_expr_prepend(ctx, &mut prepend, *vars)));
                }
            }
            s.body = lower_stmts(ctx, s.body);
            ast::Stmt::With(s)
        }
        ast::Stmt::Match(mut s) => {
            s.subject = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.subject));
            for case in &mut s.cases {
                if let Some(guard) = case.guard.take() {
                    case.guard = Some(Box::new(unnest_expr_prepend(ctx, &mut prepend, *guard)));
                }
                case.body = lower_stmts(ctx, std::mem::take(&mut case.body));
            }
            ast::Stmt::Match(s)
        }
        ast::Stmt::Raise(mut s) => {
            if let Some(exc) = s.exc.take() {
                s.exc = Some(Box::new(unnest_expr_prepend(ctx, &mut prepend, *exc)));
            }
            if let Some(cause) = s.cause.take() {
                s.cause = Some(Box::new(unnest_expr_prepend(ctx, &mut prepend, *cause)));
            }
            ast::Stmt::Raise(s)
        }
        ast::Stmt::Try(mut s) => {
            s.body = lower_stmts(ctx, s.body);
            s.orelse = lower_stmts(ctx, s.orelse);
            s.finalbody = lower_stmts(ctx, s.finalbody);
            for ast::ExceptHandler::ExceptHandler(handler) in &mut s.handlers {
                if let Some(type_) = handler.type_.take() {
                    handler.type_ = Some(Box::new(unnest_expr_prepend(ctx, &mut prepend, *type_)));
                }
                handler.body = lower_stmts(ctx, std::mem::take(&mut handler.body));
            }
            ast::Stmt::Try(s)
        }
        ast::Stmt::Assert(mut s) => {
            s.test = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.test));
            if let Some(msg) = s.msg.take() {
                s.msg = Some(Box::new(unnest_expr_prepend(ctx, &mut prepend, *msg)));
            }
            ast::Stmt::Assert(s)
        }
        ast::Stmt::Import(s) => ast::Stmt::Import(s),
        ast::Stmt::ImportFrom(s) => ast::Stmt::ImportFrom(s),
        ast::Stmt::Global(s) => ast::Stmt::Global(s),
        ast::Stmt::Nonlocal(s) => ast::Stmt::Nonlocal(s),
        ast::Stmt::Expr(mut s) => {
            s.value = Box::new(unnest_expr_prepend(ctx, &mut prepend, *s.value));
            ast::Stmt::Expr(s)
        }
        ast::Stmt::Pass(s) => ast::Stmt::Pass(s),
        ast::Stmt::Break(s) => ast::Stmt::Break(s),
        ast::Stmt::Continue(s) => ast::Stmt::Continue(s),
        ast::Stmt::IpyEscapeCommand(s) => ast::Stmt::IpyEscapeCommand(s),
    };

    prepend.push(stmt);
    prepend
}

pub fn lower_stmt(ctx: &Context, stmt: Stmt) -> Vec<Stmt> {
    walk_stmt(ctx, stmt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::assert_ast_eq;
    use ruff_python_parser::parse_module;

    #[test]
    fn lowers_binop_expr() {
        let input = r#"
a = (1 + 2) + (3 + 4)
"#;
        let module = parse_module(input).unwrap().into_syntax();
        let ctx = Context {
            namer: Namer::new(),
            options: Options::for_test(),
        };
        let lowered = lower_module(&ctx, module);
        let expected = r#"
_dp_tmp_0 = 1 + 2
_dp_tmp_1 = 3 + 4
_dp_tmp_2 = _dp_tmp_0 + _dp_tmp_1
a = _dp_tmp_2
"#;
        let expected = parse_module(expected).unwrap().into_syntax();
        assert_ast_eq(&lowered.body, &expected.body);
    }
}

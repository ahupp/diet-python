use super::*;

pub(super) fn is_module_init_temp_name(name: &str) -> bool {
    name == "_dp_module_init" || name.starts_with("_dp_fn__dp_module_init_")
}

pub(super) struct BasicBlockSupportChecker {
    pub(super) supported: bool,
    pub(super) loop_depth: usize,
    pub(super) allow_await: bool,
}

impl Default for BasicBlockSupportChecker {
    fn default() -> Self {
        Self {
            supported: true,
            loop_depth: 0,
            allow_await: false,
        }
    }
}

impl BasicBlockSupportChecker {
    fn mark_unsupported(&mut self) {
        self.supported = false;
    }

    fn panic_stmt(&self, message: &str, stmt: &Stmt) -> ! {
        let rendered = crate::ruff_ast_to_string(stmt);
        panic!(
            "BB lowering invariant violated: {message}\nstmt:\n{}",
            rendered.trim_end()
        );
    }
}

impl Transformer for BasicBlockSupportChecker {
    fn visit_body(&mut self, body: &mut StmtBody) {
        if !self.supported {
            return;
        }
        if has_dead_stmt_after_terminator(body) {
            self.mark_unsupported();
            return;
        }
        walk_stmt_body(self, body);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if !self.supported {
            return;
        }
        match stmt {
            Stmt::Expr(_)
            | Stmt::Pass(_)
            | Stmt::Assign(_)
            | Stmt::Delete(_)
            | Stmt::Return(_)
            | Stmt::Raise(_) => {
                walk_stmt(self, stmt);
            }
            Stmt::FunctionDef(_) => {
                // A nested function definition is executable as a linear
                // statement in the parent CFG. We intentionally don't inspect
                // its body here; nested-function support is validated when the
                // nested function itself is visited for BB lowering.
            }
            Stmt::BodyStmt(_) => walk_stmt(self, stmt),
            Stmt::If(if_stmt) => {
                if if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| clause.test.is_some())
                {
                    self.panic_stmt("`elif` chain reached support checker", stmt);
                }
                walk_stmt(self, stmt);
            }
            Stmt::While(while_stmt) => {
                self.visit_expr(while_stmt.test.as_mut());
                self.loop_depth += 1;
                self.visit_body(&mut while_stmt.body);
                self.loop_depth -= 1;
                self.visit_body(&mut while_stmt.orelse);
            }
            Stmt::For(for_stmt) => {
                if for_stmt.is_async && !self.allow_await {
                    self.mark_unsupported();
                    return;
                }
                self.visit_expr(for_stmt.iter.as_mut());
                self.loop_depth += 1;
                self.visit_body(&mut for_stmt.body);
                self.loop_depth -= 1;
                self.visit_body(&mut for_stmt.orelse);
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(&mut try_stmt.body);
                for handler in try_stmt.handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_.as_mut());
                    }
                    self.visit_body(&mut handler.body);
                }
                self.visit_body(&mut try_stmt.orelse);
                self.visit_body(&mut try_stmt.finalbody);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {
                if self.loop_depth == 0 {
                    self.panic_stmt(
                        "`break`/`continue` outside loop reached support checker",
                        stmt,
                    );
                }
            }
            _ => self.panic_stmt("unsupported statement kind reached support checker", stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if !self.supported {
            return;
        }
        match expr {
            Expr::Await(_) => {
                if !self.allow_await {
                    self.mark_unsupported();
                    return;
                }
            }
            Expr::Yield(_) | Expr::YieldFrom(_) => {
                self.mark_unsupported();
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

fn has_dead_stmt_after_terminator(body: &StmtBody) -> bool {
    let mut terminated = false;
    for stmt in &body.body {
        if terminated {
            return true;
        }
        terminated = matches!(
            stmt.as_ref(),
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        );
    }
    false
}

pub(super) fn has_dead_stmt_suffixes(stmts: &[Box<Stmt>]) -> bool {
    let mut terminated = false;
    for stmt in stmts {
        let stmt = stmt.as_ref();
        if terminated {
            return true;
        }
        if has_dead_stmt_suffixes_in_stmt(stmt) {
            return true;
        }
        if matches!(
            stmt,
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        ) {
            terminated = true;
        }
    }
    false
}

fn has_dead_stmt_suffixes_in_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::BodyStmt(body) => has_dead_stmt_suffixes(&body.body),
        Stmt::If(if_stmt) => {
            has_dead_stmt_suffixes(&if_stmt.body.body)
                || if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| has_dead_stmt_suffixes(&clause.body.body))
        }
        Stmt::While(while_stmt) => {
            has_dead_stmt_suffixes(&while_stmt.body.body)
                || has_dead_stmt_suffixes(&while_stmt.orelse.body)
        }
        Stmt::For(for_stmt) => {
            has_dead_stmt_suffixes(&for_stmt.body.body)
                || has_dead_stmt_suffixes(&for_stmt.orelse.body)
        }
        Stmt::Try(try_stmt) => {
            has_dead_stmt_suffixes(&try_stmt.body.body)
                || try_stmt.handlers.iter().any(|handler| {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    has_dead_stmt_suffixes(&handler.body.body)
                })
                || has_dead_stmt_suffixes(&try_stmt.orelse.body)
                || has_dead_stmt_suffixes(&try_stmt.finalbody.body)
        }
        _ => false,
    }
}

pub(super) fn prune_dead_stmt_suffixes(stmts: &[Box<Stmt>]) -> Vec<Box<Stmt>> {
    let mut out = Vec::new();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        prune_dead_stmt_suffixes_in_stmt(&mut stmt);
        let terminates = matches!(
            stmt,
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        );
        out.push(Box::new(stmt));
        if terminates {
            break;
        }
    }
    out
}

fn prune_dead_stmt_suffixes_in_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::BodyStmt(body) => {
            body.body = prune_dead_stmt_suffixes(&body.body);
        }
        Stmt::If(if_stmt) => {
            if_stmt.body.body = prune_dead_stmt_suffixes(&if_stmt.body.body);
            for clause in &mut if_stmt.elif_else_clauses {
                clause.body.body = prune_dead_stmt_suffixes(&clause.body.body);
            }
        }
        Stmt::While(while_stmt) => {
            while_stmt.body.body = prune_dead_stmt_suffixes(&while_stmt.body.body);
            while_stmt.orelse.body = prune_dead_stmt_suffixes(&while_stmt.orelse.body);
        }
        Stmt::For(for_stmt) => {
            for_stmt.body.body = prune_dead_stmt_suffixes(&for_stmt.body.body);
            for_stmt.orelse.body = prune_dead_stmt_suffixes(&for_stmt.orelse.body);
        }
        Stmt::Try(try_stmt) => {
            try_stmt.body.body = prune_dead_stmt_suffixes(&try_stmt.body.body);
            for handler in &mut try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                handler.body.body = prune_dead_stmt_suffixes(&handler.body.body);
            }
            try_stmt.orelse.body = prune_dead_stmt_suffixes(&try_stmt.orelse.body);
            try_stmt.finalbody.body = prune_dead_stmt_suffixes(&try_stmt.finalbody.body);
        }
        _ => {}
    }
}

#[derive(Default)]
struct YieldLikeProbe {
    has_yield: bool,
    has_yield_from: bool,
    has_await: bool,
}

impl Transformer for YieldLikeProbe {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Yield(_) => self.has_yield = true,
            Expr::YieldFrom(_) => self.has_yield_from = true,
            Expr::Await(_) => self.has_await = true,
            _ => {}
        }
        walk_expr(self, expr);
    }
}

pub(super) fn has_yield_exprs_in_stmts(stmts: &[Box<Stmt>]) -> bool {
    let mut probe = YieldLikeProbe::default();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_yield || probe.has_yield_from {
            return true;
        }
    }
    false
}

pub(super) fn has_await_in_stmts(stmts: &[Box<Stmt>]) -> bool {
    let mut probe = YieldLikeProbe::default();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_await {
            return true;
        }
    }
    false
}

fn walk_stmt_body<V: Transformer + ?Sized>(visitor: &mut V, body: &mut StmtBody) {
    for stmt in body.body.iter_mut() {
        visitor.visit_stmt(stmt.as_mut());
    }
}

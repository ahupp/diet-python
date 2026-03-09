use ruff_python_ast::Expr;
use std::cell::RefCell;
use std::collections::HashSet;

use super::Options;
use crate::transform::scope::ScopeKind;

use crate::namegen::fresh_name;
use crate::template::is_simple;
use crate::transform::ast_rewrite::LoweredExpr;
use crate::{py_expr, py_stmt};

#[derive(Clone, Debug)]
pub struct ScopeFrame {
    pub kind: ScopeKind,
    pub in_async_function: bool,
    pub globals: HashSet<String>,
    pub nonlocals: HashSet<String>,
}

impl ScopeFrame {
    pub fn module() -> Self {
        Self {
            kind: ScopeKind::Module,
            in_async_function: false,
            globals: HashSet::new(),
            nonlocals: HashSet::new(),
        }
    }

    pub fn new(kind: ScopeKind, globals: HashSet<String>, nonlocals: HashSet<String>) -> Self {
        Self {
            kind,
            in_async_function: false,
            globals,
            nonlocals,
        }
    }
}

pub struct Context {
    pub options: Options,
    pub source: String,
    scope_stack: RefCell<Vec<ScopeFrame>>,
}

impl Context {
    pub fn new(options: Options, source: &str) -> Self {
        Self {
            options,
            source: source.to_string(),
            scope_stack: RefCell::new(vec![ScopeFrame::module()]),
        }
    }

    pub(crate) fn tmpify(&self, name: &str, expr: Expr) -> LoweredExpr {
        let tmp = fresh_name(name);
        let assign = py_stmt!("{tmp:id} = {expr:expr}", tmp = tmp.as_str(), expr = expr);
        LoweredExpr::modified(py_expr!("{tmp:id}", tmp = tmp.as_str()), assign)
    }

    pub(crate) fn maybe_placeholder_lowered(&self, expr: Expr) -> LoweredExpr {
        if is_simple(&expr) && !matches!(&expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_)) {
            return LoweredExpr::unmodified(expr);
        }

        self.tmpify("tmp", expr)
    }

    pub fn source_slice(&self, range: ruff_text_size::TextRange) -> Option<&str> {
        let start = range.start().to_usize();
        let end = range.end().to_usize();
        self.source.get(start..end)
    }

    pub fn line_number_at(&self, offset: usize) -> usize {
        self.source[..offset]
            .bytes()
            .filter(|&b| b == b'\n')
            .count()
            + 1
    }

    pub fn fresh(&self, name: &str) -> String {
        fresh_name(name)
    }

    pub fn reset_scope_stack(&self) {
        self.scope_stack.replace(vec![ScopeFrame::module()]);
    }

    pub fn push_scope(&self, frame: ScopeFrame) {
        self.scope_stack.borrow_mut().push(frame);
    }

    pub fn pop_scope(&self) {
        self.scope_stack.borrow_mut().pop();
    }

    pub fn current_scope(&self) -> ScopeFrame {
        self.scope_stack
            .borrow()
            .last()
            .cloned()
            .unwrap_or_else(ScopeFrame::module)
    }
}

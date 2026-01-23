use std::cell::{Cell};

use ruff_python_ast::{ Expr, Stmt};

use super::Options;

use crate::transform::ast_rewrite::LoweredExpr;
use crate::{py_expr, py_stmt};
use crate::template::is_simple;

pub struct Namer {
    counter: Cell<usize>,
}

impl Namer {
    pub fn new() -> Self {
        Self {
            counter: Cell::new(0),
        }
    }

    pub fn fresh(&self, name: &str) -> String {
        let id = self.counter.get() + 1;
        self.counter.set(id);
        format!("_dp_{name}_{id}")
    }
}

pub struct Context {
    pub namer: Namer,
    pub options: Options,
    pub source: String,
}


impl Context {
    pub fn new(options: Options, source: &str) -> Self {
        Self {
            namer: Namer::new(),
            options,
            source: source.to_string(),
        }
    }

    pub(crate) fn tmpify(&self, name: &str, expr: Expr) -> LoweredExpr {
        let tmp = self.namer.fresh(name);
        let assign = py_stmt!(
            "{tmp:id} = {expr:expr}",
            tmp = tmp.as_str(),
            expr = expr
        );
        LoweredExpr::modified(py_expr!("{tmp:id}", tmp = tmp.as_str()), assign)
    }


    pub(crate) fn named_placeholder(&self, expr: Expr) -> (String, Vec<Stmt>) {

        let name = self.fresh("tmp");
        let assign = py_stmt!(
            "{name:id} = {expr:expr}",
            name = name.as_str(),
            expr = expr
        );
        (name, assign)
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

    pub fn fresh(&self, name: &str) -> String {
        self.namer.fresh(name)
    }


}

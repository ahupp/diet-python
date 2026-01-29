use ruff_python_ast::Expr;
use std::cell::Cell;

use super::Options;

use crate::transform::ast_rewrite::LoweredExpr;
use crate::{py_expr, py_stmt};
use crate::template::is_simple;
use crate::namegen::fresh_name;

pub struct Context {
    pub options: Options,
    pub source: String,
    needs_typing_import: Cell<bool>,
    needs_templatelib_import: Cell<bool>,
}


impl Context {
    pub fn new(options: Options, source: &str) -> Self {
        Self {
            options,
            source: source.to_string(),
            needs_typing_import: Cell::new(false),
            needs_templatelib_import: Cell::new(false),
        }
    }

    pub(crate) fn tmpify(&self, name: &str, expr: Expr) -> LoweredExpr {
        let tmp = fresh_name(name);
        let assign = py_stmt!(
            "{tmp:id} = {expr:expr}",
            tmp = tmp.as_str(),
            expr = expr
        );
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

    pub fn fresh(&self, name: &str) -> String {
        fresh_name(name)
    }

    pub fn require_typing_import(&self) {
        self.needs_typing_import.set(true);
    }

    pub fn require_templatelib_import(&self) {
        self.needs_templatelib_import.set(true);
    }

    pub fn needs_typing_import(&self) -> bool {
        self.needs_typing_import.get()
    }

    pub fn needs_templatelib_import(&self) -> bool {
        self.needs_templatelib_import.get()
    }

}

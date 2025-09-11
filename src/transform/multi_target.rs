use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::Stmt;

/// Desugar assignments and deletions with multiple targets into simpler forms.
pub struct MultiTargetRewriter {
    tmp_count: Cell<usize>,
}

impl MultiTargetRewriter {
    pub fn new() -> Self {
        Self {
            tmp_count: Cell::new(0),
        }
    }

    fn next_tmp(&self) -> String {
        let id = self.tmp_count.get() + 1;
        self.tmp_count.set(id);
        format!("_dp_tmp_{}", id)
    }
}

impl Transformer for MultiTargetRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
        match stmt {
            Stmt::Assign(assign) => {
                if assign.targets.len() > 1 {
                    let tmp_name = self.next_tmp();
                    let value = (*assign.value).clone();
                    let tmp_expr = crate::py_expr!("{name:id}", name = tmp_name.as_str());

                    let mut stmts = Vec::with_capacity(assign.targets.len() + 1);
                    stmts.push(crate::py_stmt!(
                        "{name:id} = {value:expr}",
                        name = tmp_name.as_str(),
                        value = value,
                    ));
                    for target in &assign.targets {
                        stmts.push(crate::py_stmt!(
                            "{target:expr} = {tmp:expr}",
                            target = target.clone(),
                            tmp = tmp_expr.clone(),
                        ));
                    }
                    *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
                }
            }
            Stmt::Delete(del) => {
                if del.targets.len() > 1 {
                    let mut stmts = Vec::with_capacity(del.targets.len());
                    for target in &del.targets {
                        stmts.push(crate::py_stmt!(
                            "del {target:expr}",
                            target = target.clone()
                        ));
                    }
                    *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
                }
            }
            Stmt::Global(global) => {
                if global.names.len() > 1 {
                    let mut stmts = Vec::with_capacity(global.names.len());
                    for name in &global.names {
                        stmts.push(crate::py_stmt!("global {name:id}", name = name.as_str()));
                    }
                    *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
                }
            }
            Stmt::Nonlocal(nonlocal) => {
                if nonlocal.names.len() > 1 {
                    let mut stmts = Vec::with_capacity(nonlocal.names.len());
                    for name in &nonlocal.names {
                        stmts.push(crate::py_stmt!("nonlocal {name:id}", name = name.as_str()));
                    }
                    *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = MultiTargetRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn desugars_chain_assignment() {
        let output = rewrite("a = b = c");
        let expected = "_dp_tmp_1 = c\na = _dp_tmp_1\nb = _dp_tmp_1";
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn desugars_multi_delete() {
        let output = rewrite("del a, b");
        let expected = "del a\ndel b";
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn desugars_multi_global() {
        let output = rewrite("global a, b");
        let expected = "global a\nglobal b";
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn desugars_multi_nonlocal() {
        let output = rewrite("def f():\n    nonlocal a, b");
        let expected = "def f():\n    nonlocal a\n    nonlocal b";
        assert_flatten_eq!(output, expected);
    }
}

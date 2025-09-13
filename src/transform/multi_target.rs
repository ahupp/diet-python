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
        let output = rewrite(
            r#"
a = b = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = _dp_tmp_1
b = _dp_tmp_1
"#;
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn desugars_multi_delete() {
        let output = rewrite(
            r#"
del a, b
"#,
        );
        let expected = r#"
del a
del b
"#;
        assert_flatten_eq!(output, expected);
    }
}

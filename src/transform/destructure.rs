use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};

/// Desugar destructuring assignments like `a, b = value` or `[a, b] = value`.
pub struct DestructureRewriter {
    tmp_count: Cell<usize>,
}

impl DestructureRewriter {
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

impl Transformer for DestructureRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
        if let Stmt::Assign(assign) = stmt {
            assert_eq!(
                assign.targets.len(),
                1,
                "multi-target rewriting must run first"
            );
            match &assign.targets[0] {
                Expr::Tuple(tuple) => {
                    let tmp_name = self.next_tmp();
                    let value = (*assign.value).clone();
                    let tmp_expr = crate::py_expr!("{name:id}", name = tmp_name.as_str());
                    let mut stmts = Vec::with_capacity(tuple.elts.len() + 1);
                    stmts.push(crate::py_stmt!(
                        "{name:id} = {value:expr}",
                        name = tmp_name.as_str(),
                        value = value,
                    ));
                    for (i, elt) in tuple.elts.iter().enumerate() {
                        stmts.push(crate::py_stmt!(
                            "{target:expr} = {tmp:expr}[{idx:literal}]",
                            target = elt.clone(),
                            tmp = tmp_expr.clone(),
                            idx = i,
                        ));
                    }
                    *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
                }
                Expr::List(list) => {
                    let tmp_name = self.next_tmp();
                    let value = (*assign.value).clone();
                    let tmp_expr = crate::py_expr!("{name:id}", name = tmp_name.as_str());
                    let mut stmts = Vec::with_capacity(list.elts.len() + 1);
                    stmts.push(crate::py_stmt!(
                        "{name:id} = {value:expr}",
                        name = tmp_name.as_str(),
                        value = value,
                    ));
                    for (i, elt) in list.elts.iter().enumerate() {
                        stmts.push(crate::py_stmt!(
                            "{target:expr} = {tmp:expr}[{idx:literal}]",
                            target = elt.clone(),
                            tmp = tmp_expr.clone(),
                            idx = i,
                        ));
                    }
                    *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
                }
                Expr::Name(_) | Expr::Attribute(_) | Expr::Subscript(_) => {}
                // Only some expressions are valid assignment targets; everything else
                // results in a panic.
                _ => {
                    panic!("unsupported assignment target");
                }
            }
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
        let rewriter = DestructureRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn desugars_tuple_unpacking() {
        let output = rewrite(
            r#"
a, b = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = _dp_tmp_1[0]
b = _dp_tmp_1[1]
"#;
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn desugars_list_unpacking() {
        let output = rewrite(
            r#"
[a, b] = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = _dp_tmp_1[0]
b = _dp_tmp_1[1]
"#;
        assert_flatten_eq!(output, expected);
    }
}

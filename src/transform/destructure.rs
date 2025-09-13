use std::cell::Cell;

use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{Expr, Stmt};

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
                Expr::Attribute(attr) => {
                    let obj = (*attr.value).clone();
                    let value = (*assign.value).clone();
                    *stmt = crate::py_stmt!(
                        "__dp__.setattr({obj:expr}, {name:literal}, {value:expr})",
                        obj = obj,
                        name = attr.attr.as_str(),
                        value = value,
                    );
                }
                Expr::Subscript(sub) => {
                    let obj = (*sub.value).clone();
                    let key = (*sub.slice).clone();
                    let value = (*assign.value).clone();
                    *stmt = crate::py_stmt!(
                        "__dp__.setitem({obj:expr}, {key:expr}, {value:expr})",
                        obj = obj,
                        key = key,
                        value = value,
                    );
                }
                Expr::Name(_) => {}
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
    use crate::transform::multi_target::MultiTargetRewriter;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let multi = MultiTargetRewriter::new();
        walk_body(&multi, &mut module.body);
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

    #[test]
    fn rewrites_attribute_assignment() {
        let output = rewrite(
            r#"
a.b = c
"#,
        );
        let expected = r#"
__dp__.setattr(a, "b", c)
"#;
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_subscript_assignment() {
        let output = rewrite(
            r#"
a[b] = c
"#,
        );
        let expected = r#"
__dp__.setitem(a, b, c)
"#;
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_chain_assignment_with_subscript() {
        let output = rewrite(
            r#"
a[0] = b = 1
"#,
        );
        let expected = r#"
_dp_tmp_1 = 1
__dp__.setitem(a, 0, _dp_tmp_1)
b = _dp_tmp_1
"#;
        assert_flatten_eq!(output, expected);
    }
}

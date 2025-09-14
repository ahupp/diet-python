use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Stmt};

pub struct ImportRewriter;

impl ImportRewriter {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for ImportRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
        match stmt {
            Stmt::Import(ast::StmtImport { names, .. }) => {
                let mut stmts = Vec::new();
                for alias in names {
                    let module_name = alias.name.id.to_string();
                    let binding = alias
                        .asname
                        .as_ref()
                        .map(|n| n.id.as_str())
                        .unwrap_or_else(|| module_name.split('.').next().unwrap());
                    let assign = crate::py_stmt!(
                        "
{name:id} = __dp__.import_({module:literal}, __spec__)
",
                        name = binding,
                        module = module_name.as_str(),
                    );
                    stmts.push(assign);
                }
                *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
            }
            Stmt::ImportFrom(ast::StmtImportFrom {
                module,
                names,
                level,
                ..
            }) => {
                if names.iter().any(|alias| alias.name.id.as_str() == "*") {
                    return;
                }
                let module_name = module.as_ref().map(|n| n.id.as_str()).unwrap_or("");
                let level_val = *level;
                let mut stmts = Vec::new();
                for alias in names {
                    let orig = alias.name.id.as_str();
                    let binding = alias.asname.as_ref().map(|n| n.id.as_str()).unwrap_or(orig);
                    let assign = if level_val == 0 {
                        crate::py_stmt!(
                            "
{name:id} = __dp__.import_({module:literal}, __spec__, [{orig:literal}]).{attr:id}
",
                            name = binding,
                            module = module_name,
                            orig = orig,
                            attr = orig,
                        )
                    } else {
                        crate::py_stmt!(
                            "
{name:id} = __dp__.import_({module:literal}, __spec__, [{orig:literal}], {level:id}).{attr:id}
",
                            name = binding,
                            module = module_name,
                            orig = orig,
                            level = level_val.to_string(),
                            attr = orig,
                        )
                    };
                    stmts.push(assign);
                }
                *stmt = crate::py_stmt!("{body:stmt}", body = stmts);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensure_import::ensure_import;
    use crate::test_util::assert_transform_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    #[test]
    fn rewrites_basic_import() {
        let input = r#"
import a
"#;
        let expected = r#"
a = __dp__.import_("a", __spec__)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_from_import() {
        let input = r#"
from a.b import c
"#;

        let expected = r#"
c = __dp__.import_("a.b", __spec__, ["c"]).c
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_relative_import() {
        let input = r#"
            r#"
from ..a import b
"#;
        let expected = r#"
b = __dp__.import_("a", __spec__, ["b"], 2).b
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn inserts_after_future_and_docstring() {
        let input = r#"
            r#"
"doc"
from __future__ import annotations
x = 1
"#;
        assert_transform_eq(
            input,
            r#"
"doc"
from __future__ import annotations
import __dp__
x = 1
"#,
        );
    }
}

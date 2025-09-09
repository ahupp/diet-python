use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Stmt};

pub fn ensure_import(module: &mut ast::ModModule, name: &str) {
    let has_import = module.body.iter().any(|stmt| {
        if let Stmt::Import(ast::StmtImport { names, .. }) = stmt {
            names.iter().any(|alias| alias.name.id.as_str() == name)
        } else {
            false
        }
    });

    if !has_import {
        let import = crate::py_stmt!("import {name:id}", name = name);
        module.body.insert(0, import);
    }
}

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
                if names
                    .iter()
                    .any(|alias| alias.name.id.as_str() == "dp_intrinsics")
                {
                    return;
                }
                let mut stmts = Vec::new();
                for alias in names {
                    let module_name = alias.name.id.to_string();
                    let binding = alias
                        .asname
                        .as_ref()
                        .map(|n| n.id.as_str())
                        .unwrap_or_else(|| module_name.split('.').next().unwrap());
                    let assign = crate::py_stmt!(
                        "{name:id} = dp_intrinsics.import_({module:literal}, __spec__)",
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
                            "{name:id} = dp_intrinsics.import_({module:literal}, __spec__, [{orig:literal}]).{attr:id}",
                            name = binding,
                            module = module_name,
                            orig = orig,
                            attr = orig,
                        )
                    } else {
                        crate::py_stmt!(
                            "{name:id} = dp_intrinsics.import_({module:literal}, __spec__, [{orig:literal}], {level:id}).{attr:id}",
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
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = ImportRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_basic_import() {
        let output = rewrite("import a");
        let expected = "a = dp_intrinsics.import_(\"a\", __spec__)";
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_from_import() {
        let output = rewrite("from a.b import c");
        let expected = "c = dp_intrinsics.import_(\"a.b\", __spec__, [\"c\"]).c";
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_relative_import() {
        let output = rewrite("from ..a import b");
        let expected = "b = dp_intrinsics.import_(\"a\", __spec__, [\"b\"], 2).b";
        assert_flatten_eq!(output, expected);
    }
}

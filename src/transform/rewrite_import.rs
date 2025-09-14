use super::Options;
use ruff_python_ast::{self as ast, Stmt};

pub fn rewrite(ast::StmtImport { names, .. }: ast::StmtImport) -> Stmt {
    let mut stmts = Vec::new();
    for alias in names {
        let module_name = alias.name.id.to_string();
        let binding = alias
            .asname
            .as_ref()
            .map(|n| n.id.as_str())
            .unwrap_or_else(|| module_name.split('.').next().unwrap());
        let assign = crate::py_stmt!(
            "{name:id} = __dp__.import_({module:literal}, __spec__)",
            name = binding,
            module = module_name.as_str(),
        );
        stmts.push(assign);
    }
    crate::py_stmt!("{body:stmt}", body = stmts)
}

pub fn rewrite_from(
    ast::StmtImportFrom {
        module,
        names,
        level,
        ..
    }: ast::StmtImportFrom,
    options: &Options,
) -> Option<Stmt> {
    if names.iter().any(|alias| alias.name.id.as_str() == "*") {
        if options.allow_import_star {
            return None;
        }
        panic!("import star not allowed");
    }
    let module_name = module.as_ref().map(|n| n.id.as_str()).unwrap_or("");
    let level_val = level;
    let mut stmts = Vec::new();
    for alias in names {
        let orig = alias.name.id.as_str();
        let binding = alias.asname.as_ref().map(|n| n.id.as_str()).unwrap_or(orig);
        let assign = if level_val == 0 {
            crate::py_stmt!(
                "{name:id} = __dp__.import_({module:literal}, __spec__, [{orig:literal}]).{attr:id}",
                name = binding,
                module = module_name,
                orig = orig,
                attr = orig,
            )
        } else {
            crate::py_stmt!(
                "{name:id} = __dp__.import_({module:literal}, __spec__, [{orig:literal}], {level:id}).{attr:id}",
                name = binding,
                module = module_name,
                orig = orig,
                level = level_val.to_string(),
                attr = orig,
            )
        };
        stmts.push(assign);
    }
    Some(crate::py_stmt!("{body:stmt}", body = stmts))
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;
    use crate::transform::{expr::ExprRewriter, Options};
    use ruff_python_parser::parse_module;

    #[test]
    fn rewrites_basic_import() {
        let input = r#"
import a
"#;
        let expected = r#"
a = getattr(__dp__, "import_")("a", __spec__)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_from_import() {
        let input = r#"
from a.b import c
"#;

        let expected = r#"
c = getattr(getattr(__dp__, "import_")("a.b", __spec__, list(("c",))), "c")
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_relative_import() {
        let input = r#"
from ..a import b
"#;
        let expected = r#"
b = getattr(getattr(__dp__, "import_")("a", __spec__, list(("b",)), 2), "b")
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn inserts_after_future_and_docstring() {
        let input = r#"
"doc"
from __future__ import annotations
x = 1
"#;
        assert_transform_eq(
            input,
            r#"
"doc"
annotations = getattr(getattr(__dp__, "import_")("__future__", __spec__, list(("annotations",))), "annotations")
x = 1
"#,
        );
    }

    fn rewrite_source(source: &str, options: Options) -> String {
        let mut module = parse_module(source).expect("parse error").into_syntax();
        let expr_transformer = ExprRewriter::new(options);
        expr_transformer.rewrite_body(&mut module.body);
        crate::template::flatten(&mut module.body);
        crate::ruff_ast_to_string(&module.body)
    }

    #[test]
    fn allows_import_star() {
        let input = r#"
from a import *
"#;
        let output = rewrite_source(
            input,
            Options {
                allow_import_star: true,
                inject_import: false,
            },
        );
        assert_eq!(output.trim(), "from a import *");
    }

    #[test]
    #[should_panic(expected = "import star not allowed")]
    fn panics_on_import_star() {
        let input = r#"
from a import *
"#;
        let _ = rewrite_source(
            input,
            Options {
                allow_import_star: false,
                inject_import: false,
            },
        );
    }
}

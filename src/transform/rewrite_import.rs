use crate::py_stmt;

use super::{ImportStarHandling, Options};
use ruff_python_ast::{self as ast, Stmt};

pub fn should_rewrite_import_from(import_from: &ast::StmtImportFrom, options: &Options) -> bool {
    if import_from
        .names
        .iter()
        .any(|alias| alias.name.id.as_str() == "*")
    {
        !matches!(options.import_star_handling, ImportStarHandling::Allowed)
    } else {
        true
    }
}

pub fn rewrite(ast::StmtImport { names, .. }: ast::StmtImport) -> Vec<Stmt> {
    names
        .into_iter()
        .map(|alias| {
            let module_name = alias.name.id.to_string();
            let binding = alias
                .asname
                .as_ref()
                .map(|n| n.id.as_str())
                .unwrap_or_else(|| module_name.split('.').next().unwrap());
            py_stmt!(
                "{name:id} = __dp__.import_({module:literal}, __spec__)",
                name = binding,
                module = module_name.as_str(),
            )
        })
        .flatten()
        .collect()
}

pub fn rewrite_from(import_from: ast::StmtImportFrom, options: &Options) -> Vec<Stmt> {
    debug_assert!(should_rewrite_import_from(&import_from, options));

    let ast::StmtImportFrom {
        module,
        names,
        level,
        ..
    } = import_from;

    if names.iter().any(|alias| alias.name.id.as_str() == "*") {
        return match options.import_star_handling {
            ImportStarHandling::Allowed => {
                unreachable!("rewrite_from is only called when import-star rewriting is required")
            }
            ImportStarHandling::Error => panic!("import star not allowed"),
            ImportStarHandling::Strip => vec![],
        };
    }
    let module_name = module.as_ref().map(|n| n.id.as_str()).unwrap_or("");
    let level_val = level.to_string();
    names.into_iter().map(|alias| {
        let orig = alias.name.id.as_str();
        let binding = alias.asname.as_ref().map(|n| n.id.as_str()).unwrap_or(orig);
        py_stmt!(
                "{name:id} = __dp__.import_({module:literal}, __spec__, [{orig:literal}], {level:id}).{attr:id}",
                name = binding,
                module = module_name,
                orig = orig,
                level = level_val.as_str(),
                attr = orig,
            )

    }).flatten().collect()
}

#[cfg(test)]
mod tests {
    use crate::body_transform::Transformer;
    use crate::transform::{context::Context, expr::ExprRewriter, ImportStarHandling, Options};
    use ruff_python_parser::parse_module;

    crate::transform_fixture_test!("tests_rewrite_import.txt");

    fn rewrite_source(source: &str, options: Options) -> String {
        let mut module = parse_module(source).expect("parse error").into_syntax();
        let ctx = Context::new(options);
        let mut expr_transformer = ExprRewriter::new(&ctx);
        expr_transformer.visit_body(&mut module.body);
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
                import_star_handling: ImportStarHandling::Allowed,
                ..Options::for_test()
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
                import_star_handling: ImportStarHandling::Error,
                ..Options::for_test()
            },
        );
    }

    #[test]
    fn strips_import_star() {
        let input = r#"
from a import *
"#;
        let output = rewrite_source(
            input,
            Options {
                import_star_handling: ImportStarHandling::Strip,
                ..Options::for_test()
            },
        );
        assert_eq!(output.trim(), "");
    }
}

use crate::py_stmt;

use super::{context::Context, driver::Rewrite, ImportStarHandling, Options};
use ruff_python_ast::{self as ast, Stmt};
use ruff_python_parser::parse_module;

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

pub fn rewrite(ast::StmtImport { names, .. }: ast::StmtImport) -> Rewrite {
    Rewrite::Visit(
        names
            .into_iter()
            .map(|alias| {
                let module_name = alias.name.id.to_string();
                let binding = alias
                    .asname
                    .as_ref()
                    .map(|n| n.id.as_str())
                    .unwrap_or_else(|| module_name.split('.').next().unwrap());
                let needs_fromlist = alias.asname.is_some() && module_name.contains('.');
                if needs_fromlist {
                    let attr = module_name
                        .rsplit_once('.')
                        .map(|(_, last)| last)
                        .unwrap_or(module_name.as_str());
                    py_stmt!(
                        "{name:id} = __dp__.import_({module:literal}, __spec__, __dp__.list(({attr:literal},)))",
                        name = binding,
                        module = module_name.as_str(),
                        attr = attr,
                    )
                } else {
                    py_stmt!(
                        "{name:id} = __dp__.import_({module:literal}, __spec__)",
                        name = binding,
                        module = module_name.as_str(),
                    )
                }
            })
            .flatten()
            .collect(),
    )
}

pub fn rewrite_from(import_from: ast::StmtImportFrom, ctx: &Context, options: &Options) -> Rewrite {
    if !should_rewrite_import_from(&import_from, options) {
        return Rewrite::Walk(vec![Stmt::ImportFrom(import_from)]);
    }

    let ast::StmtImportFrom {
        module,
        names,
        level,
        ..
    } = import_from;

    if names.iter().any(|alias| alias.name.id.as_str() == "*") {
        return Rewrite::Visit(match options.import_star_handling {
            ImportStarHandling::Allowed => {
                unreachable!("rewrite_from is only called when import-star rewriting is required")
            }
            ImportStarHandling::Error => panic!("import star not allowed"),
            ImportStarHandling::Strip => vec![],
        });
    }
    let module_name = module.as_ref().map(|n| n.id.as_str()).unwrap_or("");
    let temp_binding = ctx.fresh("import");
    let mut statements = Vec::new();

    let fromlist: Vec<String> = names
        .iter()
        .map(|alias| format!("{:?}", alias.name.id.as_str()))
        .collect();
    let fromlist_literal = format!("[{}]", fromlist.join(", "));
    let module_literal = format!("{:?}", module_name);
    let import_stmt_source = if level > 0 {
        format!(
            "{tmp} = __dp__.import_({module}, __spec__, {fromlist}, {level})",
            tmp = temp_binding,
            module = module_literal,
            fromlist = fromlist_literal,
            level = level
        )
    } else {
        format!(
            "{tmp} = __dp__.import_({module}, __spec__, {fromlist})",
            tmp = temp_binding,
            module = module_literal,
            fromlist = fromlist_literal
        )
    };

    let mut import_stmt = parse_module(import_stmt_source.as_str())
        .expect("failed to parse rewritten import")
        .into_syntax()
        .body;
    let import_stmt = import_stmt
        .pop()
        .expect("expected single statement when parsing import rewrite");
    statements.push(import_stmt);

    for alias in names {
        let orig = alias.name.id.as_str();
        let binding = alias.asname.as_ref().map(|n| n.id.as_str()).unwrap_or(orig);
        statements.extend(py_stmt!(
            "{name:id} = __dp__.import_attr({module:id}, {attr:literal})",
            name = binding,
            module = temp_binding.as_str(),
            attr = orig,
        ));
    }

    statements.extend(py_stmt!("del {module:id}", module = temp_binding.as_str()));

    Rewrite::Visit(statements)
}

#[cfg(test)]
mod tests {
    use crate::body_transform::Transformer;
    use crate::transform::{context::Context, driver::ExprRewriter, ImportStarHandling, Options};
    use ruff_python_parser::parse_module;

    crate::transform_fixture_test!("tests_rewrite_import.txt");

    fn rewrite_source(source: &str, options: Options) -> String {
        let mut module = parse_module(source).expect("parse error").into_syntax();
        let ctx = Context::new(options.clone());
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

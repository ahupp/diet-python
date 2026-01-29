use crate::{py_stmt, template::{empty_body, into_body}, transform::ast_rewrite::Rewrite};

use super::{context::Context, ImportStarHandling, Options};
use ruff_python_ast::{self as ast};
use ruff_python_parser::parse_module;

pub fn should_rewrite_import_from(import_from: &ast::StmtImportFrom, options: &Options) -> bool {
    let has_import_star = import_from
        .names
        .iter()
        .any(|alias| alias.name.id.as_str() == "*");
    if has_import_star && matches!(options.import_star_handling, ImportStarHandling::Allowed) {
        return false;
    }
    if options.force_import_rewrite {
        return true;
    }
    if import_from
        .module
        .as_ref()
        .is_some_and(|module| module.id.as_str() == "__future__")
    {
        return false;
    }
    if has_import_star {
        !matches!(options.import_star_handling, ImportStarHandling::Allowed)
    } else {
        true
    }
}

pub fn rewrite(ast::StmtImport { names, .. }: ast::StmtImport) -> Rewrite {
    // TODO: hard-coded "import _testinternalcapi"
    let stmts: Vec<ast::Stmt> = names
        .into_iter()
        .flat_map(|alias| {
            let module_name = alias.name.id.to_string();
            let binding = alias
                .asname
                .as_ref()
                .map(|n| n.id.as_str())
                .unwrap_or_else(|| module_name.split('.').next().unwrap());
                let needs_fromlist = alias.asname.is_some() && module_name.contains('.');
                if needs_fromlist {
                    let mut expr =
                        format!("__dp__.import_({:?}, __spec__)", module_name.as_str());
                    let mut parts = module_name.split('.').collect::<Vec<_>>();
                    if !parts.is_empty() {
                        parts.remove(0);
                    }
                    for part in parts {
                        expr = format!("__dp__.import_attr({}, {:?})", expr, part);
                    }
                    let stmt_source = format!("{binding} = {expr}", binding = binding, expr = expr);
                    let body = parse_module(stmt_source.as_str())
                        .expect("failed to parse rewritten dotted import")
                        .into_syntax()
                        .body;
                    body.body
                        .iter()
                        .map(|stmt| stmt.as_ref().clone())
                        .collect::<Vec<_>>()
                } else {
                    vec![py_stmt!(
                        "{name:id} = __dp__.import_({module:literal}, __spec__)",
                        name = binding,
                        module = module_name.as_str(),
                    )]
                }
            })
        .collect();
    Rewrite::Walk(into_body(stmts))
}

pub fn rewrite_from(context: &Context, import_from: ast::StmtImportFrom) -> Rewrite {
    if !should_rewrite_import_from(&import_from, &context.options) {
        return Rewrite::Unmodified(import_from.into());
    }

    let ast::StmtImportFrom {
        module,
        names,
        level,
        ..
    } = import_from;

    if names.iter().any(|alias| alias.name.id.as_str() == "*") {
        return Rewrite::Walk(match context.options.import_star_handling {
            ImportStarHandling::Allowed => {
                unreachable!("rewrite_from is only called when import-star rewriting is required")
            }
            ImportStarHandling::Error => panic!("import star not allowed"),
            ImportStarHandling::Strip => empty_body().into(),
        });
    }
    let module_name = module.as_ref().map(|n| n.id.as_str()).unwrap_or("");
    let temp_binding = context.fresh("import");
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
        .body
        .pop()
        .expect("expected single statement when parsing import rewrite");
    statements.push(*import_stmt);

    for alias in names {
        let orig = alias.name.id.as_str();
        let binding = alias.asname.as_ref().map(|n| n.id.as_str()).unwrap_or(orig);
        statements.push(py_stmt!(
            "{name:id} = __dp__.import_attr({module:id}, {attr:literal})",
            name = binding,
            module = temp_binding.as_str(),
            attr = orig,
        ));
    }

    Rewrite::Walk(into_body(statements))
}

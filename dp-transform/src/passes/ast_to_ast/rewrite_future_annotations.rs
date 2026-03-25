use crate::passes::ast_to_ast::body::Suite;
use crate::py_expr;
use crate::{passes::ast_to_ast::context::Context, transformer::Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;
use std::collections::HashSet;

pub fn rewrite(_context: &Context, body: &mut Suite) -> HashSet<String> {
    let mut rewriter = FutureAnnotationsRewriter::new();
    let future_features = rewriter.strip_future_imports(body);
    if future_features.contains("annotations") {
        (&mut rewriter).visit_body(body);
    }
    future_features
}

struct FutureAnnotationsRewriter {
    indent: Indentation,
}

impl FutureAnnotationsRewriter {
    fn new() -> Self {
        Self {
            indent: Indentation::new("    ".to_string()),
        }
    }

    fn strip_future_imports(&mut self, body: &mut Suite) -> HashSet<String> {
        let mut future_features = HashSet::new();
        let mut index = 0;
        while index < body.len() {
            let mut remove_stmt = false;
            if let Stmt::ImportFrom(import_from) = &mut body[index] {
                if is_future_import(import_from) {
                    future_features.extend(
                        import_from
                            .names
                            .iter()
                            .map(|alias| alias.name.id.to_string()),
                    );
                    remove_stmt = true;
                }
            }

            if remove_stmt {
                body.remove(index);
            } else {
                index += 1;
            }
        }
        future_features
    }

    fn annotation_string(&self, expr: &Expr) -> String {
        Generator::new(&self.indent, LineEnding::default()).expr(expr)
    }
}

fn is_known_future_feature(feature: &str) -> bool {
    matches!(
        feature,
        "nested_scopes"
            | "generators"
            | "division"
            | "absolute_import"
            | "with_statement"
            | "print_function"
            | "unicode_literals"
            | "barry_as_FLUFL"
            | "generator_stop"
            | "annotations"
    )
}

impl Transformer for FutureAnnotationsRewriter {
    fn visit_annotation(&mut self, expr: &mut Expr) {
        let rendered = self.annotation_string(expr);
        *expr = py_expr!("{literal:literal}", literal = rendered.as_str());
    }
}

fn is_future_import(import_from: &ast::StmtImportFrom) -> bool {
    import_from
        .module
        .as_ref()
        .is_some_and(|module| module.id.as_str() == "__future__")
}

pub(crate) fn invalid_future_feature_syntax_error_stmts(
    future_features: &HashSet<String>,
) -> Vec<Stmt> {
    let mut invalid_features = future_features
        .iter()
        .filter(|feature| !is_known_future_feature(feature))
        .cloned()
        .collect::<Vec<_>>();
    invalid_features.sort();
    let Some(invalid_feature) = invalid_features.into_iter().next() else {
        return Vec::new();
    };

    let global_stmt: ast::StmtGlobal =
        crate::py_stmt_typed!("global {feature:id}", feature = invalid_feature.as_str(),);
    let nonlocal_stmt: ast::StmtNonlocal =
        crate::py_stmt_typed!("nonlocal {feature:id}", feature = invalid_feature.as_str(),);
    vec![Stmt::Global(global_stmt), Stmt::Nonlocal(nonlocal_stmt)]
}

#[cfg(test)]
mod test;

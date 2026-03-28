use crate::passes::ast_to_ast::body::Suite;
use crate::py_expr;
use crate::{passes::ast_to_ast::context::Context, transformer::Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::{ParseError, ParseErrorType};
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

pub fn validate_future_imports(body: &Suite) -> Result<(), ParseError> {
    for stmt in body {
        let Stmt::ImportFrom(import_from) = stmt else {
            continue;
        };
        if !is_future_import(import_from) {
            continue;
        }
        for alias in &import_from.names {
            if !is_known_future_feature(&alias.name) {
                return Err(ParseError {
                    error: ParseErrorType::OtherError(format!(
                        "Future feature `{}` is not defined",
                        alias.name
                    )),
                    location: alias.range,
                });
            }
        }
    }
    Ok(())
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

#[cfg(test)]
mod test;

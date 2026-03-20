use crate::passes::ast_to_ast::body::Suite;
use crate::py_expr;
use crate::{passes::ast_to_ast::context::Context, transformer::Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;

pub fn rewrite(_context: &Context, body: &mut Suite) {
    let mut rewriter = FutureAnnotationsRewriter::new();
    if !rewriter.has_future_annotations(body) {
        return;
    }
    rewriter.strip_future_import(body);
    (&mut rewriter).visit_body(body);
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

    fn strip_future_import(&mut self, body: &mut Suite) {
        let mut index = 0;
        while index < body.len() {
            let mut remove_stmt = false;
            if let Stmt::ImportFrom(import_from) = &mut body[index] {
                if is_future_annotations(import_from) {
                    import_from
                        .names
                        .retain(|alias| alias.name.id.as_str() != "annotations");
                    if import_from.names.is_empty() {
                        remove_stmt = true;
                    }
                }
            }

            if remove_stmt {
                body.remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn has_future_annotations(&self, body: &Suite) -> bool {
        body.iter().any(|stmt| match stmt {
            Stmt::ImportFrom(import_from) => {
                is_future_annotations(import_from)
                    && import_from
                        .names
                        .iter()
                        .any(|alias| alias.name.id.as_str() == "annotations")
            }
            _ => false,
        })
    }

    fn annotation_string(&self, expr: &Expr) -> String {
        Generator::new(&self.indent, LineEnding::default()).expr(expr)
    }
}

impl Transformer for FutureAnnotationsRewriter {
    fn visit_annotation(&mut self, expr: &mut Expr) {
        let rendered = self.annotation_string(expr);
        *expr = py_expr!("{literal:literal}", literal = rendered.as_str());
    }
}

fn is_future_annotations(import_from: &ast::StmtImportFrom) -> bool {
    import_from
        .module
        .as_ref()
        .is_some_and(|module| module.id.as_str() == "__future__")
}

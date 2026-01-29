use crate::{transform::context::Context, transformer::Transformer};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;

pub fn rewrite(_context: &Context, body: &mut StmtBody) {
    let mut rewriter = FutureAnnotationsRewriter::new();
    rewriter.strip_future_import(body);
    if rewriter.enabled {
        (&mut rewriter).visit_body(body);
    }
}

// When `from __future__ import annotations` is present, stringify annotation
// expressions early so later rewrites don't change the deferred-evaluation form.
struct FutureAnnotationsRewriter {
    enabled: bool,
    indent: Indentation,
}

impl FutureAnnotationsRewriter {
    fn new() -> Self {
        Self {
            enabled: false,
            indent: Indentation::new("    ".to_string()),
        }
    }

    fn strip_future_import(&mut self, body: &mut StmtBody) {
        let mut index = 0;
        while index < body.body.len() {
            let mut remove_stmt = false;
            if let Stmt::ImportFrom(import_from) = body.body[index].as_mut() {
                if is_future_annotations(import_from) {
                    let before_len = import_from.names.len();
                    import_from
                        .names
                        .retain(|alias| alias.name.id.as_str() != "annotations");
                    if import_from.names.is_empty() {
                        remove_stmt = true;
                    }
                    if import_from.names.len() != before_len {
                        self.enabled = true;
                    }
                }
            }

            if remove_stmt {
                body.body.remove(index);
            } else {
                index += 1;
            }
        }
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

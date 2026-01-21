use crate::body_transform::Transformer;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;

pub fn rewrite(body: &mut Vec<Stmt>) {
    let mut rewriter = FutureAnnotationsRewriter::new();
    rewriter.strip_future_import(body);
    if rewriter.enabled {
        rewriter.visit_body(body);
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

    fn strip_future_import(&mut self, body: &mut Vec<Stmt>) {
        let mut index = 0;
        while index < body.len() {
            let mut remove_stmt = false;
            if let Stmt::ImportFrom(import_from) = &mut body[index] {
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
                body.remove(index);
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

#[cfg(test)]
mod tests {
    use ruff_python_ast::Stmt;
    use ruff_python_parser::parse_module;

    use super::rewrite;

    #[test]
    fn preserves_other_future_imports() {
        let mut module = parse_module(
            "from __future__ import annotations, division\nfrom __future__ import generator_stop\n",
        )
        .unwrap()
        .into_syntax();
        rewrite(&mut module.body);
        assert_eq!(module.body.len(), 2);
        match &module.body[0] {
            Stmt::ImportFrom(import_from) => {
                assert_eq!(import_from.names.len(), 1);
                assert_eq!(import_from.names[0].name.id.as_str(), "division");
            }
            other => panic!("expected future import, got {other:?}"),
        }
    }
}

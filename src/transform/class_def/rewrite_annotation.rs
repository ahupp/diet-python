use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::{body_transform::Transformer, py_stmt};

#[derive(Default)]
pub(crate) struct AnnotationCollector {
    annotations: Vec<(String, Expr)>,
}

impl AnnotationCollector {
    pub(crate) fn collect(body: &mut Vec<Stmt>) -> Vec<(String, Expr)> {
        let mut collector = Self::default();
        collector.visit_body(body);
        collector.annotations
    }

    fn rewrite_annotation(&mut self, ann_assign: &mut ast::StmtAnnAssign) -> Option<Stmt> {
        if !ann_assign.simple {
            return None;
        }

        let Expr::Name(ast::ExprName { id, .. }) = ann_assign.target.as_ref() else {
            return None;
        };
        let name = id.to_string();

        let annotation_expr = (*ann_assign.annotation).clone();
        self.annotations.push((name, annotation_expr));

        if let Some(value) = ann_assign.value.take() {
            let target = (*ann_assign.target).clone();
            let mut stmts = py_stmt!(
                "{target:expr} = {value:expr}",
                target = target,
                value = *value,
            );
            Some(
                stmts
                    .pop()
                    .expect("py_stmt! produced no statement for assignment"),
            )
        } else {
            let mut stmts = py_stmt!("pass");
            Some(
                stmts
                    .pop()
                    .expect("py_stmt! produced no statement for pass"),
            )
        }
    }
}

impl Transformer for AnnotationCollector {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        for stmt in body.iter_mut() {
            match stmt {
                Stmt::AnnAssign(ann_assign) => {
                    if let Some(replacement) = self.rewrite_annotation(ann_assign) {
                        *stmt = replacement;
                    }
                }
                Stmt::ClassDef(_) => {
                    // Nested classes are handled by the main rewriter.
                }
                Stmt::FunctionDef(_) => {}
                _ => {}
            }
        }
    }
}

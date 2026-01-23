use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::{body_transform::Transformer, py_stmt};

#[derive(Default)]
pub(crate) struct AnnotationCollector {
    annotations: Vec<(String, Expr)>,
}

impl AnnotationCollector {
    pub(crate) fn rewrite(body: &mut Vec<Stmt>) -> Vec<(String, Expr)> {
        let mut collector = Self::default();
        collector.visit_body(body);
        collector.annotations
    }

    fn rewrite_annotation(&mut self, ann_assign: ast::StmtAnnAssign) -> Vec<Stmt> {
        if !ann_assign.simple {
            return vec![Stmt::AnnAssign(ann_assign)];
        }

        let ast::StmtAnnAssign {
            target,
            annotation,
            value,
            simple,
            range,
            node_index,
        } = ann_assign;

        let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() else {
            return vec![Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                annotation,
                value,
                simple,
                range,
                node_index,
            })];
        };
        let name = id.to_string();

        let mut statements = Vec::new();

        if let Some(value) = value {
            statements.extend(py_stmt!(
                "{target:expr} = {value:expr}",
                target = *target,
                value = *value,
            ));
        } else {
            statements.extend(py_stmt!("pass"));
        }

        self.annotations.push((name, *annotation));

        statements
    }
}

impl Transformer for AnnotationCollector {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        for stmt in body.drain(..) {
            match stmt {
                Stmt::AnnAssign(ann_assign) => {
                    new_body.extend(self.rewrite_annotation(ann_assign));
                }

                _ => new_body.push(stmt),
            }
        }
        *body = new_body;
    }
}

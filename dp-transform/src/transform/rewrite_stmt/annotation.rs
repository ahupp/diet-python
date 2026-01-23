use ruff_python_ast::{Expr, Stmt, self as ast};

use crate::{body_transform::{Transformer, walk_stmt}, py_stmt};

#[derive(Default)]
struct AnnotationCollector {
    entries: Vec<(String, Box<Expr>)>,
}

impl Transformer for AnnotationCollector {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        for mut stmt in body.drain(..) {

            match stmt {
                Stmt::AnnAssign(ast::StmtAnnAssign { target, annotation, value, simple, .. }) => {
                    let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() else {
                        panic!("unsupported AnnAssign target, should be gone: {target:?}");
                    };
                    if value.is_some() {
                        panic!("AnnAssign with value should have been split into a bare annotation and an assignment");
                    }
                    self.entries.push((id.to_string(), annotation));
                }
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {
                    new_body.push(stmt);
                }
                _ => {
                    walk_stmt(self, &mut stmt);
                    new_body.push(stmt);
                }
            }
        }
        *body = new_body;
    }
}

pub fn rewrite_ann_assign_delete(stmts: &mut Vec<Stmt>) {
    let mut collector = AnnotationCollector::default();
    collector.visit_body(stmts);

    // drop the collected annotations
}

pub fn rewrite_ann_assign_to_dunder_annotate(stmts: &mut Vec<Stmt>) {
    let mut collector = AnnotationCollector::default();
    collector.visit_body(stmts);

    let mut annotation_writes = Vec::new();
    for (name, expr) in collector.entries.into_iter() {
        annotation_writes.extend(py_stmt!(
            "_dp_annotations[{name:literal}] = {value:expr}",
            name = name.as_str(),
            value = expr,
        ));
    }

    let annotate = py_stmt!(
        r#"
def __annotate__(_dp_format):
    if _dp_format > 2:
        raise NotImplementedError
    _dp_annotations = {}
    {annotation_writes:stmt}
    return _dp_annotations
"#,
        annotation_writes = annotation_writes,
    );    

    stmts.extend(annotate);
}

use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};

use crate::{py_stmt, template::empty_body, transform::context::Context};
use crate::transformer::{Transformer, walk_stmt};

pub fn rewrite_ann_assign_to_dunder_annotate(_context: &Context, stmt: &mut StmtBody) {
    // Assume called with module body stmt, which gets __anotate__
    let entries = AnnotationStripper::strip(stmt);
    if entries.is_empty() {
        return;
    }
    let ds = to_dunder_annotate(entries);
    stmt.body.push(Box::new(ds));

}

#[derive(Default)]
struct AnnotationStripper {
    entries: Vec<(String, Expr)>,
}

impl AnnotationStripper {
  fn strip(stmt: &mut StmtBody) -> Vec<(String, Expr)> {
    let mut collector = AnnotationStripper::default();
    collector.visit_body(stmt);
    collector.entries
  }
}


impl Transformer for AnnotationStripper {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                AnnotationStripper::strip(&mut func_def.body);
                // drop the collected annotations
            }
            Stmt::ClassDef(class_def) => {
                let ds = to_dunder_annotate(AnnotationStripper::strip(&mut class_def.body));
                class_def.body.body.push(Box::new(ds));
            }
            Stmt::AnnAssign(ast::StmtAnnAssign { target, annotation, value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    self.entries
                        .push((id.to_string(), annotation.as_ref().clone()));
                } else {
                    // ignore annotations on stuff like "self.x: int = 1"
                }

                if let Some(value) = value.as_mut() {
                    self.visit_expr(value);
                    // TODO: copy range and node_index from the original statement
                    *stmt = py_stmt!("{target:expr} = {value:expr}", target = target.clone(), value = value.clone());
                } else {
                    *stmt = empty_body().into();
                }
            }
            _ => {
                walk_stmt(self, stmt);
            }
        }
    }

}

fn to_dunder_annotate(entries: Vec<(String, Expr)>) -> Stmt {
    py_stmt!(
        r#"
def __annotate__(_dp_format):
    if _dp_format > 2:
        raise NotImplementedError
    return {annotation_entries:dict}
"#,
        annotation_entries = entries,
    )
}

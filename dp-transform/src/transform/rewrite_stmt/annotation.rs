use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;

use crate::transformer::{walk_stmt, Transformer};
use crate::{
    py_expr, py_stmt,
    transform::{context::Context, rewrite_expr::make_tuple},
};

pub fn rewrite_ann_assign_to_dunder_annotate(_context: &Context, stmt: &mut StmtBody) {
    // Assume called with module body stmt, which gets __annotate__.
    let entries = AnnotationStripper::strip(stmt);
    if entries.is_empty() {
        return;
    }
    let ds = to_annotate_fn(entries, "__annotate__");
    stmt.body.push(Box::new(ds));
}

#[derive(Default)]
struct AnnotationStripper {
    entries: Vec<(String, Expr, String)>,
    indent: Indentation,
}

impl AnnotationStripper {
    fn strip(stmt: &mut StmtBody) -> Vec<(String, Expr, String)> {
        let mut collector = AnnotationStripper {
            entries: Vec::new(),
            indent: Indentation::new("    ".to_string()),
        };
        collector.visit_body(stmt);
        collector.entries
    }

    fn annotation_string(&self, expr: &Expr) -> String {
        Generator::new(&self.indent, LineEnding::default()).expr(expr)
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
                let entries = AnnotationStripper::strip(&mut class_def.body);
                if !entries.is_empty() {
                    // CPython stores class annotation thunks under __annotate_func__,
                    // and exposes __annotate__ via type-level descriptor logic.
                    let ds = to_annotate_fn(entries, "__annotate_func__");
                    class_def.body.body.push(Box::new(ds));
                }
            }
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                annotation,
                value,
                ..
            }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    self.entries.push((
                        id.to_string(),
                        annotation.as_ref().clone(),
                        self.annotation_string(annotation),
                    ));
                } else {
                    // ignore annotations on stuff like "self.x: int = 1"
                }

                if let Some(value) = value.as_mut() {
                    self.visit_expr(value);
                    // TODO: copy range and node_index from the original statement
                    *stmt = py_stmt!(
                        "{target:expr} = {value:expr}",
                        target = target.clone(),
                        value = value.clone()
                    );
                } else {
                    *stmt = py_stmt!("pass");
                }
            }
            _ => {
                walk_stmt(self, stmt);
            }
        }
    }
}

fn to_annotate_fn(entries: Vec<(String, Expr, String)>, name: &str) -> Stmt {
    let value_pairs = entries
        .into_iter()
        .map(|(key, value, source)| {
            (
                py_expr!(
                    "({key:literal}, {value:expr})",
                    key = key.as_str(),
                    value = value
                ),
                py_expr!(
                    "({key:literal}, {value:literal})",
                    key = key.as_str(),
                    value = source.as_str()
                ),
            )
        })
        .collect::<Vec<_>>();
    let value_dict = py_expr!(
        "__dp__.dict({items:expr})",
        items = make_tuple(value_pairs.iter().map(|(value_pair, _)| value_pair.clone()).collect())
    );
    let string_dict = py_expr!(
        "__dp__.dict({items:expr})",
        items = make_tuple(
            value_pairs
                .iter()
                .map(|(_, string_pair)| string_pair.clone())
                .collect()
        )
    );
    // Capture __dp__ at definition time so annotationlib fake-globals execution in
    // FORWARDREF/STRING modes cannot replace runtime helpers/builtins used by this thunk.
    // Format values in Python 3.15's annotationlib are:
    // VALUE=1, VALUE_WITH_FAKE_GLOBALS=2, FORWARDREF=3, STRING=4.
    // We handle STRING directly from captured source text.
    // For FORWARDREF, raise NotImplementedError so annotationlib drives
    // VALUE_WITH_FAKE_GLOBALS fallback with its stringifier globals.
    py_stmt!(
        r#"
def {annotate_name:id}(_dp_format, _dp=__dp__):
    if _dp.eq(_dp_format, 4):
        return {string_dict:expr}
    if _dp.gt(_dp_format, 2):
        raise _dp.builtins.NotImplementedError
    return {value_dict:expr}
"#,
        annotate_name = name,
        string_dict = string_dict,
        value_dict = value_dict,
    )
}

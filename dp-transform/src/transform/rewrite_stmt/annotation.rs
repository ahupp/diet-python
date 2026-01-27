use ruff_python_ast::{Expr, Stmt, self as ast};

use crate::{body_transform::{Transformer, walk_stmt}, py_stmt};
use ruff_text_size::TextRange;

#[derive(Default)]
struct AnnotationCollector {
    entries: Vec<(String, Box<Expr>)>,
}

fn pass_stmt() -> Stmt {
    Stmt::Pass(ast::StmtPass {
        node_index: Default::default(),
        range: TextRange::default(),
    })
}

struct EmptyBodyFiller;

impl Transformer for EmptyBodyFiller {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::If(ast::StmtIf { body, elif_else_clauses, .. }) => {
                for stmt in body.iter_mut() {
                    self.visit_stmt(stmt);
                }
                if body.is_empty() {
                    body.push(pass_stmt());
                }
                for clause in elif_else_clauses.iter_mut() {
                    for stmt in clause.body.iter_mut() {
                        self.visit_stmt(stmt);
                    }
                    if clause.body.is_empty() {
                        clause.body.push(pass_stmt());
                    }
                }
            }
            Stmt::For(ast::StmtFor { body, orelse, .. })
            | Stmt::While(ast::StmtWhile { body, orelse, .. }) => {
                for stmt in body.iter_mut() {
                    self.visit_stmt(stmt);
                }
                if body.is_empty() {
                    body.push(pass_stmt());
                }
                for stmt in orelse.iter_mut() {
                    self.visit_stmt(stmt);
                }
            }
            Stmt::With(ast::StmtWith { body, .. }) => {
                for stmt in body.iter_mut() {
                    self.visit_stmt(stmt);
                }
                if body.is_empty() {
                    body.push(pass_stmt());
                }
            }
            Stmt::Try(ast::StmtTry {
                body,
                handlers,
                orelse,
                finalbody,
                ..
            }) => {
                for stmt in body.iter_mut() {
                    self.visit_stmt(stmt);
                }
                if body.is_empty() {
                    body.push(pass_stmt());
                }
                for ast::ExceptHandler::ExceptHandler(handler) in handlers.iter_mut() {
                    for stmt in handler.body.iter_mut() {
                        self.visit_stmt(stmt);
                    }
                    if handler.body.is_empty() {
                        handler.body.push(pass_stmt());
                    }
                }
                for stmt in orelse.iter_mut() {
                    self.visit_stmt(stmt);
                }
                if orelse.is_empty() {
                    // orelse is optional; leave empty
                }
                for stmt in finalbody.iter_mut() {
                    self.visit_stmt(stmt);
                }
                if finalbody.is_empty() {
                    // finalbody is optional; leave empty
                }
            }
            Stmt::Match(ast::StmtMatch { cases, .. }) => {
                for case in cases.iter_mut() {
                    for stmt in case.body.iter_mut() {
                        self.visit_stmt(stmt);
                    }
                    if case.body.is_empty() {
                        case.body.push(pass_stmt());
                    }
                }
            }
            Stmt::FunctionDef(ast::StmtFunctionDef { body, .. })
            | Stmt::ClassDef(ast::StmtClassDef { body, .. }) => {
                for stmt in body.iter_mut() {
                    self.visit_stmt(stmt);
                }
                if body.is_empty() {
                    body.push(pass_stmt());
                }
            }
            _ => {
                walk_stmt(self, stmt);
            }
        }
    }
}

fn fill_empty_bodies(stmts: &mut Vec<Stmt>) {
    let mut filler = EmptyBodyFiller;
    for stmt in stmts.iter_mut() {
        filler.visit_stmt(stmt);
    }
}

impl Transformer for AnnotationCollector {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        for mut stmt in body.drain(..) {

            match stmt {
                Stmt::AnnAssign(ast::StmtAnnAssign { target, annotation, value, .. }) => {
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
    fill_empty_bodies(stmts);
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
    fill_empty_bodies(stmts);
}

use ruff_python_ast::{self as ast, Stmt};
use ruff_text_size::TextRange;

pub type Body = ast::Body;
pub type Suite = ast::Suite;

pub fn suite_ref(body: &Body) -> &Suite {
    &body.body
}

pub fn suite_mut(body: &mut Body) -> &mut Suite {
    &mut body.body
}

pub fn take_suite(body: &mut Body) -> Suite {
    std::mem::take(&mut body.body)
}

pub fn body_from_suite(body: Suite) -> Body {
    Body {
        body,
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }
}

pub fn empty_body() -> Body {
    body_from_suite(empty_suite())
}

pub fn empty_suite() -> Suite {
    vec![]
}

pub fn cloned_suite(body: &Body) -> Suite {
    body.body.clone()
}

pub fn stmt_ref(body: &Body, index: usize) -> &Stmt {
    body.body[index].as_ref()
}

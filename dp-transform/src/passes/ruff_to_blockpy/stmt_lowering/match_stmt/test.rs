use super::super::{simplify_stmt_ast_once_for_blockpy, BlockPyStmtFragmentBuilder};
use super::*;
use crate::passes::ast_to_ast::{context::Context, Options};

#[test]
fn stmt_match_simplify_ast_desugars_before_blockpy_lowering() {
    let stmt = py_stmt!(
        "
match x:
    case 1:
        pass"
    );
    let Stmt::Match(match_stmt) = stmt else {
        panic!("expected match stmt");
    };

    let context = Context::new(Options::for_test(), "");
    let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Match(match_stmt));

    assert!(!matches!(simplified.as_slice(), [Stmt::Match(_)]));
}

#[test]
fn stmt_match_to_blockpy_uses_trait_owned_simplification_path() {
    let stmt = py_stmt!(
        "
match x:
    case 1:
        y = 1"
    );
    let Stmt::Match(match_stmt) = stmt else {
        panic!("expected match stmt");
    };
    let context = Context::new(Options::for_test(), "");
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    match_stmt
        .to_blockpy(&context, &mut out, None, &mut next_label_id)
        .expect("match lowering should succeed");

    let fragment = out.finish();
    assert!(!fragment.body.is_empty() || fragment.term.is_some());
}

use super::{assigned_names_in_blockpy_stmt, assigned_names_in_blockpy_term};
use crate::block_py::{
    BlockBuilder, BlockLabel, BlockTerm, Expr, StructuredIf, StructuredInstr, TermIf, TermRaise,
};
use crate::py_expr;
use std::collections::HashSet;

#[test]
fn assigned_names_in_blockpy_stmt_collects_nested_fragments() {
    let stmt: StructuredInstr<Expr> = StructuredInstr::If(StructuredIf {
        test: py_expr!("(test_name := source_test)"),
        body: BlockBuilder::with_term(
            vec![StructuredInstr::Expr(py_expr!(
                "(body_name := source_body)"
            ))],
            Some(BlockTerm::Return(py_expr!(
                "(return_name := source_return)"
            ))),
        ),
        orelse: BlockBuilder::with_term(
            vec![StructuredInstr::Expr(py_expr!(
                "(else_name := source_else)"
            ))],
            Some(BlockTerm::Raise(TermRaise {
                exc: Some(py_expr!("(raise_name := source_raise)")),
            })),
        ),
    });

    assert_eq!(
        assigned_names_in_blockpy_stmt(&stmt),
        HashSet::from([
            "test_name".to_string(),
            "body_name".to_string(),
            "return_name".to_string(),
            "else_name".to_string(),
            "raise_name".to_string(),
        ])
    );
}

#[test]
fn assigned_names_in_blockpy_term_keeps_jump_edge_args_out_of_results() {
    let term: BlockTerm<Expr> =
        BlockTerm::Jump(crate::block_py::BlockEdge::new(BlockLabel::from_index(0)));

    assert!(assigned_names_in_blockpy_term(&term).is_empty());
}

#[test]
fn assigned_names_in_blockpy_term_collects_named_exprs_from_if_term() {
    let term = BlockTerm::IfTerm(TermIf {
        test: py_expr!("(branch_name := branch_source)"),
        then_label: BlockLabel::from_index(0),
        else_label: BlockLabel::from_index(1),
    });

    assert_eq!(
        assigned_names_in_blockpy_term(&term),
        HashSet::from(["branch_name".to_string()])
    );
}

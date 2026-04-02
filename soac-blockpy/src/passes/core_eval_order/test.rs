use super::*;
use crate::block_py::{
    BinOp, BinOpKind, BlockPyBlock, BlockPyLabel, BlockPyTerm, CoreBlockPyCallArg,
    CoreBlockPyExprWithAwaitAndYield, Store, StructuredBlockPyStmtFor, UnresolvedName,
};

fn test_name(id: &str) -> UnresolvedName {
    let ast::Expr::Name(expr) = crate::py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr.into()
}

fn is_name_like(expr: &CoreBlockPyExprWithAwaitAndYield) -> bool {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Name(_) => true,
        CoreBlockPyExprWithAwaitAndYield::Load(_) => true,
        _ => false,
    }
}

#[test]
fn eval_order_hoists_call_arguments_in_return_value_to_temps() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from_index(0),
        body: Vec::new(),
        term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "f(g(x), h(y))"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert!(lowered.body.is_empty());
    let BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::Call(call)) = &lowered.term else {
        panic!("expected call expr");
    };
    assert!(is_name_like(call.func.as_ref()));
    assert!(matches!(
        &call.args[0],
        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
    ));
    assert!(matches!(
        &call.args[1],
        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
    ));
}

#[test]
fn eval_order_hoists_return_value_to_temp() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from_index(0),
        body: Vec::new(),
        term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "f(g(x))"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert!(lowered.body.is_empty());
    let BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::Call(call)) = lowered.term else {
        panic!("expected return of recursive call");
    };
    assert!(is_name_like(call.func.as_ref()));
}

#[test]
fn eval_order_hoists_nested_call_in_assignment_rhs() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from_index(0),
        body: vec![StructuredBlockPyStmtFor::Expr(
            Store::new(
                fresh_eval_name(),
                Box::new(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
                    "f(g(x))"
                ))),
            )
            .into(),
        )],
        term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "__dp_NONE"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert_eq!(lowered.body.len(), 1);
    let StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithAwaitAndYield::Store(assign)) =
        &lowered.body[0]
    else {
        panic!("expected rewritten temp store");
    };
    let CoreBlockPyExprWithAwaitAndYield::Call(call) = assign.value.as_ref() else {
        panic!("expected outer call");
    };
    assert!(is_name_like(call.func.as_ref()));
    assert!(matches!(
        &call.args[0],
        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
    ));
}

#[test]
fn eval_order_hoists_await_in_assignment_call_argument() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from_index(0),
        body: vec![StructuredBlockPyStmtFor::Expr(
            Store::new(
                test_name("total"),
                Box::new(CoreBlockPyExprWithAwaitAndYield::BinOp(BinOp::new(
                    BinOpKind::InplaceAdd,
                    CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!("total")),
                    CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!("await Once()")),
                ))),
            )
            .into(),
        )],
        term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "__dp_NONE"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert_eq!(lowered.body.len(), 3);
    let StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithAwaitAndYield::Store(temp_assign)) =
        &lowered.body[0]
    else {
        panic!("expected hoisted await temp store");
    };
    assert!(matches!(
        *temp_assign.value,
        CoreBlockPyExprWithAwaitAndYield::Await(_)
    ));
    let StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithAwaitAndYield::Store(assign)) =
        &lowered.body[1]
    else {
        panic!("expected rewritten store");
    };
    let CoreBlockPyExprWithAwaitAndYield::BinOp(op) = &*assign.value else {
        panic!("expected iadd operation");
    };
    assert_eq!(op.kind, BinOpKind::InplaceAdd);
    assert!(matches!(
        op.right.as_ref(),
        CoreBlockPyExprWithAwaitAndYield::Name(_)
    ));
    assert!(matches!(
        lowered.body[2],
        StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithAwaitAndYield::Del(_))
    ));
}

#[test]
fn eval_order_without_await_hoists_yield_from_in_assignment_call_argument() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from_index(0),
        body: vec![StructuredBlockPyStmtFor::Expr(
            Store::new(
                test_name("total"),
                Box::new(CoreBlockPyExprWithYield::BinOp(BinOp::new(
                    BinOpKind::InplaceAdd,
                    CoreBlockPyExprWithYield::Name(test_name("total")),
                    CoreBlockPyExprWithYield::YieldFrom(CoreBlockPyYieldFrom {
                        node_index: Default::default(),
                        range: Default::default(),
                        value: Box::new(CoreBlockPyExprWithYield::Name(test_name("it"))),
                    }),
                ))),
            )
            .into(),
        )],
        term: BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(test_name("__dp_NONE"))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block_without_await(block);
    assert_eq!(lowered.body.len(), 5);
    let StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithYield::Store(temp_assign)) =
        &lowered.body[0]
    else {
        panic!("expected hoisted yield-from temp store");
    };
    assert!(matches!(
        *temp_assign.value,
        CoreBlockPyExprWithYield::YieldFrom(_)
    ));
    let StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithYield::Store(binop_assign)) =
        &lowered.body[1]
    else {
        panic!("expected hoisted inplace-add temp store");
    };
    let CoreBlockPyExprWithYield::BinOp(op) = &*binop_assign.value else {
        panic!("expected inplace add operation");
    };
    assert!(matches!(
        op.right.as_ref(),
        CoreBlockPyExprWithYield::Name(_)
    ));
    let StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithYield::Store(assign)) = &lowered.body[2]
    else {
        panic!("expected final store into total");
    };
    assert!(matches!(
        assign.value.as_ref(),
        CoreBlockPyExprWithYield::Name(_)
    ));
    assert!(matches!(
        lowered.body[3],
        StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithYield::Del(_))
    ));
    assert!(matches!(
        lowered.body[4],
        StructuredBlockPyStmtFor::Expr(CoreBlockPyExprWithYield::Del(_))
    ));
}

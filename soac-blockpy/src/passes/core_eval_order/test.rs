use super::*;
use crate::block_py::{
    BinOp, BinOpKind, Block, BlockLabel, BlockTerm, CallArgPositional,
    CoreBlockPyExprWithAwaitAndYield, Meta, Store, UnresolvedName, WithMeta, YieldFrom,
};

fn test_name(id: &str) -> UnresolvedName {
    let ast::Expr::Name(expr) = crate::py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr.into()
}

fn is_name_like(expr: &CoreBlockPyExprWithAwaitAndYield) -> bool {
    matches!(expr, CoreBlockPyExprWithAwaitAndYield::Load(_))
}

fn test_load_with_await_and_yield(id: &str) -> CoreBlockPyExprWithAwaitAndYield {
    let name = test_name(id);
    Load::new(name).into()
}

fn test_load_with_yield(id: &str) -> CoreBlockPyExprWithYield {
    let name = test_name(id);
    Load::new(name).into()
}

#[test]
fn eval_order_hoists_call_arguments_in_return_value_to_temps() {
    let block = Block {
        label: BlockLabel::from_index(0),
        body: Vec::new(),
        term: BlockTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "f(g(x), h(y))"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert!(lowered.body.is_empty());
    let BlockTerm::Return(CoreBlockPyExprWithAwaitAndYield::Call(call)) = &lowered.term else {
        panic!("expected call expr");
    };
    assert!(is_name_like(call.func.as_ref()));
    assert!(matches!(
        &call.args[0],
        CallArgPositional::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
    ));
    assert!(matches!(
        &call.args[1],
        CallArgPositional::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
    ));
}

#[test]
fn eval_order_hoists_return_value_to_temp() {
    let block = Block {
        label: BlockLabel::from_index(0),
        body: Vec::new(),
        term: BlockTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "f(g(x))"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert!(lowered.body.is_empty());
    let BlockTerm::Return(CoreBlockPyExprWithAwaitAndYield::Call(call)) = lowered.term else {
        panic!("expected return of recursive call");
    };
    assert!(is_name_like(call.func.as_ref()));
}

#[test]
fn eval_order_hoists_nested_call_in_assignment_rhs() {
    let block = Block {
        label: BlockLabel::from_index(0),
        body: vec![Store::new(
            fresh_eval_name(),
            Box::new(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
                "f(g(x))"
            ))),
        )
        .into()],
        term: BlockTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "__dp_NONE"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert_eq!(lowered.body.len(), 1);
    let CoreBlockPyExprWithAwaitAndYield::Store(assign) = &lowered.body[0] else {
        panic!("expected rewritten temp store");
    };
    let CoreBlockPyExprWithAwaitAndYield::Call(call) = assign.value.as_ref() else {
        panic!("expected outer call");
    };
    assert!(is_name_like(call.func.as_ref()));
    assert!(matches!(
        &call.args[0],
        CallArgPositional::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
    ));
}

#[test]
fn eval_order_hoists_await_in_assignment_call_argument() {
    let block = Block {
        label: BlockLabel::from_index(0),
        body: vec![Store::new(
            test_name("total"),
            Box::new(CoreBlockPyExprWithAwaitAndYield::BinOp(BinOp::new(
                BinOpKind::InplaceAdd,
                CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!("total")),
                CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!("await Once()")),
            ))),
        )
        .into()],
        term: BlockTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "__dp_NONE"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert_eq!(lowered.body.len(), 3);
    let CoreBlockPyExprWithAwaitAndYield::Store(temp_assign) = &lowered.body[0] else {
        panic!("expected hoisted await temp store");
    };
    assert!(matches!(
        *temp_assign.value,
        CoreBlockPyExprWithAwaitAndYield::Await(_)
    ));
    let CoreBlockPyExprWithAwaitAndYield::Store(assign) = &lowered.body[1] else {
        panic!("expected rewritten store");
    };
    let CoreBlockPyExprWithAwaitAndYield::BinOp(op) = &*assign.value else {
        panic!("expected iadd operation");
    };
    assert_eq!(op.kind, BinOpKind::InplaceAdd);
    assert!(matches!(
        op.right.as_ref(),
        CoreBlockPyExprWithAwaitAndYield::Load(_)
    ));
    assert!(matches!(
        lowered.body[2],
        CoreBlockPyExprWithAwaitAndYield::Del(_)
    ));
}

#[test]
fn eval_order_without_await_hoists_yield_from_in_assignment_call_argument() {
    let block = Block {
        label: BlockLabel::from_index(0),
        body: vec![Store::new(
            test_name("total"),
            Box::new(CoreBlockPyExprWithYield::BinOp(BinOp::new(
                BinOpKind::InplaceAdd,
                test_load_with_yield("total"),
                CoreBlockPyExprWithYield::YieldFrom(
                    YieldFrom::new(test_load_with_yield("it")).with_meta(Meta::default()),
                ),
            ))),
        )
        .into()],
        term: BlockTerm::Return(test_load_with_yield("__dp_NONE")),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block_without_await(block);
    assert_eq!(lowered.body.len(), 5);
    let CoreBlockPyExprWithYield::Store(temp_assign) = &lowered.body[0] else {
        panic!("expected hoisted yield-from temp store");
    };
    assert!(matches!(
        *temp_assign.value,
        CoreBlockPyExprWithYield::YieldFrom(_)
    ));
    let CoreBlockPyExprWithYield::Store(binop_assign) = &lowered.body[1] else {
        panic!("expected hoisted inplace-add temp store");
    };
    let CoreBlockPyExprWithYield::BinOp(op) = &*binop_assign.value else {
        panic!("expected inplace add operation");
    };
    assert!(matches!(
        op.right.as_ref(),
        CoreBlockPyExprWithYield::Load(_)
    ));
    let CoreBlockPyExprWithYield::Store(assign) = &lowered.body[2] else {
        panic!("expected final store into total");
    };
    assert!(matches!(
        assign.value.as_ref(),
        CoreBlockPyExprWithYield::Load(_)
    ));
    assert!(matches!(lowered.body[3], CoreBlockPyExprWithYield::Del(_)));
    assert!(matches!(lowered.body[4], CoreBlockPyExprWithYield::Del(_)));
}

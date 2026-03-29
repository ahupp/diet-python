use super::*;
use crate::block_py::{
    BlockPyBlock, BlockPyLabel, BlockPyTerm, CoreBlockPyCallArg, CoreBlockPyExprWithAwaitAndYield,
};

fn test_name(id: &str) -> ast::ExprName {
    let ast::Expr::Name(expr) = crate::py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr
}

#[test]
fn eval_order_hoists_call_arguments_in_return_value_to_temps() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from(0u32),
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
    assert!(matches!(
        call.func.as_ref(),
        CoreBlockPyExprWithAwaitAndYield::Name(_)
    ));
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
        label: BlockPyLabel::from(0u32),
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
    assert!(matches!(
        call.func.as_ref(),
        CoreBlockPyExprWithAwaitAndYield::Name(_)
    ));
}

#[test]
fn eval_order_hoists_nested_call_in_assignment_rhs() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from(0u32),
        body: vec![StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: fresh_eval_name(),
            value: CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!("f(g(x))")),
        })],
        term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "__dp_NONE"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert_eq!(lowered.body.len(), 1);
    let StructuredBlockPyStmt::Assign(assign) = &lowered.body[0] else {
        panic!("expected rewritten assignment");
    };
    let CoreBlockPyExprWithAwaitAndYield::Call(call) = &assign.value else {
        panic!("expected outer call");
    };
    assert!(matches!(
        call.func.as_ref(),
        CoreBlockPyExprWithAwaitAndYield::Name(_)
    ));
    assert!(matches!(
        &call.args[0],
        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
    ));
}

#[test]
fn eval_order_hoists_await_in_assignment_call_argument() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from(0u32),
        body: vec![StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: test_name("total"),
            value: CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
                "__dp_iadd(total, await Once())"
            )),
        })],
        term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "__dp_NONE"
        ))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block(block);
    assert_eq!(lowered.body.len(), 3);
    let StructuredBlockPyStmt::Assign(temp_assign) = &lowered.body[0] else {
        panic!("expected hoisted await temp assignment");
    };
    assert!(matches!(
        temp_assign.value,
        CoreBlockPyExprWithAwaitAndYield::Await(_)
    ));
    let StructuredBlockPyStmt::Assign(assign) = &lowered.body[1] else {
        panic!("expected rewritten assignment");
    };
    let CoreBlockPyExprWithAwaitAndYield::Op(call) = &assign.value else {
        panic!("expected iadd operation");
    };
    assert_eq!(call.helper_name(), "__dp_iadd");
    let call_args = (*call.clone()).into_call_args();
    assert!(matches!(
        &call_args[1],
        CoreBlockPyExprWithAwaitAndYield::Name(_)
    ));
    assert!(matches!(lowered.body[2], StructuredBlockPyStmt::Delete(_)));
}

#[test]
fn eval_order_without_await_hoists_yield_from_in_assignment_call_argument() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from(0u32),
        body: vec![StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: test_name("total"),
            value: CoreBlockPyExprWithYield::Call(CoreBlockPyCall {
                node_index: Default::default(),
                range: Default::default(),
                func: Box::new(CoreBlockPyExprWithYield::Name(test_name("__dp_iadd"))),
                args: vec![
                    CoreBlockPyCallArg::Positional(CoreBlockPyExprWithYield::Name(test_name(
                        "total",
                    ))),
                    CoreBlockPyCallArg::Positional(CoreBlockPyExprWithYield::YieldFrom(
                        CoreBlockPyYieldFrom {
                            node_index: Default::default(),
                            range: Default::default(),
                            value: Box::new(CoreBlockPyExprWithYield::Name(test_name("it"))),
                        },
                    )),
                ],
                keywords: Vec::new(),
            }),
        })],
        term: BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(test_name("__dp_NONE"))),
        params: Vec::new(),
        exc_edge: None,
    };

    let lowered = make_eval_order_explicit_in_core_block_without_await(block);
    assert_eq!(lowered.body.len(), 3);
    let StructuredBlockPyStmt::Assign(temp_assign) = &lowered.body[0] else {
        panic!("expected hoisted yield-from temp assignment");
    };
    assert!(matches!(
        temp_assign.value,
        CoreBlockPyExprWithYield::YieldFrom(_)
    ));
    let StructuredBlockPyStmt::Assign(assign) = &lowered.body[1] else {
        panic!("expected rewritten assignment");
    };
    let CoreBlockPyExprWithYield::Call(call) = &assign.value else {
        panic!("expected iadd call");
    };
    assert!(matches!(
        call.args[1],
        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithYield::Name(_))
    ));
    assert!(matches!(lowered.body[2], StructuredBlockPyStmt::Delete(_)));
}

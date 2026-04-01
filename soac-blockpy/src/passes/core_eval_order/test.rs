use super::*;
use crate::block_py::{
    BlockPyBlock, BlockPyLabel, BlockPyTerm, CoreBlockPyCallArg, CoreBlockPyExprWithAwaitAndYield,
    InplaceBinOp, InplaceBinOpKind, OperationDetail,
};

fn test_name(id: &str) -> ast::ExprName {
    let ast::Expr::Name(expr) = crate::py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr
}

fn is_name_like(expr: &CoreBlockPyExprWithAwaitAndYield) -> bool {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Name(_) => true,
        CoreBlockPyExprWithAwaitAndYield::Op(operation) => matches!(
            operation,
            crate::block_py::OperationDetail::LoadName(_)
                | crate::block_py::OperationDetail::LoadRuntime(_)
        ),
        _ => false,
    }
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
    let BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::Op(operation)) = &lowered.term else {
        panic!("expected call expr");
    };
    let OperationDetail::Call(call) = operation else {
        panic!("expected call operation");
    };
    assert!(is_name_like(call.func.as_ref()));
    assert!(matches!(
        &call.args[0],
        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Op(operation))
            if matches!(operation, OperationDetail::Call(_))
    ));
    assert!(matches!(
        &call.args[1],
        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Op(operation))
            if matches!(operation, OperationDetail::Call(_))
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
    let BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::Op(operation)) = lowered.term else {
        panic!("expected return of recursive call");
    };
    let OperationDetail::Call(call) = operation else {
        panic!("expected call operation");
    };
    assert!(is_name_like(call.func.as_ref()));
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
    let CoreBlockPyExprWithAwaitAndYield::Op(operation) = &assign.value else {
        panic!("expected outer call");
    };
    let OperationDetail::Call(call) = operation else {
        panic!("expected call operation");
    };
    assert!(is_name_like(call.func.as_ref()));
    assert!(matches!(
        &call.args[0],
        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Op(operation))
            if matches!(operation, OperationDetail::Call(_))
    ));
}

#[test]
fn eval_order_hoists_await_in_assignment_call_argument() {
    let block = BlockPyBlock {
        label: BlockPyLabel::from(0u32),
        body: vec![StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: test_name("total"),
            value: CoreBlockPyExprWithAwaitAndYield::Op(OperationDetail::from(InplaceBinOp::new(
                InplaceBinOpKind::Add,
                CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!("total")),
                CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!("await Once()")),
            ))),
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
    assert!(matches!(
        call,
        OperationDetail::InplaceBinOp(op) if op.kind == InplaceBinOpKind::Add
    ));
    let OperationDetail::InplaceBinOp(op) = call else {
        unreachable!("iadd guard should ensure inplace binop");
    };
    assert!(matches!(
        op.right.as_ref(),
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
            value: CoreBlockPyExprWithYield::Op(OperationDetail::from(InplaceBinOp::new(
                InplaceBinOpKind::Add,
                CoreBlockPyExprWithYield::Name(test_name("total")),
                CoreBlockPyExprWithYield::YieldFrom(CoreBlockPyYieldFrom {
                    node_index: Default::default(),
                    range: Default::default(),
                    value: Box::new(CoreBlockPyExprWithYield::Name(test_name("it"))),
                }),
            ))),
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
    let CoreBlockPyExprWithYield::Op(operation) = &assign.value else {
        panic!("expected inplace add operation");
    };
    let OperationDetail::InplaceBinOp(op) = operation else {
        panic!("expected inplace add detail");
    };
    assert!(matches!(
        op.right.as_ref(),
        CoreBlockPyExprWithYield::Name(_)
    ));
    assert!(matches!(lowered.body[2], StructuredBlockPyStmt::Delete(_)));
}

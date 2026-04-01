use super::*;

use crate::block_py::{
    BlockPyCallableSemanticInfo, BlockPyFunction, BlockPyFunctionKind, BlockPyLabel, BlockPyStmt,
    BlockPyTerm, CfgBlock, CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield,
    FunctionName,
};
use crate::passes::core_eval_order::make_eval_order_explicit_in_core_block;

fn test_name_gen() -> crate::block_py::FunctionNameGen {
    let mut module_name_gen = crate::block_py::ModuleNameGen::new(0);
    module_name_gen.next_function_name_gen()
}

#[test]
fn lowers_await_to_yield_from_await_iter() {
    let structured_block = make_eval_order_explicit_in_core_block(CfgBlock {
        label: BlockPyLabel::from(0u32),
        body: Vec::new(),
        term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "await foo()"
        ))),
        params: Vec::new(),
        exc_edge: None,
    });
    let module = BlockPyModule {
        callable_defs: vec![BlockPyFunction {
            function_id: crate::block_py::FunctionId(0),
            name_gen: test_name_gen(),
            names: FunctionName::new("f", "f", "f", "f"),
            kind: BlockPyFunctionKind::Coroutine,
            params: Default::default(),
            blocks: vec![CfgBlock {
                label: structured_block.label,
                body: structured_block
                    .body
                    .into_iter()
                    .map(BlockPyStmt::from)
                    .collect(),
                term: structured_block.term,
                params: structured_block.params,
                exc_edge: structured_block.exc_edge,
            }],
            doc: None,
            storage_layout: None,
            semantic: BlockPyCallableSemanticInfo::default(),
        }],
    };

    let lowered = lower_awaits_in_core_blockpy_module(module);
    let block = &lowered.callable_defs[0].blocks[0];
    assert_eq!(block.body.len(), 1);
    let crate::block_py::BlockPyStmt::Assign(await_assign) = &block.body[0] else {
        panic!("expected lowered await assignment");
    };
    let BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(return_name)) = &block.term else {
        panic!("expected return of lowered await temp");
    };
    assert_eq!(return_name.id, await_assign.target.id);
    let CoreBlockPyExprWithYield::YieldFrom(yield_from) = &await_assign.value else {
        panic!("expected lowered await yield from");
    };
    let CoreBlockPyExprWithYield::Op(operation) = yield_from.value.as_ref() else {
        panic!("expected await_iter call");
    };
    let crate::block_py::OperationDetail::Call(call) = operation else {
        panic!("expected await_iter call op");
    };
    let CoreBlockPyExprWithYield::Op(operation) = call.func.as_ref() else {
        panic!("expected await helper load");
    };
    assert!(matches!(
        operation,
        crate::block_py::OperationDetail::LoadRuntime(op) if op.name == "await_iter"
    ));
}

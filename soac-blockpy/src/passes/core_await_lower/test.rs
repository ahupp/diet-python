use super::*;

use crate::block_py::{
    Block, BlockLabel, BlockPyFunction, BlockPyNameLike, BlockTerm, CallableScopeInfo,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, FunctionKind, FunctionName,
};
use crate::passes::core_eval_order::make_eval_order_explicit_in_core_block;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

fn test_name_gen() -> crate::block_py::FunctionNameGen {
    let module_name_gen = crate::block_py::ModuleNameGen::new(0);
    module_name_gen.next_function_name_gen()
}

fn name_expr(name: &str) -> ast::ExprName {
    let Expr::Name(name) = py_expr!("{name:id}", name = name) else {
        unreachable!();
    };
    name
}

fn core_load_with_await_and_yield(name: &str) -> CoreBlockPyExprWithAwaitAndYield {
    let name = name_expr(name);
    let meta = name.meta();
    crate::block_py::Load::new(name).with_meta(meta).into()
}

#[test]
fn lowers_await_to_yield_from_await_iter() {
    let structured_block = make_eval_order_explicit_in_core_block(Block {
        label: BlockLabel::from_index(0),
        body: Vec::new(),
        term: BlockTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
            "await foo()"
        ))),
        params: Vec::new(),
        exc_edge: None,
    });
    let module = BlockPyModule {
        module_name_gen: crate::block_py::ModuleNameGen::new(0),
        callable_defs: vec![BlockPyFunction {
            function_id: crate::block_py::FunctionId(0),
            name_gen: test_name_gen(),
            names: FunctionName::new("f", "f", "f", "f"),
            kind: FunctionKind::Coroutine,
            params: Default::default(),
            blocks: vec![Block {
                label: structured_block.label,
                body: structured_block.body,
                term: structured_block.term,
                params: structured_block.params,
                exc_edge: structured_block.exc_edge,
            }],
            doc: None,
            storage_layout: None,
            scope: CallableScopeInfo::default(),
        }],
        counter_defs: Vec::new(),
        module_constants: Vec::new(),
    };

    let lowered = lower_awaits_in_core_blockpy_module(module);
    let block = &lowered.callable_defs[0].blocks[0];
    assert_eq!(block.body.len(), 1);
    let CoreBlockPyExprWithYield::Store(await_assign) = &block.body[0] else {
        panic!("expected lowered await store expr");
    };
    let BlockTerm::Return(CoreBlockPyExprWithYield::Load(return_load)) = &block.term else {
        panic!("expected return of lowered await temp");
    };
    assert_eq!(return_load.name.id_str(), await_assign.name.id_str());
    let CoreBlockPyExprWithYield::YieldFrom(yield_from) = &*await_assign.value else {
        panic!("expected lowered await yield from");
    };
    let CoreBlockPyExprWithYield::Call(call) = yield_from.value.as_ref() else {
        panic!("expected await_iter call op");
    };
    let CoreBlockPyExprWithYield::Load(operation) = call.func.as_ref() else {
        panic!("expected await helper load");
    };
    assert!(operation.name.is_runtime_symbol("await_iter"));
}

use super::lower_try_jump_exception_flow;
use crate::block_py::{BbBlock, BlockPyEdge, BlockPyLabel, BlockPyTerm, LocatedCoreBlockPyExpr};
use crate::{transform_str_to_bb_ir_with_options, Options};

#[test]
fn preserves_existing_exception_edges() {
    let source = r#"
def f(x):
    return x
"#;
    let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
        .expect("lowering must succeed")
        .expect("bb module must exist");
    let (body_label, except_label) = {
        let function = module
            .callable_defs
            .iter_mut()
            .find(|function| function.names.qualname == "f")
            .expect("must contain f");
        let body_label = BlockPyLabel::from("_dp_manual_body");
        let except_label = BlockPyLabel::from("_dp_manual_except");

        function.blocks.push(BbBlock {
            label: body_label.clone(),
            body: vec![],
            term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Return(
                <LocatedCoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
            ),
            params: vec![crate::block_py::BlockParam {
                name: "_dp_try_exc_manual".to_string(),
                role: crate::block_py::BlockParamRole::Exception,
            }],
            exc_edge: Some(BlockPyEdge::new(except_label.clone())),
        });
        function.blocks.push(BbBlock {
            label: except_label.clone(),
            body: vec![],
            term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Return(
                <LocatedCoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
            ),
            params: Vec::new(),
            exc_edge: None,
        });
        (body_label, except_label)
    };

    let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
    let lowered_function = lowered
        .callable_defs
        .iter()
        .find(|candidate| candidate.names.qualname == "f")
        .expect("must contain lowered f");
    let body_block = lowered_function
        .blocks
        .iter()
        .find(|block| block.label == body_label)
        .expect("body block must exist");
    assert_eq!(
        body_block
            .exc_edge
            .as_ref()
            .map(|edge| edge.target.as_str()),
        Some(except_label.as_str()),
        "body region should dispatch to except block on exception"
    );
    assert_eq!(
        body_block.exception_param(),
        Some("_dp_try_exc_manual"),
        "exception binding name should be attached to body region"
    );
}

#[test]
fn rejects_try_jump_with_unknown_label() {
    let source = r#"
def f():
    return 1
"#;
    let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
        .expect("lowering must succeed")
        .expect("bb module must exist");
    let function = module
        .callable_defs
        .first_mut()
        .expect("must contain function");
    function.blocks[0].exc_edge = Some(BlockPyEdge::new(BlockPyLabel::from("missing_except")));

    let err = lower_try_jump_exception_flow(&module).expect_err("must reject unknown labels");
    assert!(
        err.contains("unknown exception target"),
        "unexpected error: {err}"
    );
}

#[test]
fn splits_exception_edge_block_into_one_op_segments() {
    let source = r#"
def f():
    a = 1
    b = 2
    return b
"#;
    let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
        .expect("lowering must succeed")
        .expect("bb module must exist");
    let function = module
        .callable_defs
        .iter_mut()
        .find(|function| function.names.qualname == "f")
        .expect("must contain f");
    let block_index = function
        .blocks
        .iter()
        .position(|block| block.body.len() >= 2)
        .expect("must contain multi-op block");
    let original_label = function.blocks[block_index].label.clone();
    let except_label = BlockPyLabel::from("_dp_manual_except_split");
    function.blocks.push(BbBlock {
        label: except_label.clone(),
        body: vec![],
        term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Return(
            <LocatedCoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
        ),
        params: Vec::new(),
        exc_edge: None,
    });
    function.blocks[block_index].exc_edge = Some(BlockPyEdge::new(except_label.clone()));
    function.blocks[block_index].set_exception_param("_dp_try_exc_split");

    let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
    let lowered_function = lowered
        .callable_defs
        .iter()
        .find(|candidate| candidate.names.qualname == "f")
        .expect("must contain lowered f");

    let first = lowered_function
        .blocks
        .iter()
        .find(|block| block.label == original_label)
        .expect("split must keep original block label");
    assert_eq!(first.body.len(), 1, "first split block must contain one op");
    assert!(
        matches!(first.term, BlockPyTerm::Jump(_)),
        "split op block must jump to next split block"
    );
    assert_eq!(
        first.exc_edge.as_ref().map(|edge| edge.target.as_str()),
        Some(except_label.as_str()),
        "split block must preserve exception edge target"
    );

    let split_tail = lowered_function
        .blocks
        .iter()
        .find(|block| block.label.as_str().contains("__excchk_"))
        .expect("must contain split tail block");
    assert!(
        split_tail.body.len() <= 1,
        "split tail block should not aggregate ops"
    );
}

#[test]
fn keeps_pure_expr_ops_grouped_until_local_state_changes() {
    let source = r#"
def f():
    x()
    y()
    z = 1
    w()
"#;
    let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
        .expect("lowering must succeed")
        .expect("bb module must exist");
    let function = module
        .callable_defs
        .iter_mut()
        .find(|function| function.names.qualname == "f")
        .expect("must contain f");
    let block_index = function
        .blocks
        .iter()
        .position(|block| block.body.len() >= 4)
        .expect("must contain multi-op block");
    let original_label = function.blocks[block_index].label.clone();
    let except_label = BlockPyLabel::from("_dp_manual_except_group");
    function.blocks.push(BbBlock {
        label: except_label.clone(),
        body: vec![],
        term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Return(
            <LocatedCoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
        ),
        params: Vec::new(),
        exc_edge: None,
    });
    function.blocks[block_index].exc_edge = Some(BlockPyEdge::new(except_label.clone()));
    function.blocks[block_index].set_exception_param("_dp_try_exc_group");

    let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
    let lowered_function = lowered
        .callable_defs
        .iter()
        .find(|candidate| candidate.names.qualname == "f")
        .expect("must contain lowered f");

    let first = lowered_function
        .blocks
        .iter()
        .find(|block| block.label == original_label)
        .expect("lowered entry block must exist");
    assert_eq!(
        first.body.len(),
        3,
        "pure expr ops should remain grouped until the local assignment"
    );
    assert!(
        matches!(first.term, BlockPyTerm::Jump(_)),
        "state-changing assignment should still split the block"
    );

    let next = lowered_function
        .blocks
        .iter()
        .find(|block| block.label.as_str().contains("__excchk_"))
        .expect("must contain split successor");
    assert_eq!(
        next.body.len(),
        1,
        "ops after the assignment should start a new segment"
    );
}

#[test]
fn preserves_value_return_after_plain_try_except() {
    let source = r#"
def f():
    try:
        pass
    except Exception:
        pass
    return 1
"#;
    let module = transform_str_to_bb_ir_with_options(source, Options::for_test())
        .expect("lowering must succeed")
        .expect("bb module must exist");
    let raw_function = module
        .callable_defs
        .iter()
        .find(|candidate| candidate.names.qualname == "f")
        .expect("must contain raw f");
    assert!(
        raw_function.blocks.iter().any(|block| {
            matches!(
                block.term,
                crate::block_py::BlockPyTerm::Return(crate::block_py::CoreBlockPyExpr::Literal(
                    crate::block_py::CoreBlockPyLiteral::NumberLiteral(
                        crate::block_py::CoreNumberLiteral {
                            value: crate::block_py::CoreNumberLiteralValue::Int(_),
                            ..
                        }
                    )
                ))
            )
        }),
        "{raw_function:#?}"
    );
    let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
    let lowered_function = lowered
        .callable_defs
        .iter()
        .find(|candidate| candidate.names.qualname == "f")
        .expect("must contain lowered f");

    assert!(
        lowered_function.blocks.iter().any(|block| {
            matches!(
                block.term,
                crate::block_py::BlockPyTerm::Return(crate::block_py::CoreBlockPyExpr::Literal(
                    crate::block_py::CoreBlockPyLiteral::NumberLiteral(
                        crate::block_py::CoreNumberLiteral {
                            value: crate::block_py::CoreNumberLiteralValue::Int(_),
                            ..
                        }
                    )
                ))
            )
        }),
        "{lowered_function:#?}"
    );
}

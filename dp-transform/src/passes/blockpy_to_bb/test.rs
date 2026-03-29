use crate::block_py::{
    BlockPyAssign, BlockPyBlock, BlockPyIf, BlockPyLabel, BlockPyStmtFragment, BlockPyTerm,
    CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyLiteral, CoreStringLiteral,
    GetAttr, LocatedCoreBlockPyExpr, LocatedName, Operation, StructuredBlockPyStmt,
};
use crate::passes::ruff_to_blockpy::{
    lower_structured_located_blocks_to_bb_blocks, populate_exception_edge_args,
};
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;
use std::collections::HashMap;

#[test]
fn linearizes_structured_if_stmt_into_explicit_blocks() {
    let block: BlockPyBlock<LocatedCoreBlockPyExpr, LocatedName> = BlockPyBlock {
        label: BlockPyLabel::from(0u32),
        body: vec![
            StructuredBlockPyStmt::Assign(BlockPyAssign {
                target: LocatedName::from(ast::ExprName {
                    id: "x".into(),
                    ctx: ast::ExprContext::Store,
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                }),
                value: core_name_expr("a"),
            }),
            StructuredBlockPyStmt::If(BlockPyIf {
                test: core_name_expr("cond"),
                body: BlockPyStmtFragment::from_stmts(vec![StructuredBlockPyStmt::Assign(
                    BlockPyAssign {
                        target: LocatedName::from(ast::ExprName {
                            id: "x".into(),
                            ctx: ast::ExprContext::Store,
                            range: TextRange::default(),
                            node_index: ast::AtomicNodeIndex::default(),
                        }),
                        value: core_name_expr("b"),
                    },
                )]),
                orelse: BlockPyStmtFragment::from_stmts(vec![StructuredBlockPyStmt::Assign(
                    BlockPyAssign {
                        target: LocatedName::from(ast::ExprName {
                            id: "x".into(),
                            ctx: ast::ExprContext::Store,
                            range: TextRange::default(),
                            node_index: ast::AtomicNodeIndex::default(),
                        }),
                        value: core_name_expr("c"),
                    },
                )]),
            }),
            StructuredBlockPyStmt::Expr(core_call_expr("sink", vec![core_name_expr("x")])),
        ],
        term: BlockPyTerm::Return(core_name_expr("__dp_NONE")),
        params: Vec::new(),
        exc_edge: None,
    };

    let blocks = lower_structured_located_blocks_to_bb_blocks(
        &[crate::block_py::CfgBlock {
            label: block.label,
            body: block.body,
            term: block.term,
            params: block.params,
            exc_edge: None,
        }],
        &HashMap::new(),
    );

    assert_eq!(blocks.len(), 4, "{blocks:?}");
    assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
}

fn core_name_expr(name: &str) -> LocatedCoreBlockPyExpr {
    LocatedCoreBlockPyExpr::Name(LocatedName::from(ast::ExprName {
        id: name.into(),
        ctx: ast::ExprContext::Load,
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }))
}

fn core_call_expr(name: &str, args: Vec<LocatedCoreBlockPyExpr>) -> LocatedCoreBlockPyExpr {
    LocatedCoreBlockPyExpr::Call(CoreBlockPyCall {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        func: Box::new(core_name_expr(name)),
        args: args
            .into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        keywords: Vec::new(),
    })
}

fn core_string_expr(value: &str) -> LocatedCoreBlockPyExpr {
    LocatedCoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(CoreStringLiteral {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        value: value.to_string(),
    }))
}

#[test]
fn rewrites_current_exception_placeholders_in_final_core_blocks() {
    let block: BlockPyBlock<LocatedCoreBlockPyExpr, LocatedName> = BlockPyBlock {
        label: BlockPyLabel::from(0u32),
        body: vec![StructuredBlockPyStmt::Expr(core_call_expr(
            "__dp_current_exception",
            Vec::new(),
        ))],
        term: BlockPyTerm::Return(core_call_expr("__dp_exc_info", Vec::new())),
        params: vec![crate::block_py::BlockParam {
            name: "_dp_try_exc_0".to_string(),
            role: crate::block_py::BlockParamRole::Exception,
        }],
        exc_edge: None,
    };

    let lowered = lower_structured_located_blocks_to_bb_blocks(
        &[crate::block_py::CfgBlock {
            label: block.label,
            body: block.body,
            term: block.term,
            params: block.params,
            exc_edge: None,
        }],
        &HashMap::new(),
    );
    let block = &lowered[0];

    let crate::block_py::BlockPyStmt::Expr(body_expr) = &block.body[0] else {
        panic!("expected expr stmt in lowered BB block");
    };
    assert!(matches!(
        body_expr,
        CoreBlockPyExpr::Name(name) if name.id.as_str() == "_dp_try_exc_0"
    ));

    let BlockPyTerm::Return(CoreBlockPyExpr::Call(call)) = &block.term else {
        panic!("expected rewritten return expr");
    };
    assert!(matches!(
        call.func.as_ref(),
        CoreBlockPyExpr::Name(name)
            if name.id.as_str() == "__dp_exc_info_from_exception"
    ));
    assert!(matches!(
        call.args.as_slice(),
        [CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(name))]
            if name.id.as_str() == "_dp_try_exc_0"
    ));
}

#[test]
fn rewrites_current_exception_inside_intrinsic_helper_args() {
    let block: BlockPyBlock<LocatedCoreBlockPyExpr, LocatedName> = BlockPyBlock {
        label: BlockPyLabel::from(0u32),
        body: Vec::new(),
        term: BlockPyTerm::Return(CoreBlockPyExpr::Op(Box::new(Operation::GetAttr(GetAttr {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            arg0: core_call_expr("__dp_current_exception", Vec::new()),
            arg1: "value".to_string(),
        })))),
        params: vec![crate::block_py::BlockParam {
            name: "_dp_try_exc_0".to_string(),
            role: crate::block_py::BlockParamRole::Exception,
        }],
        exc_edge: None,
    };

    let lowered = lower_structured_located_blocks_to_bb_blocks(
        &[crate::block_py::CfgBlock {
            label: block.label,
            body: block.body,
            term: block.term,
            params: block.params,
            exc_edge: None,
        }],
        &HashMap::new(),
    );
    let block = &lowered[0];

    let BlockPyTerm::Return(CoreBlockPyExpr::Op(operation)) = &block.term else {
        panic!("expected operation return expr");
    };
    let Operation::GetAttr(GetAttr { arg0, arg1, .. }) = operation.as_ref() else {
        panic!("expected getattr operation");
    };
    assert!(matches!(
        arg0,
        CoreBlockPyExpr::Name(name) if name.id.as_str() == "_dp_try_exc_0"
    ));
    assert_eq!(arg1, "value");
}

#[test]
fn exception_edges_seed_hidden_try_exception_locals_from_current_exception() {
    let mut blocks: Vec<
        crate::block_py::CfgBlock<
            crate::block_py::BlockPyStmt<LocatedCoreBlockPyExpr, LocatedName>,
            BlockPyTerm<LocatedCoreBlockPyExpr>,
        >,
    > = vec![
        crate::block_py::CfgBlock {
            label: BlockPyLabel::from(0u32),
            body: Vec::new(),
            term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Return(
                <LocatedCoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
            ),
            params: vec![crate::block_py::BlockParam {
                name: "_dp_outer_exc".to_string(),
                role: crate::block_py::BlockParamRole::Exception,
            }],
            exc_edge: Some(crate::block_py::BlockPyEdge::new(BlockPyLabel::from(1u32))),
        },
        crate::block_py::CfgBlock {
            label: BlockPyLabel::from(1u32),
            body: Vec::new(),
            term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Jump(
                crate::block_py::BlockPyEdge::with_args(
                    BlockPyLabel::from(2u32),
                    vec![
                        crate::block_py::BlockArg::AbruptKind(
                            crate::block_py::AbruptKind::Exception,
                        ),
                        crate::block_py::BlockArg::Name("_dp_try_exc_payload".to_string()),
                    ],
                ),
            ),
            params: vec![
                crate::block_py::BlockParam {
                    name: "_dp_inner_exc".to_string(),
                    role: crate::block_py::BlockParamRole::Exception,
                },
                crate::block_py::BlockParam {
                    name: "_dp_try_exc_payload".to_string(),
                    role: crate::block_py::BlockParamRole::Local,
                },
            ],
            exc_edge: None,
        },
        crate::block_py::CfgBlock {
            label: BlockPyLabel::from(2u32),
            body: Vec::new(),
            term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Return(
                <LocatedCoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
            ),
            params: Vec::new(),
            exc_edge: None,
        },
    ];

    populate_exception_edge_args(&mut blocks);

    let edge = blocks[0]
        .exc_edge
        .as_ref()
        .expect("source block must keep exception edge");
    assert!(matches!(
        edge.args.as_slice(),
        [
            crate::block_py::BlockArg::CurrentException,
            crate::block_py::BlockArg::CurrentException
        ]
    ));
}

use crate::block_py::{
    BlockPyBlock, BlockPyLabel, BlockPyLiteral, BlockPyStmtFragment, BlockPyTerm,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreStringLiteral, GetAttr, LocatedCoreBlockPyExpr,
    LocatedName, Store, StructuredIf, StructuredInstr, WithMeta,
};
use crate::passes::ruff_to_blockpy::{
    lower_structured_located_blocks_to_bb_blocks, populate_exception_edge_args,
};
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;

#[test]
fn linearizes_structured_if_stmt_into_explicit_blocks() {
    let block: BlockPyBlock<LocatedCoreBlockPyExpr> = BlockPyBlock {
        label: BlockPyLabel::from_index(0),
        body: vec![
            StructuredInstr::Expr(
                Store::new(
                    LocatedName::from(ast::ExprName {
                        id: "x".into(),
                        ctx: ast::ExprContext::Store,
                        range: TextRange::default(),
                        node_index: ast::AtomicNodeIndex::default(),
                    }),
                    Box::new(core_name_expr("a")),
                )
                .into(),
            ),
            StructuredInstr::If(StructuredIf {
                test: core_name_expr("cond"),
                body: BlockPyStmtFragment::from_stmts(vec![StructuredInstr::Expr(
                    Store::new(
                        LocatedName::from(ast::ExprName {
                            id: "x".into(),
                            ctx: ast::ExprContext::Store,
                            range: TextRange::default(),
                            node_index: ast::AtomicNodeIndex::default(),
                        }),
                        Box::new(core_name_expr("b")),
                    )
                    .into(),
                )]),
                orelse: BlockPyStmtFragment::from_stmts(vec![StructuredInstr::Expr(
                    Store::new(
                        LocatedName::from(ast::ExprName {
                            id: "x".into(),
                            ctx: ast::ExprContext::Store,
                            range: TextRange::default(),
                            node_index: ast::AtomicNodeIndex::default(),
                        }),
                        Box::new(core_name_expr("c")),
                    )
                    .into(),
                )]),
            }),
            StructuredInstr::Expr(core_call_expr("sink", vec![core_name_expr("x")])),
        ],
        term: BlockPyTerm::Return(core_name_expr("__dp_NONE")),
        params: Vec::new(),
        exc_edge: None,
    };

    let blocks = lower_structured_located_blocks_to_bb_blocks(&[crate::block_py::CfgBlock {
        label: block.label,
        body: block.body,
        term: block.term,
        params: block.params,
        exc_edge: None,
    }]);

    assert_eq!(blocks.len(), 4, "{blocks:?}");
    assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
}

fn core_name_expr(name: &str) -> LocatedCoreBlockPyExpr {
    let name = LocatedName::from(ast::ExprName {
        id: name.into(),
        ctx: ast::ExprContext::Load,
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    });
    crate::block_py::Load::new(name.clone())
        .with_meta(crate::block_py::Meta::synthetic())
        .into()
}

fn core_call_expr(name: &str, args: Vec<LocatedCoreBlockPyExpr>) -> LocatedCoreBlockPyExpr {
    crate::block_py::core_call_expr_with_meta(
        core_name_expr(name),
        ast::AtomicNodeIndex::default(),
        TextRange::default(),
        args.into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        Vec::new(),
    )
}

fn core_string_expr(value: &str) -> LocatedCoreBlockPyExpr {
    LocatedCoreBlockPyExpr::Literal(
        BlockPyLiteral::StringLiteral(CoreStringLiteral {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            value: value.to_string(),
        })
        .into(),
    )
}

#[test]
fn rewrites_current_exception_placeholders_in_final_core_blocks() {
    let block: BlockPyBlock<LocatedCoreBlockPyExpr> = BlockPyBlock {
        label: BlockPyLabel::from_index(0),
        body: vec![StructuredInstr::Expr(core_call_expr(
            "current_exception",
            Vec::new(),
        ))],
        term: BlockPyTerm::Return(core_call_expr("current_exception", Vec::new())),
        params: vec![crate::block_py::BlockParam {
            name: "_dp_try_exc_0".to_string(),
            role: crate::block_py::BlockParamRole::Exception,
        }],
        exc_edge: None,
    };

    let lowered = lower_structured_located_blocks_to_bb_blocks(&[crate::block_py::CfgBlock {
        label: block.label,
        body: block.body,
        term: block.term,
        params: block.params,
        exc_edge: None,
    }]);
    let block = &lowered[0];

    let body_expr = &block.body[0];
    assert!(matches!(
        body_expr,
        CoreBlockPyExpr::Load(load) if load.name.id.as_str() == "_dp_try_exc_0"
    ));

    let BlockPyTerm::Return(CoreBlockPyExpr::Load(load)) = &block.term else {
        panic!("expected rewritten return expr");
    };
    assert_eq!(load.name.id.as_str(), "_dp_try_exc_0");
}

#[test]
fn rewrites_current_exception_inside_intrinsic_helper_args() {
    let block: BlockPyBlock<LocatedCoreBlockPyExpr> = BlockPyBlock {
        label: BlockPyLabel::from_index(0),
        body: Vec::new(),
        term: BlockPyTerm::Return(
            GetAttr::new(
                core_call_expr("current_exception", Vec::new()),
                CoreBlockPyExpr::Literal(
                    BlockPyLiteral::StringLiteral(CoreStringLiteral {
                        node_index: ast::AtomicNodeIndex::default(),
                        range: TextRange::default(),
                        value: "value".to_string(),
                    })
                    .into(),
                ),
            )
            .with_meta(crate::block_py::Meta::new(
                ast::AtomicNodeIndex::default(),
                TextRange::default(),
            ))
            .into(),
        ),
        params: vec![crate::block_py::BlockParam {
            name: "_dp_try_exc_0".to_string(),
            role: crate::block_py::BlockParamRole::Exception,
        }],
        exc_edge: None,
    };

    let lowered = lower_structured_located_blocks_to_bb_blocks(&[crate::block_py::CfgBlock {
        label: block.label,
        body: block.body,
        term: block.term,
        params: block.params,
        exc_edge: None,
    }]);
    let block = &lowered[0];

    let BlockPyTerm::Return(CoreBlockPyExpr::GetAttr(GetAttr { value, attr, .. })) = &block.term
    else {
        panic!("expected getattr operation");
    };
    assert!(matches!(
        value.as_ref(),
        CoreBlockPyExpr::Load(load) if load.name.id.as_str() == "_dp_try_exc_0"
    ));
    assert!(matches!(
        attr.as_ref(),
        CoreBlockPyExpr::Literal(literal)
            if matches!(
                literal.as_literal(),
                BlockPyLiteral::StringLiteral(CoreStringLiteral { value, .. }) if value == "value"
            )
    ));
}

#[test]
fn exception_edges_seed_hidden_try_exception_locals_from_current_exception() {
    let mut blocks: Vec<
        crate::block_py::CfgBlock<LocatedCoreBlockPyExpr, BlockPyTerm<LocatedCoreBlockPyExpr>>,
    > =
        vec![
        crate::block_py::CfgBlock {
            label: BlockPyLabel::from_index(0),
            body: Vec::new(),
            term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Return(
                <LocatedCoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
            ),
            params: vec![crate::block_py::BlockParam {
                name: "_dp_outer_exc".to_string(),
                role: crate::block_py::BlockParamRole::Exception,
            }],
            exc_edge: Some(crate::block_py::BlockPyEdge::new(BlockPyLabel::from_index(1))),
        },
        crate::block_py::CfgBlock {
            label: BlockPyLabel::from_index(1),
            body: Vec::new(),
            term: BlockPyTerm::<LocatedCoreBlockPyExpr>::Jump(
                crate::block_py::BlockPyEdge::with_args(
                    BlockPyLabel::from_index(2),
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
                    role: crate::block_py::BlockParamRole::AbruptPayload,
                },
            ],
            exc_edge: None,
        },
        crate::block_py::CfgBlock {
            label: BlockPyLabel::from_index(2),
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

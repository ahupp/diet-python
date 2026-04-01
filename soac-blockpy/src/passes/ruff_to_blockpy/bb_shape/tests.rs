use super::{
    lower_structured_blocks_to_bb_blocks, rewrite_current_exception_in_blockpy_expr,
    rewrite_current_exception_in_blockpy_term,
};
use crate::block_py::{
    BlockParam, BlockParamRole, BlockPyAssign, BlockPyIf, BlockPyIfTerm, BlockPyLabel,
    BlockPyNameLike, BlockPyStmt, BlockPyStmtFragment, BlockPyTerm, CfgBlock, CoreBlockPyCallArg,
    CoreBlockPyExpr, LocatedCoreBlockPyExpr, LocatedName, ResolvedStorageBlock,
    StructuredBlockPyStmt,
};
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;

pub(crate) fn lower_structured_core_blocks_to_bb_blocks<N>(
    blocks: &[CfgBlock<
        StructuredBlockPyStmt<CoreBlockPyExpr<N>, N>,
        BlockPyTerm<CoreBlockPyExpr<N>>,
    >],
) -> Vec<CfgBlock<BlockPyStmt<CoreBlockPyExpr<N>, N>, BlockPyTerm<CoreBlockPyExpr<N>>>>
where
    N: BlockPyNameLike,
{
    let mut normalized_blocks = blocks.to_vec();
    rewrite_current_exception_in_core_blocks_structured(&mut normalized_blocks);
    lower_structured_blocks_to_bb_blocks(&normalized_blocks)
}

pub(crate) fn lower_structured_located_blocks_to_bb_blocks(
    blocks: &[CfgBlock<
        StructuredBlockPyStmt<CoreBlockPyExpr<LocatedName>, LocatedName>,
        BlockPyTerm<LocatedCoreBlockPyExpr>,
    >],
) -> Vec<ResolvedStorageBlock> {
    lower_structured_core_blocks_to_bb_blocks(blocks)
}

fn rewrite_current_exception_in_core_blocks_structured<N>(
    blocks: &mut [CfgBlock<
        StructuredBlockPyStmt<CoreBlockPyExpr<N>, N>,
        BlockPyTerm<CoreBlockPyExpr<N>>,
    >],
) where
    N: BlockPyNameLike,
{
    for block in blocks {
        let Some(exc_name) = block.exception_param().map(ToString::to_string) else {
            continue;
        };
        for stmt in &mut block.body {
            rewrite_current_exception_in_blockpy_stmt(stmt, exc_name.as_str());
        }
        rewrite_current_exception_in_blockpy_term(&mut block.term, exc_name.as_str());
    }
}

fn rewrite_current_exception_in_blockpy_stmt<N>(
    stmt: &mut StructuredBlockPyStmt<CoreBlockPyExpr<N>, N>,
    exc_name: &str,
) where
    N: BlockPyNameLike,
{
    match stmt {
        StructuredBlockPyStmt::Assign(assign) => {
            rewrite_current_exception_in_blockpy_expr(&mut assign.value, exc_name);
        }
        StructuredBlockPyStmt::Expr(expr) => {
            rewrite_current_exception_in_blockpy_expr(expr, exc_name);
        }
        StructuredBlockPyStmt::Delete(_) => {}
        StructuredBlockPyStmt::If(if_stmt) => {
            rewrite_current_exception_in_blockpy_expr(&mut if_stmt.test, exc_name);
            for stmt in &mut if_stmt.body.body {
                rewrite_current_exception_in_blockpy_stmt(stmt, exc_name);
            }
            if let Some(term) = if_stmt.body.term.as_mut() {
                rewrite_current_exception_in_blockpy_term(term, exc_name);
            }
            for stmt in &mut if_stmt.orelse.body {
                rewrite_current_exception_in_blockpy_stmt(stmt, exc_name);
            }
            if let Some(term) = if_stmt.orelse.term.as_mut() {
                rewrite_current_exception_in_blockpy_term(term, exc_name);
            }
        }
    }
}

fn expr_name(name: &str, ctx: ast::ExprContext) -> ast::ExprName {
    ast::ExprName {
        id: name.into(),
        ctx,
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }
}

fn core_name_expr(name: &str) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Name(expr_name(name, ast::ExprContext::Load).into())
}

#[test]
fn lower_structured_core_blocks_to_bb_blocks_handles_unlocated_names() {
    let blocks = vec![CfgBlock {
        label: BlockPyLabel::from(0u32),
        body: vec![StructuredBlockPyStmt::If(BlockPyIf {
            test: crate::block_py::core_call_expr_with_meta(
                core_name_expr("current_exception"),
                ast::AtomicNodeIndex::default(),
                TextRange::default(),
                Vec::<CoreBlockPyCallArg<CoreBlockPyExpr>>::new(),
                Vec::new(),
            ),
            body: BlockPyStmtFragment::from_stmts(vec![StructuredBlockPyStmt::Assign(
                BlockPyAssign {
                    target: expr_name("x", ast::ExprContext::Store).into(),
                    value: core_name_expr("a"),
                },
            )]),
            orelse: BlockPyStmtFragment::from_stmts(vec![StructuredBlockPyStmt::Assign(
                BlockPyAssign {
                    target: expr_name("x", ast::ExprContext::Store).into(),
                    value: core_name_expr("b"),
                },
            )]),
        })],
        term: BlockPyTerm::Return(core_name_expr("__dp_NONE")),
        params: vec![BlockParam {
            name: "_dp_try_exc_0".to_string(),
            role: BlockParamRole::Exception,
        }],
        exc_edge: None,
    }];

    let lowered = lower_structured_core_blocks_to_bb_blocks(&blocks);

    assert_eq!(lowered.len(), 3, "{lowered:?}");
    let BlockPyTerm::IfTerm(BlockPyIfTerm {
        test: CoreBlockPyExpr::Name(name),
        ..
    }) = &lowered[0].term
    else {
        panic!("expected rewritten current-exception test");
    };
    assert_eq!(name.id_str(), "_dp_try_exc_0");
}

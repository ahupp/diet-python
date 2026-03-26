use super::{
    lower_structured_blocks_to_bb_blocks, rewrite_current_exception_in_blockpy_expr,
    rewrite_current_exception_in_blockpy_term,
};
use crate::block_py::{
    BbBlock, BbStmt, BlockParam, BlockParamRole, BlockPyAssign, BlockPyIf, BlockPyIfTerm,
    BlockPyLabel, BlockPyNameLike, BlockPyStmt, BlockPyStmtFragment, BlockPyTerm, CfgBlock,
    CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExpr, LocatedCoreBlockPyExpr, LocatedName,
};
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;
use std::collections::HashMap;

pub(crate) fn lower_structured_core_blocks_to_bb_blocks<N>(
    blocks: &[CfgBlock<BlockPyStmt<CoreBlockPyExpr<N>, N>, BlockPyTerm<CoreBlockPyExpr<N>>>],
    block_params: &HashMap<String, Vec<String>>,
) -> Vec<CfgBlock<BbStmt<CoreBlockPyExpr<N>, N>, BlockPyTerm<CoreBlockPyExpr<N>>>>
where
    N: BlockPyNameLike,
{
    let mut normalized_blocks = blocks.to_vec();
    rewrite_current_exception_in_core_blocks_structured(&mut normalized_blocks);
    lower_structured_blocks_to_bb_blocks(&normalized_blocks, block_params)
}

pub(crate) fn lower_structured_located_blocks_to_bb_blocks(
    blocks: &[CfgBlock<
        BlockPyStmt<CoreBlockPyExpr<LocatedName>, LocatedName>,
        BlockPyTerm<LocatedCoreBlockPyExpr>,
    >],
    block_params: &HashMap<String, Vec<String>>,
) -> Vec<BbBlock> {
    lower_structured_core_blocks_to_bb_blocks(blocks, block_params)
}

fn rewrite_current_exception_in_core_blocks_structured<N>(
    blocks: &mut [CfgBlock<BlockPyStmt<CoreBlockPyExpr<N>, N>, BlockPyTerm<CoreBlockPyExpr<N>>>],
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
    stmt: &mut BlockPyStmt<CoreBlockPyExpr<N>, N>,
    exc_name: &str,
) where
    N: BlockPyNameLike,
{
    match stmt {
        BlockPyStmt::Assign(assign) => {
            rewrite_current_exception_in_blockpy_expr(&mut assign.value, exc_name);
        }
        BlockPyStmt::Expr(expr) => {
            rewrite_current_exception_in_blockpy_expr(expr, exc_name);
        }
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::If(if_stmt) => {
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
    CoreBlockPyExpr::Name(expr_name(name, ast::ExprContext::Load))
}

#[test]
fn lower_structured_core_blocks_to_bb_blocks_handles_unlocated_names() {
    let blocks = vec![CfgBlock {
        label: BlockPyLabel::from("start"),
        body: vec![BlockPyStmt::If(BlockPyIf {
            test: CoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                func: Box::new(core_name_expr("__dp_current_exception")),
                args: Vec::<CoreBlockPyCallArg<CoreBlockPyExpr>>::new(),
                keywords: Vec::new(),
            }),
            body: BlockPyStmtFragment::from_stmts(vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("x", ast::ExprContext::Store),
                value: core_name_expr("a"),
            })]),
            orelse: BlockPyStmtFragment::from_stmts(vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("x", ast::ExprContext::Store),
                value: core_name_expr("b"),
            })]),
        })],
        term: BlockPyTerm::Return(core_name_expr("__dp_NONE")),
        params: vec![BlockParam {
            name: "_dp_try_exc_0".to_string(),
            role: BlockParamRole::Exception,
        }],
        exc_edge: None,
    }];

    let lowered = lower_structured_core_blocks_to_bb_blocks(&blocks, &HashMap::new());

    assert_eq!(lowered.len(), 3, "{lowered:?}");
    let BlockPyTerm::IfTerm(BlockPyIfTerm {
        test: CoreBlockPyExpr::Name(name),
        ..
    }) = &lowered[0].term
    else {
        panic!("expected rewritten current-exception test");
    };
    assert_eq!(name.id.as_str(), "_dp_try_exc_0");
}

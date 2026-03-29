use super::*;
use crate::block_py::{
    BlockPyCfgBlockBuilder, BlockPyIfTerm, BlockPyLabel, BlockPyRaise, BlockPyStmtFragmentBuilder,
    BlockPyTerm, Expr, StructuredBlockPyStmt,
};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ruff_to_blockpy::stmt_lowering::lower_nested_stmt_into_with_expr;

fn with_exc_meta(
    block: crate::block_py::CfgBlock<StructuredBlockPyStmt, BlockPyTerm>,
    exc_target: Option<&BlockPyLabel>,
) -> LoweredBlockPyBlock {
    crate::block_py::CfgBlock {
        label: block.label,
        body: block.body,
        term: block.term,
        params: block.params,
        exc_edge: exc_target.cloned().map(crate::block_py::BlockPyEdge::new),
    }
}

pub(crate) fn compat_block_from_blockpy(
    label: BlockPyLabel,
    body: Vec<Stmt>,
    term: BlockPyTerm,
) -> LoweredBlockPyBlock {
    compat_block_from_blockpy_with_exc_target(label, body, term, None)
}

pub(crate) fn compat_block_from_blockpy_with_exc_target(
    label: BlockPyLabel,
    body: Vec<Stmt>,
    term: BlockPyTerm,
    exc_target: Option<&BlockPyLabel>,
) -> LoweredBlockPyBlock {
    let body = lower_stmts_to_blockpy_stmts::<Expr>(&body).unwrap_or_else(|err| {
        panic!("failed to convert compatibility block body to BlockPy: {err}")
    });
    assert!(
        body.term.is_none(),
        "compatibility block body should not contain its own terminator"
    );
    let mut block = BlockPyCfgBlockBuilder::<StructuredBlockPyStmt, BlockPyTerm>::new(label);
    block.extend(body.body);
    block.set_term(term);
    with_exc_meta(block.finish(None), exc_target)
}

fn compat_block_builder_with_expr_setup(
    context: &Context,
    body: Vec<Stmt>,
) -> Result<BlockPyStmtFragmentBuilder<Expr>, String> {
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;
    for stmt in &body {
        lower_nested_stmt_into_with_expr(context, stmt, &mut out, None, &mut next_label_id)?;
    }
    Ok(out)
}

pub(crate) fn compat_if_jump_block_with_expr_setup_and_exc_target(
    context: &Context,
    label: BlockPyLabel,
    body: Vec<Stmt>,
    test: Expr,
    then_label: BlockPyLabel,
    else_label: BlockPyLabel,
    exc_target: Option<&BlockPyLabel>,
) -> Result<LoweredBlockPyBlock, String> {
    let mut out = compat_block_builder_with_expr_setup(context, body)?;
    let mut next_label_id = 0usize;
    let test = crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
        test,
        &mut out,
        None,
        &mut next_label_id,
    )?;
    let fragment = out.finish();
    assert!(
        fragment.term.is_none(),
        "compatibility block body should not contain its own terminator"
    );
    let mut block = BlockPyCfgBlockBuilder::<StructuredBlockPyStmt, BlockPyTerm>::new(label);
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::IfTerm(BlockPyIfTerm {
        test,
        then_label,
        else_label,
    }));
    Ok(with_exc_meta(block.finish(None), exc_target))
}

pub(crate) fn set_region_exc_param(
    blocks: &mut [LoweredBlockPyBlock],
    region: &std::ops::Range<usize>,
    exc_param: &str,
) {
    for block in &mut blocks[region.clone()] {
        block.set_exception_param(exc_param.to_string());
    }
}

pub(crate) fn emit_sequence_jump_block(
    blocks: &mut Vec<LoweredBlockPyBlock>,
    label: BlockPyLabel,
    linear: Vec<Stmt>,
    target_label: BlockPyLabel,
    exc_target: Option<&BlockPyLabel>,
) -> BlockPyLabel {
    blocks.push(compat_block_from_blockpy_with_exc_target(
        label.clone(),
        linear,
        BlockPyTerm::Jump(target_label.into()),
        exc_target,
    ));
    label
}

pub(crate) fn emit_sequence_return_block_with_expr_setup(
    context: &Context,
    blocks: &mut Vec<LoweredBlockPyBlock>,
    label: BlockPyLabel,
    linear: Vec<Stmt>,
    value: Option<Expr>,
    exc_target: Option<&BlockPyLabel>,
) -> Result<BlockPyLabel, String> {
    let mut out = compat_block_builder_with_expr_setup(context, linear)?;
    let mut next_label_id = 0usize;
    let value = value
        .map(|expr| {
            crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
                expr,
                &mut out,
                None,
                &mut next_label_id,
            )
        })
        .transpose()?;
    let fragment = out.finish();
    assert!(
        fragment.term.is_none(),
        "compatibility block body should not contain its own terminator"
    );
    let mut block =
        BlockPyCfgBlockBuilder::<StructuredBlockPyStmt, BlockPyTerm>::new(label.clone());
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::Return(
        value.unwrap_or_else(|| crate::py_expr!("__dp_NONE")),
    ));
    blocks.push(with_exc_meta(block.finish(None), exc_target));
    Ok(label)
}

pub(crate) fn emit_sequence_raise_block_with_expr_setup(
    context: &Context,
    blocks: &mut Vec<LoweredBlockPyBlock>,
    label: BlockPyLabel,
    linear: Vec<Stmt>,
    exc: BlockPyRaise,
    exc_target: Option<&BlockPyLabel>,
) -> Result<BlockPyLabel, String> {
    let mut out = compat_block_builder_with_expr_setup(context, linear)?;
    let mut next_label_id = 0usize;
    let exc = BlockPyRaise {
        exc: exc
            .exc
            .map(|expr| {
                crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
                    expr,
                    &mut out,
                    None,
                    &mut next_label_id,
                )
            })
            .transpose()?,
    };
    let fragment = out.finish();
    assert!(
        fragment.term.is_none(),
        "compatibility block body should not contain its own terminator"
    );
    let mut block =
        BlockPyCfgBlockBuilder::<StructuredBlockPyStmt, BlockPyTerm>::new(label.clone());
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::Raise(exc));
    blocks.push(with_exc_meta(block.finish(None), exc_target));
    Ok(label)
}

pub(crate) fn emit_if_branch_block_with_expr_setup(
    context: &Context,
    blocks: &mut Vec<LoweredBlockPyBlock>,
    label: BlockPyLabel,
    body: Vec<Stmt>,
    test: Expr,
    then_label: BlockPyLabel,
    else_label: BlockPyLabel,
    exc_target: Option<&BlockPyLabel>,
) -> Result<BlockPyLabel, String> {
    blocks.push(compat_if_jump_block_with_expr_setup_and_exc_target(
        context,
        label.clone(),
        body,
        test,
        then_label,
        else_label,
        exc_target,
    )?);
    Ok(label)
}

pub(crate) fn emit_simple_while_blocks_with_expr_setup(
    context: &Context,
    blocks: &mut Vec<LoweredBlockPyBlock>,
    test_label: BlockPyLabel,
    linear_label: Option<BlockPyLabel>,
    linear: Vec<Stmt>,
    test: Expr,
    body_entry: BlockPyLabel,
    cond_false_entry: BlockPyLabel,
    exc_target: Option<&BlockPyLabel>,
) -> Result<BlockPyLabel, String> {
    blocks.push(compat_if_jump_block_with_expr_setup_and_exc_target(
        context,
        test_label.clone(),
        Vec::new(),
        test,
        body_entry,
        cond_false_entry,
        exc_target,
    )?);
    if let Some(linear_label) = linear_label {
        blocks.push(compat_block_from_blockpy_with_exc_target(
            linear_label.clone(),
            linear,
            BlockPyTerm::Jump(test_label.into()),
            exc_target,
        ));
        Ok(linear_label)
    } else {
        Ok(test_label)
    }
}

pub(crate) fn emit_for_loop_blocks(
    blocks: &mut Vec<LoweredBlockPyBlock>,
    setup_label: BlockPyLabel,
    assign_label: BlockPyLabel,
    loop_check_label: BlockPyLabel,
    loop_continue_label: BlockPyLabel,
    linear: Vec<Stmt>,
    iter_name: &str,
    tmp_name: &str,
    iterable: Expr,
    is_async: bool,
    exhausted_entry: BlockPyLabel,
    body_entry: BlockPyLabel,
    assign_body: Vec<Stmt>,
    exc_target: Option<&BlockPyLabel>,
) -> BlockPyLabel {
    let iter_expr = py_expr!("{iter:id}", iter = iter_name);
    let tmp_expr = py_expr!("{tmp:id}", tmp = tmp_name);

    blocks.push(compat_block_from_blockpy_with_exc_target(
        assign_label.clone(),
        assign_body,
        BlockPyTerm::Jump(body_entry.into()),
        exc_target,
    ));

    let exhausted_test = py_expr!(
        "__dp_is_({value:expr}, __dp__.ITER_COMPLETE)",
        value = tmp_expr
    );
    let check_body = if is_async {
        vec![py_stmt!(
            "{tmp:id} = await __dp_anext_or_sentinel({iter:expr})",
            tmp = tmp_name,
            iter = iter_expr.clone(),
        )]
    } else {
        vec![py_stmt!(
            "{tmp:id} = __dp_next_or_sentinel({iter:expr})",
            tmp = tmp_name,
            iter = iter_expr.clone(),
        )]
    };
    blocks.push(compat_block_from_blockpy_with_exc_target(
        loop_check_label.clone(),
        check_body,
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: exhausted_test.into(),
            then_label: exhausted_entry,
            else_label: assign_label.clone(),
        }),
        exc_target,
    ));

    let mut setup_body = linear;
    if is_async {
        setup_body.push(py_stmt!(
            "{iter:id} = __dp_aiter({iterable:expr})",
            iter = iter_name,
            iterable = iterable,
        ));
    } else {
        setup_body.push(py_stmt!(
            "{iter:id} = __dp_iter({iterable:expr})",
            iter = iter_name,
            iterable = iterable,
        ));
    }
    blocks.push(compat_block_from_blockpy_with_exc_target(
        setup_label.clone(),
        setup_body,
        BlockPyTerm::Jump(loop_continue_label.into()),
        exc_target,
    ));
    setup_label
}

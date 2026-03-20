use super::*;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::block_py::{
    BlockPyBlock, BlockPyCfgBlockBuilder, BlockPyIfTerm, BlockPyLabel, BlockPyRaise, BlockPyStmt,
    BlockPyStmtFragmentBuilder, BlockPyTerm, Expr,
};
use crate::basic_block::ruff_to_blockpy::stmt_lowering::lower_nested_stmt_into_with_expr;

pub(crate) fn compat_block_from_blockpy(
    label: String,
    body: Vec<Stmt>,
    term: BlockPyTerm,
) -> BlockPyBlock {
    let body = lower_stmts_to_blockpy_stmts::<Expr>(&body).unwrap_or_else(|err| {
        panic!("failed to convert compatibility block body to BlockPy: {err}")
    });
    assert!(
        body.term.is_none(),
        "compatibility block body should not contain its own terminator"
    );
    let mut block =
        BlockPyCfgBlockBuilder::<BlockPyStmt, BlockPyTerm>::new(BlockPyLabel::from(label));
    block.extend(body.body);
    block.set_term(term);
    block.finish(None)
}

pub(crate) fn compat_if_jump_block(
    label: String,
    body: Vec<Stmt>,
    test: Expr,
    then_label: String,
    else_label: String,
) -> BlockPyBlock {
    compat_block_from_blockpy(
        label.clone(),
        body,
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: test.into(),
            then_label: BlockPyLabel::from(then_label),
            else_label: BlockPyLabel::from(else_label),
        }),
    )
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

pub(crate) fn compat_if_jump_block_with_expr_setup(
    context: &Context,
    label: String,
    body: Vec<Stmt>,
    test: Expr,
    then_label: String,
    else_label: String,
) -> Result<BlockPyBlock, String> {
    let mut out = compat_block_builder_with_expr_setup(context, body)?;
    let mut next_label_id = 0usize;
    let test = crate::basic_block::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
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
    let mut block =
        BlockPyCfgBlockBuilder::<BlockPyStmt, BlockPyTerm>::new(BlockPyLabel::from(label));
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::IfTerm(BlockPyIfTerm {
        test,
        then_label: BlockPyLabel::from(then_label),
        else_label: BlockPyLabel::from(else_label),
    }));
    Ok(block.finish(None))
}

pub(crate) fn compat_jump_block_from_blockpy(
    label: String,
    body: Vec<Stmt>,
    target_label: String,
) -> BlockPyBlock {
    compat_block_from_blockpy(
        label,
        body,
        BlockPyTerm::Jump(BlockPyLabel::from(target_label).into()),
    )
}

pub(crate) fn set_region_exc_param(
    blocks: &mut [BlockPyBlock],
    region: &std::ops::Range<usize>,
    exc_param: &str,
) {
    for block in &mut blocks[region.clone()] {
        block.set_exception_param(exc_param.to_string());
    }
}

pub(crate) fn emit_sequence_jump_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    target_label: String,
) -> String {
    blocks.push(compat_jump_block_from_blockpy(
        label.clone(),
        linear,
        target_label,
    ));
    label
}

pub(crate) fn emit_sequence_return_block_with_expr_setup(
    context: &Context,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    value: Option<Expr>,
) -> Result<String, String> {
    let mut out = compat_block_builder_with_expr_setup(context, linear)?;
    let mut next_label_id = 0usize;
    let value = value
        .map(|expr| {
            crate::basic_block::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
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
        BlockPyCfgBlockBuilder::<BlockPyStmt, BlockPyTerm>::new(BlockPyLabel::from(label.clone()));
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::Return(value));
    blocks.push(block.finish(None));
    Ok(label)
}

pub(crate) fn emit_sequence_raise_block_with_expr_setup(
    context: &Context,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    exc: BlockPyRaise,
) -> Result<String, String> {
    let mut out = compat_block_builder_with_expr_setup(context, linear)?;
    let mut next_label_id = 0usize;
    let exc = BlockPyRaise {
        exc: exc
            .exc
            .map(|expr| {
                crate::basic_block::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup(
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
        BlockPyCfgBlockBuilder::<BlockPyStmt, BlockPyTerm>::new(BlockPyLabel::from(label.clone()));
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::Raise(exc));
    blocks.push(block.finish(None));
    Ok(label)
}

pub(crate) fn emit_if_branch_block_with_expr_setup(
    context: &Context,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    body: Vec<Stmt>,
    test: Expr,
    then_label: String,
    else_label: String,
) -> Result<String, String> {
    blocks.push(compat_if_jump_block_with_expr_setup(
        context,
        label.clone(),
        body,
        test,
        then_label,
        else_label,
    )?);
    Ok(label)
}

pub(crate) fn emit_simple_while_blocks_with_expr_setup(
    context: &Context,
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    linear: Vec<Stmt>,
    test: Expr,
    body_entry: String,
    cond_false_entry: String,
) -> Result<String, String> {
    blocks.push(compat_if_jump_block_with_expr_setup(
        context,
        test_label.clone(),
        Vec::new(),
        test,
        body_entry,
        cond_false_entry,
    )?);
    if let Some(linear_label) = linear_label {
        blocks.push(compat_jump_block_from_blockpy(
            linear_label.clone(),
            linear,
            test_label,
        ));
        Ok(linear_label)
    } else {
        Ok(test_label)
    }
}

pub(crate) fn emit_for_loop_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    setup_label: String,
    assign_label: String,
    loop_check_label: String,
    loop_continue_label: String,
    linear: Vec<Stmt>,
    iter_name: &str,
    tmp_name: &str,
    iterable: Expr,
    is_async: bool,
    exhausted_entry: String,
    body_entry: String,
    assign_body: Vec<Stmt>,
) -> String {
    let iter_expr = py_expr!("{iter:id}", iter = iter_name);
    let tmp_expr = py_expr!("{tmp:id}", tmp = tmp_name);

    blocks.push(compat_block_from_blockpy(
        assign_label.clone(),
        assign_body,
        BlockPyTerm::Jump(BlockPyLabel::from(body_entry).into()),
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
    blocks.push(compat_if_jump_block(
        loop_check_label.clone(),
        check_body,
        exhausted_test,
        exhausted_entry,
        assign_label,
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
    blocks.push(compat_block_from_blockpy(
        setup_label.clone(),
        setup_body,
        BlockPyTerm::Jump(BlockPyLabel::from(loop_continue_label).into()),
    ));
    setup_label
}

pub(crate) fn compat_next_temp(prefix: &str, next_id: &mut usize) -> String {
    let current = *next_id;
    *next_id += 1;
    format!("_dp_{prefix}_{current}")
}

pub(crate) fn compat_sanitize_ident(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn compat_next_label(fn_name: &str, next_id: &mut usize) -> String {
    let current = *next_id;
    *next_id += 1;
    format!("_dp_bb_{}_{}", compat_sanitize_ident(fn_name), current)
}

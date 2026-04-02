use super::*;
use crate::block_py::{
    BlockPyCfgBlockBuilder, BlockPyIfTerm, BlockPyLabel, BlockPyRaise, BlockPyStmtFragmentBuilder,
    BlockPyTerm, Expr, ImplicitNoneExpr, Instr, StructuredInstrFor,
};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ruff_to_blockpy::stmt_lowering::lower_nested_stmt_into_with_expr;

fn with_exc_meta<E: Instr>(
    block: crate::block_py::CfgBlock<StructuredInstrFor<E>, BlockPyTerm<E>>,
    exc_target: Option<&BlockPyLabel>,
) -> LoweredBlockPyBlock<E> {
    crate::block_py::CfgBlock {
        label: block.label,
        body: block.body,
        term: block.term,
        params: block.params,
        exc_edge: exc_target.cloned().map(crate::block_py::BlockPyEdge::new),
    }
}

pub(crate) fn compat_block_from_blockpy_with_exc_target_and_expr<E>(
    label: BlockPyLabel,
    body: Vec<Stmt>,
    term: BlockPyTerm<E>,
    exc_target: Option<&BlockPyLabel>,
) -> LoweredBlockPyBlock<E>
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    let body = lower_stmts_to_blockpy_stmts::<E>(&body).unwrap_or_else(|err| {
        panic!("failed to convert compatibility block body to BlockPy: {err}")
    });
    assert!(
        body.term.is_none(),
        "compatibility block body should not contain its own terminator"
    );
    let mut block = BlockPyCfgBlockBuilder::<StructuredInstrFor<E>, BlockPyTerm<E>>::new(label);
    block.extend(body.body);
    block.set_term(term);
    with_exc_meta(block.finish(None), exc_target)
}

fn compat_block_builder_with_expr_setup_and_expr<E>(
    context: &Context,
    body: Vec<Stmt>,
) -> Result<BlockPyStmtFragmentBuilder<E>, String>
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    let mut out = BlockPyStmtFragmentBuilder::<E>::new();
    let mut next_label_id = 0usize;
    for stmt in &body {
        lower_nested_stmt_into_with_expr(context, stmt, &mut out, None, &mut next_label_id)?;
    }
    Ok(out)
}

pub(crate) fn compat_if_jump_block_with_expr_setup_and_exc_target_and_expr<E>(
    context: &Context,
    label: BlockPyLabel,
    body: Vec<Stmt>,
    test: Expr,
    then_label: BlockPyLabel,
    else_label: BlockPyLabel,
    exc_target: Option<&BlockPyLabel>,
) -> Result<LoweredBlockPyBlock<E>, String>
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    let mut out = compat_block_builder_with_expr_setup_and_expr::<E>(context, body)?;
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
    let mut block = BlockPyCfgBlockBuilder::<StructuredInstrFor<E>, BlockPyTerm<E>>::new(label);
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::IfTerm(BlockPyIfTerm {
        test,
        then_label,
        else_label,
    }));
    Ok(with_exc_meta(block.finish(None), exc_target))
}

pub(crate) fn set_region_exc_param<E: Instr>(
    blocks: &mut [LoweredBlockPyBlock<E>],
    region: &std::ops::Range<usize>,
    exc_param: &str,
) {
    for block in &mut blocks[region.clone()] {
        let old_exc_param = block.exception_param().map(ToString::to_string);
        block.set_exception_param(exc_param.to_string());
        if let Some(old_exc_param) = old_exc_param {
            if old_exc_param != exc_param {
                rename_exception_edge_args(block, old_exc_param.as_str(), exc_param);
            }
        }
    }
}

fn rename_exception_edge_args<E: Instr>(
    block: &mut LoweredBlockPyBlock<E>,
    old_exc_param: &str,
    new_exc_param: &str,
) {
    fn rewrite_edge_args(
        args: &mut [crate::block_py::BlockArg],
        old_exc_param: &str,
        new_exc_param: &str,
    ) {
        for arg in args {
            if let crate::block_py::BlockArg::Name(name) = arg {
                if name == old_exc_param {
                    *name = new_exc_param.to_string();
                }
            }
        }
    }

    if let BlockPyTerm::Jump(edge) = &mut block.term {
        rewrite_edge_args(&mut edge.args, old_exc_param, new_exc_param);
    }
    if let Some(edge) = &mut block.exc_edge {
        rewrite_edge_args(&mut edge.args, old_exc_param, new_exc_param);
    }
}

pub(crate) fn emit_sequence_jump_block<E>(
    blocks: &mut Vec<LoweredBlockPyBlock<E>>,
    label: BlockPyLabel,
    linear: Vec<Stmt>,
    target_label: BlockPyLabel,
    exc_target: Option<&BlockPyLabel>,
) -> BlockPyLabel
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    blocks.push(compat_block_from_blockpy_with_exc_target_and_expr(
        label.clone(),
        linear,
        BlockPyTerm::Jump(BlockPyEdge::new(target_label)),
        exc_target,
    ));
    label
}

pub(crate) fn emit_sequence_return_block_with_expr_setup_and_expr<E>(
    context: &Context,
    blocks: &mut Vec<LoweredBlockPyBlock<E>>,
    label: BlockPyLabel,
    linear: Vec<Stmt>,
    value: Option<Expr>,
    exc_target: Option<&BlockPyLabel>,
) -> Result<BlockPyLabel, String>
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    let mut out = compat_block_builder_with_expr_setup_and_expr::<E>(context, linear)?;
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
        BlockPyCfgBlockBuilder::<StructuredInstrFor<E>, BlockPyTerm<E>>::new(label.clone());
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::Return(
        value.unwrap_or_else(E::implicit_none_expr),
    ));
    blocks.push(with_exc_meta(block.finish(None), exc_target));
    Ok(label)
}

pub(crate) fn emit_sequence_raise_block_with_expr_setup_and_expr<E>(
    context: &Context,
    blocks: &mut Vec<LoweredBlockPyBlock<E>>,
    label: BlockPyLabel,
    linear: Vec<Stmt>,
    exc: BlockPyRaise,
    exc_target: Option<&BlockPyLabel>,
) -> Result<BlockPyLabel, String>
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    let mut out = compat_block_builder_with_expr_setup_and_expr::<E>(context, linear)?;
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
        BlockPyCfgBlockBuilder::<StructuredInstrFor<E>, BlockPyTerm<E>>::new(label.clone());
    block.extend(fragment.body);
    block.set_term(BlockPyTerm::Raise(exc));
    blocks.push(with_exc_meta(block.finish(None), exc_target));
    Ok(label)
}

pub(crate) fn emit_if_branch_block_with_expr_setup_and_expr<E>(
    context: &Context,
    blocks: &mut Vec<LoweredBlockPyBlock<E>>,
    label: BlockPyLabel,
    body: Vec<Stmt>,
    test: Expr,
    then_label: BlockPyLabel,
    else_label: BlockPyLabel,
    exc_target: Option<&BlockPyLabel>,
) -> Result<BlockPyLabel, String>
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    blocks.push(
        compat_if_jump_block_with_expr_setup_and_exc_target_and_expr(
            context,
            label.clone(),
            body,
            test,
            then_label,
            else_label,
            exc_target,
        )?,
    );
    Ok(label)
}

pub(crate) fn emit_simple_while_blocks_with_expr_setup_and_expr<E>(
    context: &Context,
    blocks: &mut Vec<LoweredBlockPyBlock<E>>,
    test_label: BlockPyLabel,
    linear_label: Option<BlockPyLabel>,
    linear: Vec<Stmt>,
    test: Expr,
    body_entry: BlockPyLabel,
    cond_false_entry: BlockPyLabel,
    exc_target: Option<&BlockPyLabel>,
) -> Result<BlockPyLabel, String>
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    blocks.push(
        compat_if_jump_block_with_expr_setup_and_exc_target_and_expr(
            context,
            test_label.clone(),
            Vec::new(),
            test,
            body_entry,
            cond_false_entry,
            exc_target,
        )?,
    );
    if let Some(linear_label) = linear_label {
        blocks.push(compat_block_from_blockpy_with_exc_target_and_expr(
            linear_label.clone(),
            linear,
            BlockPyTerm::Jump(BlockPyEdge::new(test_label)),
            exc_target,
        ));
        Ok(linear_label)
    } else {
        Ok(test_label)
    }
}

pub(crate) fn emit_for_loop_blocks<E>(
    blocks: &mut Vec<LoweredBlockPyBlock<E>>,
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
) -> BlockPyLabel
where
    E: RuffToBlockPyExpr + ImplicitNoneExpr,
{
    let iter_expr = py_expr!("{iter:id}", iter = iter_name);
    let tmp_expr = py_expr!("{tmp:id}", tmp = tmp_name);

    blocks.push(compat_block_from_blockpy_with_exc_target_and_expr(
        assign_label.clone(),
        assign_body,
        BlockPyTerm::Jump(BlockPyEdge::new(body_entry)),
        exc_target,
    ));

    let exhausted_test = py_expr!("{value:expr} is __soac__.ITER_COMPLETE", value = tmp_expr);
    let check_body = if is_async {
        vec![py_stmt!(
            "{tmp:id} = await __soac__.anext_or_sentinel({iter:expr})",
            tmp = tmp_name,
            iter = iter_expr.clone(),
        )]
    } else {
        vec![py_stmt!(
            "{tmp:id} = __soac__.next_or_sentinel({iter:expr})",
            tmp = tmp_name,
            iter = iter_expr.clone(),
        )]
    };
    blocks.push(compat_block_from_blockpy_with_exc_target_and_expr(
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
            "{iter:id} = __soac__.aiter({iterable:expr})",
            iter = iter_name,
            iterable = iterable,
        ));
    } else {
        setup_body.push(py_stmt!(
            "{iter:id} = __soac__.iter({iterable:expr})",
            iter = iter_name,
            iterable = iterable,
        ));
    }
    blocks.push(compat_block_from_blockpy_with_exc_target_and_expr(
        setup_label.clone(),
        setup_body,
        BlockPyTerm::Jump(BlockPyEdge::new(loop_continue_label)),
        exc_target,
    ));
    setup_label
}

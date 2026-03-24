use crate::block_py::intrinsics::STORE_GLOBAL_INTRINSIC;
use crate::block_py::{
    core_positional_call_expr_with_meta, core_positional_intrinsic_expr_with_meta, BindingTarget,
    BlockPyAssign, BlockPyCallableSemanticInfo, BlockPyCfgFragment, BlockPyFunction, BlockPyIf,
    BlockPyModule, BlockPyStmt, BlockPyTerm, CoreBlockPyExpr, CoreBlockPyLiteral,
    CoreStringLiteral,
};
use crate::passes::CoreBlockPyPass;

fn core_string_expr(
    value: String,
    node_index: ruff_python_ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(CoreStringLiteral {
        node_index,
        range,
        value,
    }))
}

fn rewrite_global_binding_assign(
    assign: BlockPyAssign<CoreBlockPyExpr>,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = assign.target.node_index.clone();
    let range = assign.target.range;
    let bind_name = assign.target.id.to_string();
    let globals_expr =
        core_positional_call_expr_with_meta("globals", node_index.clone(), range, Vec::new());
    BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
        &STORE_GLOBAL_INTRINSIC,
        node_index.clone(),
        range,
        vec![
            globals_expr,
            core_string_expr(bind_name, node_index, range),
            assign.value,
        ],
    ))
}

fn rewrite_name_binding_stmt(
    stmt: BlockPyStmt<CoreBlockPyExpr>,
    semantic: &BlockPyCallableSemanticInfo,
) -> BlockPyStmt<CoreBlockPyExpr> {
    match stmt {
        BlockPyStmt::Assign(assign)
            if semantic.binding_target_for_name(assign.target.id.as_str())
                == BindingTarget::ModuleGlobal =>
        {
            rewrite_global_binding_assign(assign)
        }
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => BlockPyStmt::If(BlockPyIf {
            test,
            body: rewrite_name_binding_fragment(body, semantic),
            orelse: rewrite_name_binding_fragment(orelse, semantic),
        }),
        other => other,
    }
}

fn rewrite_name_binding_fragment(
    fragment: BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>,
    semantic: &BlockPyCallableSemanticInfo,
) -> BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>> {
    BlockPyCfgFragment {
        body: fragment
            .body
            .into_iter()
            .map(|stmt| rewrite_name_binding_stmt(stmt, semantic))
            .collect(),
        term: fragment.term,
    }
}

fn lower_name_binding_callable(
    mut callable: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<CoreBlockPyPass> {
    let semantic = callable.semantic.clone();
    for block in &mut callable.blocks {
        let body = std::mem::take(&mut block.body);
        block.body = body
            .into_iter()
            .map(|stmt| rewrite_name_binding_stmt(stmt, &semantic))
            .collect();
    }
    callable
}

pub(crate) fn lower_name_binding_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPass>,
) -> BlockPyModule<CoreBlockPyPass> {
    module.map_callable_defs(lower_name_binding_callable)
}

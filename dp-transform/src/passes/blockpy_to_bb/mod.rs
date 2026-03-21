mod codegen_normalize;
mod exception_pass;

use super::blockpy_generators::lower_generator_like_function;
use super::core_eval_order::make_eval_order_explicit_in_core_callable_def_without_await;
use super::ruff_to_blockpy::{
    lowered_exception_edges, recompute_lowered_block_params, should_include_closure_storage_aliases,
};
use crate::block_py::cfg::linearize_structured_ifs;
use crate::block_py::{
    BbBlock, BbBlockMeta, BbStmt, BlockArg, BlockParam, BlockParamRole, BlockPyEdge,
    BlockPyFunction, BlockPyFunctionKind, BlockPyIfTerm, BlockPyModule, BlockPyStmt, BlockPyTerm,
    CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExprWithoutAwaitOrYield, CoreBlockPyKeywordArg,
    CoreBlockPyLiteral,
};
use crate::passes::{
    BbBlockPyPass, CoreBlockPyPassWithoutAwait, CoreBlockPyPassWithoutAwaitOrYield,
};
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;
use std::collections::HashMap;
use std::collections::HashSet;

pub use codegen_normalize::normalize_bb_module_for_codegen;
pub use exception_pass::lower_try_jump_exception_flow;

pub(crate) fn lower_yield_in_lowered_core_blockpy_module_bundle(
    module: BlockPyModule<CoreBlockPyPassWithoutAwait>,
) -> BlockPyModule<CoreBlockPyPassWithoutAwaitOrYield> {
    let module =
        module.map_callable_defs(make_eval_order_explicit_in_core_callable_def_without_await);
    let mut next_hidden_function_id = module
        .callable_defs
        .iter()
        .map(|callable| callable.function_id.0)
        .max()
        .map(|value| value + 1)
        .unwrap_or(0);
    let mut callable_defs = Vec::new();
    for callable in module.callable_defs {
        match callable.kind {
            BlockPyFunctionKind::Function => {
                let qualname = callable.names.qualname.clone();
                callable_defs.push(callable.try_into().unwrap_or_else(|_| {
                    panic!(
                        "core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for {}",
                        qualname
                    )
                }));
            }
            BlockPyFunctionKind::Generator
            | BlockPyFunctionKind::Coroutine
            | BlockPyFunctionKind::AsyncGenerator => {
                let resume_function_id = crate::block_py::FunctionId(next_hidden_function_id);
                next_hidden_function_id += 1;
                callable_defs.extend(lower_generator_like_function(callable, resume_function_id));
            }
        }
    }
    BlockPyModule { callable_defs }
}

pub(crate) fn lower_core_blockpy_module_bundle_to_bb_module(
    module: BlockPyModule<CoreBlockPyPassWithoutAwaitOrYield>,
) -> BlockPyModule<BbBlockPyPass> {
    module.map_callable_defs(lower_core_blockpy_function_to_bb_function)
}

pub(crate) fn lower_core_blockpy_function_to_bb_function(
    lowered: BlockPyFunction<CoreBlockPyPassWithoutAwaitOrYield>,
) -> BlockPyFunction<BbBlockPyPass> {
    let BlockPyFunction {
        function_id,
        names,
        kind,
        params,
        param_defaults,
        blocks,
        doc,
        closure_layout,
        facts,
        try_regions,
        extra: (),
    } = lowered;
    let lowered_view: BlockPyFunction<CoreBlockPyPassWithoutAwaitOrYield> = BlockPyFunction {
        function_id,
        names: names.clone(),
        kind,
        params: params.clone(),
        param_defaults: param_defaults.clone(),
        blocks: blocks.clone(),
        doc: doc.clone(),
        closure_layout: closure_layout.clone(),
        facts: facts.clone(),
        try_regions: try_regions.clone(),
        extra: (),
    };
    let block_params = recompute_lowered_block_params(
        &lowered_view,
        should_include_closure_storage_aliases(&lowered_view),
    );
    let exception_edges = lowered_exception_edges(&lowered_view.blocks);
    BlockPyFunction {
        function_id,
        names,
        kind,
        params,
        param_defaults,
        blocks: lower_blockpy_blocks_to_bb_blocks(&blocks, &block_params, &exception_edges),
        doc,
        closure_layout,
        facts,
        try_regions,
        extra: (),
    }
}

fn lower_blockpy_blocks_to_bb_blocks(
    blocks: &[crate::block_py::CfgBlock<
        BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>,
        BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>,
        Option<crate::block_py::BlockPyLabel>,
    >],
    block_params: &HashMap<String, Vec<String>>,
    exception_edges: &HashMap<String, Option<String>>,
) -> Vec<BbBlock> {
    let (linear_blocks, linear_block_params, linear_exception_edges) =
        linearize_structured_ifs(blocks, block_params, exception_edges);
    let mut bb_blocks = linear_blocks
        .iter()
        .map(|block| {
            let current_exception_name = block.exception_param();
            let mut normalized_body = block.body.clone();
            if let Some(exc_name) = current_exception_name {
                for stmt in &mut normalized_body {
                    rewrite_current_exception_in_blockpy_stmt(stmt, exc_name);
                }
            }
            let mut normalized_term = block.term.clone();
            if let Some(exc_name) = current_exception_name {
                rewrite_current_exception_in_blockpy_term(&mut normalized_term, exc_name);
            }
            let exc_edge = linear_exception_edges
                .get(block.label.as_str())
                .cloned()
                .flatten()
                .map(crate::block_py::BlockPyLabel::from)
                .map(BlockPyEdge::new);
            let ops = normalized_body
                .into_iter()
                .map(bb_stmt_from_blockpy_stmt)
                .collect::<Vec<_>>();
            let semantic_param_names = block
                .param_names()
                .map(ToString::to_string)
                .collect::<HashSet<_>>();
            let mut params = linear_block_params
                .get(block.label.as_str())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|param| !semantic_param_names.contains(param))
                .map(|name| BlockParam {
                    name,
                    role: BlockParamRole::Local,
                })
                .collect::<Vec<_>>();
            params.extend(block.bb_params().cloned());
            BbBlock {
                label: block.label.clone(),
                body: ops,
                term: normalized_term,
                params,
                meta: BbBlockMeta { exc_edge },
            }
        })
        .collect::<Vec<_>>();
    populate_exception_edge_args(&mut bb_blocks);
    bb_blocks
}

pub(super) fn populate_exception_edge_args(blocks: &mut [BbBlock]) {
    let label_to_index = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label.as_str().to_string(), index))
        .collect::<HashMap<_, _>>();
    for block_index in 0..blocks.len() {
        let Some(exc_target_label) = blocks[block_index]
            .meta
            .exc_edge
            .as_ref()
            .map(|edge| edge.target.clone())
        else {
            continue;
        };
        let Some(target_index) = label_to_index.get(exc_target_label.as_str()).copied() else {
            continue;
        };
        let source_params = blocks[block_index].param_name_vec();
        let source_has_owner = source_params
            .iter()
            .any(|param| param == "_dp_self" || param == "_dp_state");
        let target_params = blocks[target_index].param_name_vec();
        let exc_name = blocks[target_index]
            .exception_param()
            .map(ToString::to_string);
        let args = target_params
            .into_iter()
            .map(|target_param| {
                if exc_name.as_deref() == Some(target_param.as_str()) {
                    BlockArg::CurrentException
                } else if source_params.iter().any(|param| param == &target_param)
                    || source_has_owner
                {
                    BlockArg::Name(target_param)
                } else {
                    BlockArg::None
                }
            })
            .collect();
        blocks[block_index].meta.exc_edge = Some(BlockPyEdge::with_args(exc_target_label, args));
    }
}

fn rewrite_current_exception_in_blockpy_stmt(
    stmt: &mut BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>,
    exc_name: &str,
) {
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

fn rewrite_current_exception_in_blockpy_term(
    term: &mut BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>,
    exc_name: &str,
) {
    match term {
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            rewrite_current_exception_in_blockpy_expr(test, exc_name);
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_current_exception_in_blockpy_expr(&mut branch.index, exc_name);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                rewrite_current_exception_in_blockpy_expr(exc, exc_name);
            } else {
                raise_stmt.exc = Some(current_exception_name_expr(exc_name).into());
            }
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value.as_mut() {
                rewrite_current_exception_in_blockpy_expr(value, exc_name);
            }
        }
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
    }
}

fn rewrite_current_exception_in_blockpy_expr(
    expr: &mut CoreBlockPyExprWithoutAwaitOrYield,
    exc_name: &str,
) {
    if let CoreBlockPyExprWithoutAwaitOrYield::Call(call) = expr {
        rewrite_current_exception_in_blockpy_expr(call.func.as_mut(), exc_name);
        for arg in &mut call.args {
            match arg {
                CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                    rewrite_current_exception_in_blockpy_expr(value, exc_name);
                }
            }
        }
        for keyword in &mut call.keywords {
            match keyword {
                CoreBlockPyKeywordArg::Named { value, .. }
                | CoreBlockPyKeywordArg::Starred(value) => {
                    rewrite_current_exception_in_blockpy_expr(value, exc_name);
                }
            }
        }
    }

    if is_current_exception_call(expr) {
        *expr = current_exception_name_expr(exc_name);
    } else if is_exc_info_call(expr) {
        *expr = current_exception_info_expr(exc_name);
    }
}

fn is_current_exception_call(expr: &CoreBlockPyExprWithoutAwaitOrYield) -> bool {
    let CoreBlockPyExprWithoutAwaitOrYield::Call(call) = expr else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "current_exception")
}

fn is_exc_info_call(expr: &CoreBlockPyExprWithoutAwaitOrYield) -> bool {
    let CoreBlockPyExprWithoutAwaitOrYield::Call(call) = expr else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "exc_info")
}

fn is_dp_lookup_call_expr(func: &CoreBlockPyExprWithoutAwaitOrYield, attr_name: &str) -> bool {
    match func {
        CoreBlockPyExprWithoutAwaitOrYield::Name(name) => {
            name.id.as_str() == format!("__dp_{attr_name}")
        }
        CoreBlockPyExprWithoutAwaitOrYield::Call(call)
            if call.keywords.is_empty() && call.args.len() == 2 =>
        {
            matches!(
                call.func.as_ref(),
                CoreBlockPyExprWithoutAwaitOrYield::Name(name)
                    if name.id.as_str() == "__dp_getattr"
            ) && matches!(
                &call.args[0],
                CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwaitOrYield::Name(base))
                    if base.id.as_str() == "__dp__"
            ) && expr_static_str(match &call.args[1] {
                CoreBlockPyCallArg::Positional(value) => value,
                CoreBlockPyCallArg::Starred(_) => return false,
            }) == Some(attr_name.to_string())
        }
        _ => false,
    }
}

fn expr_static_str(expr: &CoreBlockPyExprWithoutAwaitOrYield) -> Option<String> {
    match expr {
        CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::StringLiteral(value)) => {
            Some(value.value.clone())
        }
        CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::BytesLiteral(bytes)) => {
            String::from_utf8(bytes.value.clone()).ok()
        }
        CoreBlockPyExprWithoutAwaitOrYield::Call(call)
            if call.keywords.is_empty()
                && call.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    CoreBlockPyExprWithoutAwaitOrYield::Name(name)
                        if matches!(
                            name.id.as_str(),
                            "__dp_decode_literal_bytes" | "__dp_decode_literal_source_bytes"
                        )
                ) =>
        {
            match &call.args[0] {
                CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwaitOrYield::Literal(
                    CoreBlockPyLiteral::BytesLiteral(bytes),
                )) => String::from_utf8(bytes.value.clone()).ok(),
                _ => None,
            }
        }
        _ => None,
    }
}

fn current_exception_name_expr(exc_name: &str) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Name(ast::ExprName {
        id: exc_name.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    })
}

fn current_exception_info_expr(exc_name: &str) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Call(CoreBlockPyCall {
        node_index: compat_node_index(),
        range: compat_range(),
        func: Box::new(CoreBlockPyExprWithoutAwaitOrYield::Name(ast::ExprName {
            id: "__dp_exc_info_from_exception".into(),
            ctx: ast::ExprContext::Load,
            range: compat_range(),
            node_index: compat_node_index(),
        })),
        args: vec![CoreBlockPyCallArg::Positional(current_exception_name_expr(
            exc_name,
        ))],
        keywords: Vec::new(),
    })
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

fn bb_stmt_from_blockpy_stmt(stmt: BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>) -> BbStmt {
    match stmt {
        BlockPyStmt::Assign(_) | BlockPyStmt::Expr(_) | BlockPyStmt::Delete(_) => stmt,
        BlockPyStmt::If(_) => {
            panic!("structured BlockPy If reached BB block body after linearization")
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::block_py::{
        BlockPyAssign, BlockPyBlock, BlockPyIf, BlockPyLabel, BlockPyStmt, BlockPyStmtFragment,
        BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExprWithoutAwaitOrYield,
    };
    use crate::passes::blockpy_to_bb::lower_blockpy_blocks_to_bb_blocks;
    use ruff_python_ast::{self as ast};
    use ruff_text_size::TextRange;
    use std::collections::HashMap;

    #[test]
    fn linearizes_structured_if_stmt_into_explicit_blocks() {
        let block = BlockPyBlock {
            label: BlockPyLabel::from("start"),
            body: vec![
                BlockPyStmt::Assign(BlockPyAssign {
                    target: ast::ExprName {
                        id: "x".into(),
                        ctx: ast::ExprContext::Store,
                        range: TextRange::default(),
                        node_index: ast::AtomicNodeIndex::default(),
                    },
                    value: core_name_expr("a"),
                }),
                BlockPyStmt::If(BlockPyIf {
                    test: core_name_expr("cond"),
                    body: BlockPyStmtFragment::from_stmts(vec![BlockPyStmt::Assign(
                        BlockPyAssign {
                            target: ast::ExprName {
                                id: "x".into(),
                                ctx: ast::ExprContext::Store,
                                range: TextRange::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            },
                            value: core_name_expr("b"),
                        },
                    )]),
                    orelse: BlockPyStmtFragment::from_stmts(vec![BlockPyStmt::Assign(
                        BlockPyAssign {
                            target: ast::ExprName {
                                id: "x".into(),
                                ctx: ast::ExprContext::Store,
                                range: TextRange::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            },
                            value: core_name_expr("c"),
                        },
                    )]),
                }),
                BlockPyStmt::Expr(core_call_expr("sink", vec![core_name_expr("x")])),
            ],
            term: BlockPyTerm::Return(None),
            params: Vec::new(),
            meta: Default::default(),
        };

        let blocks = lower_blockpy_blocks_to_bb_blocks(
            &[crate::block_py::CfgBlock {
                label: block.label,
                body: block.body,
                term: block.term,
                params: block.params,
                meta: None,
            }],
            &HashMap::new(),
            &HashMap::new(),
        );

        assert_eq!(blocks.len(), 4, "{blocks:?}");
        assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
    }

    fn core_name_expr(name: &str) -> CoreBlockPyExprWithoutAwaitOrYield {
        CoreBlockPyExprWithoutAwaitOrYield::Name(ast::ExprName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
        })
    }

    fn core_call_expr(
        name: &str,
        args: Vec<CoreBlockPyExprWithoutAwaitOrYield>,
    ) -> CoreBlockPyExprWithoutAwaitOrYield {
        CoreBlockPyExprWithoutAwaitOrYield::Call(CoreBlockPyCall {
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

    #[test]
    fn rewrites_current_exception_placeholders_in_final_core_blocks() {
        let block = BlockPyBlock {
            label: BlockPyLabel::from("start"),
            body: vec![BlockPyStmt::Expr(core_call_expr(
                "__dp_current_exception",
                Vec::new(),
            ))],
            term: BlockPyTerm::Return(Some(core_call_expr("__dp_exc_info", Vec::new()))),
            params: vec![crate::block_py::BlockParam {
                name: "_dp_try_exc_0".to_string(),
                role: crate::block_py::BlockParamRole::Exception,
            }],
            meta: (),
        };

        let lowered = lower_blockpy_blocks_to_bb_blocks(
            &[crate::block_py::CfgBlock {
                label: block.label,
                body: block.body,
                term: block.term,
                params: block.params,
                meta: None,
            }],
            &HashMap::new(),
            &HashMap::new(),
        );
        let block = &lowered[0];

        let BlockPyStmt::Expr(body_expr) = &block.body[0] else {
            panic!("expected expr stmt in lowered BB block");
        };
        assert!(matches!(
            body_expr,
            CoreBlockPyExprWithoutAwaitOrYield::Name(name) if name.id.as_str() == "_dp_try_exc_0"
        ));

        let BlockPyTerm::Return(Some(CoreBlockPyExprWithoutAwaitOrYield::Call(call))) = &block.term
        else {
            panic!("expected rewritten return expr");
        };
        assert!(matches!(
            call.func.as_ref(),
            CoreBlockPyExprWithoutAwaitOrYield::Name(name)
                if name.id.as_str() == "__dp_exc_info_from_exception"
        ));
        assert!(matches!(
            call.args.as_slice(),
            [CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwaitOrYield::Name(name))]
                if name.id.as_str() == "_dp_try_exc_0"
        ));
    }
}

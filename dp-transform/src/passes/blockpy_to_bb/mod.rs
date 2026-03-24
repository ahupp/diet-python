mod codegen_normalize;
mod exception_pass;

use super::blockpy_generators::lower_generator_like_function;
use super::core_eval_order::make_eval_order_explicit_in_core_callable_def_without_await;
use super::ruff_to_blockpy::{
    lowered_exception_edges, recompute_lowered_block_params, should_include_closure_storage_aliases,
};
use crate::block_py::cfg::linearize_structured_ifs;
use crate::block_py::{
    BbBlock, BbStmt, BlockArg, BlockParam, BlockParamRole, BlockPyEdge, BlockPyFunction,
    BlockPyFunctionKind, BlockPyIfTerm, BlockPyModule, BlockPyStmt, BlockPyTerm, CfgBlock,
    CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyKeywordArg,
    CoreBlockPyLiteral, IntrinsicCall,
};
use crate::passes::{BbBlockPyPass, CoreBlockPyPass, CoreBlockPyPassWithYield};
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;
use std::collections::HashMap;
use std::collections::HashSet;

pub use codegen_normalize::normalize_bb_module_for_codegen;
pub use exception_pass::lower_try_jump_exception_flow;

pub(crate) fn lower_yield_in_lowered_core_blockpy_module_bundle(
    module: BlockPyModule<CoreBlockPyPassWithYield>,
) -> BlockPyModule<CoreBlockPyPass> {
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
    module: BlockPyModule<CoreBlockPyPass>,
) -> BlockPyModule<BbBlockPyPass> {
    module.map_callable_defs(lower_core_blockpy_function_to_bb_function)
}

pub(crate) fn lower_core_blockpy_function_to_bb_function(
    lowered: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<BbBlockPyPass> {
    let BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        closure_layout,
        facts,
        semantic,
    } = lowered;
    let lowered_view: BlockPyFunction<CoreBlockPyPass> = BlockPyFunction {
        function_id,
        name_gen: crate::block_py::NameGen::new(function_id),
        names: names.clone(),
        kind,
        params: params.clone(),
        blocks: blocks.clone(),
        doc: doc.clone(),
        closure_layout: closure_layout.clone(),
        facts: facts.clone(),
        semantic: semantic.clone(),
    };
    let block_params = recompute_lowered_block_params(
        &lowered_view,
        should_include_closure_storage_aliases(&lowered_view),
    );
    BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks: lower_blockpy_blocks_to_bb_blocks(&blocks, &block_params),
        doc,
        closure_layout,
        facts,
        semantic,
    }
}

fn lower_blockpy_blocks_to_bb_blocks(
    blocks: &[crate::block_py::CfgBlock<
        BlockPyStmt<CoreBlockPyExpr>,
        BlockPyTerm<CoreBlockPyExpr>,
    >],
    block_params: &HashMap<String, Vec<String>>,
) -> Vec<BbBlock> {
    let exception_edges = lowered_exception_edges(blocks);
    let (linear_blocks, linear_block_params, linear_exception_edges) =
        linearize_structured_ifs(blocks, block_params, &exception_edges);
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
                exc_edge,
            }
        })
        .collect::<Vec<_>>();
    populate_exception_edge_args(&mut bb_blocks);
    bb_blocks
}

pub(super) fn populate_exception_edge_args(
    blocks: &mut [CfgBlock<BbStmt, BlockPyTerm<CoreBlockPyExpr>>],
) {
    let label_to_index = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label.as_str().to_string(), index))
        .collect::<HashMap<_, _>>();
    for block_index in 0..blocks.len() {
        let Some(exc_target_label) = blocks[block_index]
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
        let current_exception_aliases = match &blocks[target_index].term {
            BlockPyTerm::Jump(edge) => edge
                .args
                .iter()
                .filter_map(|arg| match arg {
                    BlockArg::Name(name) if name.starts_with("_dp_try_exc_") => Some(name.as_str()),
                    _ => None,
                })
                .collect::<HashSet<_>>(),
            _ => HashSet::new(),
        };
        let args = target_params
            .into_iter()
            .map(|target_param| {
                if exc_name.as_deref() == Some(target_param.as_str()) {
                    BlockArg::CurrentException
                } else if current_exception_aliases.contains(target_param.as_str()) {
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
        blocks[block_index].exc_edge = Some(BlockPyEdge::with_args(exc_target_label, args));
    }
}

fn rewrite_current_exception_in_blockpy_stmt(
    stmt: &mut BlockPyStmt<CoreBlockPyExpr>,
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
    term: &mut BlockPyTerm<CoreBlockPyExpr>,
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
        BlockPyTerm::Return(value) => rewrite_current_exception_in_blockpy_expr(value, exc_name),
        BlockPyTerm::Jump(_) => {}
    }
}

fn rewrite_current_exception_in_blockpy_expr(expr: &mut CoreBlockPyExpr, exc_name: &str) {
    match expr {
        CoreBlockPyExpr::Call(call) => {
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
        CoreBlockPyExpr::Intrinsic(IntrinsicCall { args, keywords, .. }) => {
            for arg in args {
                match arg {
                    CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                        rewrite_current_exception_in_blockpy_expr(value, exc_name);
                    }
                }
            }
            for keyword in keywords {
                match keyword {
                    CoreBlockPyKeywordArg::Named { value, .. }
                    | CoreBlockPyKeywordArg::Starred(value) => {
                        rewrite_current_exception_in_blockpy_expr(value, exc_name);
                    }
                }
            }
        }
        CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => {}
    }

    if is_current_exception_call(expr) {
        *expr = current_exception_name_expr(exc_name);
    } else if is_exc_info_call(expr) {
        *expr = current_exception_info_expr(exc_name);
    }
}

fn is_current_exception_call(expr: &CoreBlockPyExpr) -> bool {
    let CoreBlockPyExpr::Call(call) = expr else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "current_exception")
}

fn is_exc_info_call(expr: &CoreBlockPyExpr) -> bool {
    let CoreBlockPyExpr::Call(call) = expr else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "exc_info")
}

fn is_dp_lookup_call_expr(func: &CoreBlockPyExpr, attr_name: &str) -> bool {
    match func {
        CoreBlockPyExpr::Name(name) => name.id.as_str() == format!("__dp_{attr_name}"),
        CoreBlockPyExpr::Call(call) if call.keywords.is_empty() && call.args.len() == 2 => {
            matches!(
                call.func.as_ref(),
                CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_getattr"
            ) && is_dp_getattr_lookup_args(&call.args, attr_name)
        }
        CoreBlockPyExpr::Intrinsic(IntrinsicCall {
            intrinsic,
            args,
            keywords,
            ..
        }) if keywords.is_empty() && args.len() == 2 && intrinsic.name() == "__dp_getattr" => {
            is_dp_getattr_lookup_args(args, attr_name)
        }
        _ => false,
    }
}

fn is_dp_getattr_lookup_args(
    args: &[CoreBlockPyCallArg<CoreBlockPyExpr>],
    attr_name: &str,
) -> bool {
    matches!(
        &args[0],
        CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(base))
            if base.id.as_str() == "__dp__"
    ) && expr_static_str(match &args[1] {
        CoreBlockPyCallArg::Positional(value) => value,
        CoreBlockPyCallArg::Starred(_) => return false,
    }) == Some(attr_name.to_string())
}

fn expr_static_str(expr: &CoreBlockPyExpr) -> Option<String> {
    match expr {
        CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(value)) => {
            Some(value.value.clone())
        }
        CoreBlockPyExpr::Literal(CoreBlockPyLiteral::BytesLiteral(bytes)) => {
            String::from_utf8(bytes.value.clone()).ok()
        }
        CoreBlockPyExpr::Call(call)
            if call.keywords.is_empty()
                && call.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    CoreBlockPyExpr::Name(name)
                        if matches!(
                            name.id.as_str(),
                            "__dp_decode_literal_bytes" | "__dp_decode_literal_source_bytes"
                        )
                ) =>
        {
            match &call.args[0] {
                CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Literal(
                    CoreBlockPyLiteral::BytesLiteral(bytes),
                )) => String::from_utf8(bytes.value.clone()).ok(),
                _ => None,
            }
        }
        _ => None,
    }
}

fn current_exception_name_expr(exc_name: &str) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Name(ast::ExprName {
        id: exc_name.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    })
}

fn current_exception_info_expr(exc_name: &str) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Call(CoreBlockPyCall {
        node_index: compat_node_index(),
        range: compat_range(),
        func: Box::new(CoreBlockPyExpr::Name(ast::ExprName {
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

fn bb_stmt_from_blockpy_stmt(stmt: BlockPyStmt<CoreBlockPyExpr>) -> BbStmt {
    stmt.into()
}

#[cfg(test)]
mod tests {
    use super::populate_exception_edge_args;
    use crate::block_py::{
        BlockPyAssign, BlockPyBlock, BlockPyIf, BlockPyLabel, BlockPyStmt, BlockPyStmtFragment,
        BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyLiteral,
        CoreStringLiteral, IntrinsicCall,
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
            term: BlockPyTerm::Return(core_name_expr("__dp_NONE")),
            params: Vec::new(),
            exc_edge: None,
        };

        let blocks = lower_blockpy_blocks_to_bb_blocks(
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

    fn core_name_expr(name: &str) -> CoreBlockPyExpr {
        CoreBlockPyExpr::Name(ast::ExprName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
        })
    }

    fn core_call_expr(name: &str, args: Vec<CoreBlockPyExpr>) -> CoreBlockPyExpr {
        CoreBlockPyExpr::Call(CoreBlockPyCall {
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

    fn core_string_expr(value: &str) -> CoreBlockPyExpr {
        CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(CoreStringLiteral {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            value: value.to_string(),
        }))
    }

    #[test]
    fn rewrites_current_exception_placeholders_in_final_core_blocks() {
        let block = BlockPyBlock {
            label: BlockPyLabel::from("start"),
            body: vec![BlockPyStmt::Expr(core_call_expr(
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

        let lowered = lower_blockpy_blocks_to_bb_blocks(
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

        let crate::block_py::BbStmt::Expr(body_expr) = &block.body[0] else {
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
        let block = BlockPyBlock {
            label: BlockPyLabel::from("start"),
            body: Vec::new(),
            term: BlockPyTerm::Return(CoreBlockPyExpr::Intrinsic(IntrinsicCall {
                intrinsic: &crate::block_py::intrinsics::GETATTR_INTRINSIC,
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                args: vec![
                    CoreBlockPyCallArg::Positional(core_call_expr(
                        "__dp_current_exception",
                        Vec::new(),
                    )),
                    CoreBlockPyCallArg::Positional(core_string_expr("value")),
                ],
                keywords: Vec::new(),
            })),
            params: vec![crate::block_py::BlockParam {
                name: "_dp_try_exc_0".to_string(),
                role: crate::block_py::BlockParamRole::Exception,
            }],
            exc_edge: None,
        };

        let lowered = lower_blockpy_blocks_to_bb_blocks(
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

        let BlockPyTerm::Return(CoreBlockPyExpr::Intrinsic(call)) = &block.term else {
            panic!("expected intrinsic return expr");
        };
        assert_eq!(call.intrinsic.name(), "__dp_getattr");
        assert!(matches!(
            call.args.as_slice(),
            [
                CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(name)),
                CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Literal(
                    CoreBlockPyLiteral::StringLiteral(value)
                ))
            ] if name.id.as_str() == "_dp_try_exc_0" && value.value == "value"
        ));
    }

    #[test]
    fn exception_edges_seed_hidden_try_exception_locals_from_current_exception() {
        let mut blocks = vec![
            crate::block_py::CfgBlock {
                label: BlockPyLabel::from("source"),
                body: Vec::new(),
                term: BlockPyTerm::<CoreBlockPyExpr>::Return(
                    <CoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
                ),
                params: vec![crate::block_py::BlockParam {
                    name: "_dp_outer_exc".to_string(),
                    role: crate::block_py::BlockParamRole::Exception,
                }],
                exc_edge: Some(crate::block_py::BlockPyEdge::new(BlockPyLabel::from(
                    "target",
                ))),
            },
            crate::block_py::CfgBlock {
                label: BlockPyLabel::from("target"),
                body: Vec::new(),
                term: BlockPyTerm::<CoreBlockPyExpr>::Jump(
                    crate::block_py::BlockPyEdge::with_args(
                        BlockPyLabel::from("after"),
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
                label: BlockPyLabel::from("after"),
                body: Vec::new(),
                term: BlockPyTerm::<CoreBlockPyExpr>::Return(
                    <CoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(),
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
}

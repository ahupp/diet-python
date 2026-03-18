mod codegen_normalize;
mod codegen_trace;
mod exception_pass;

use super::annotation_export::build_exec_function_def_binding_stmts;
use super::bb_ir::{BbBlock, BbBlockMeta, BbFunction, BbModule, BbStmt, BbTerm};
use super::block_py::cfg::linearize_structured_ifs;
use super::block_py::{
    BlockPyBlock, BlockPyCallableDef, BlockPyIfTerm, BlockPyModule, BlockPyStmt, BlockPyTerm,
    CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyExprWithoutAwait,
    CoreBlockPyExprWithoutAwaitOrYield, CoreBlockPyKeywordArg, CoreBlockPyLiteral,
};
use super::blockpy_expr_simplify::simplify_blockpy_callable_def_exprs;
use super::cfg_ir::{CfgCallableDef, CfgModule};
use super::core_await_lower::lower_awaits_in_core_blockpy_callable_def;
use super::function_lowering::rewrite_deleted_name_loads;
use super::lowered_ir::{LoweredCfgMetadata, LoweredFunction};
use super::ruff_to_blockpy::{build_lowered_blockpy_function_bundle, LoweredBlockPyFunction};
use crate::basic_block::ast_to_ast::context::Context;
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::TextRange;
use std::collections::{HashMap, HashSet};

pub use codegen_normalize::normalize_bb_module_for_codegen;
pub use exception_pass::lower_try_jump_exception_flow;

pub type LoweredCoreBlockPyFunction = LoweredBlockPyFunction<BlockPyCallableDef<CoreBlockPyExpr>>;
pub type LoweredCoreBlockPyFunctionWithoutAwait =
    LoweredBlockPyFunction<BlockPyCallableDef<CoreBlockPyExprWithoutAwait>>;
pub type LoweredCoreBlockPyFunctionWithoutAwaitOrYield =
    LoweredBlockPyFunction<BlockPyCallableDef<CoreBlockPyExprWithoutAwaitOrYield>>;

#[derive(Clone)]
pub(crate) struct LoweredBlockPyModuleBundlePlan {
    pub callable_defs: Vec<BlockPyCallableDef<Expr>>,
    pub next_block_id: usize,
    pub next_function_id: usize,
}

fn next_temp_from_reserved_names(
    reserved_names: &mut HashSet<String>,
    prefix: &str,
    next_id: &mut usize,
) -> String {
    loop {
        let current = *next_id;
        *next_id += 1;
        let candidate = format!("_dp_{prefix}_{current}");
        if reserved_names.contains(candidate.as_str()) {
            continue;
        }
        reserved_names.insert(candidate.clone());
        return candidate;
    }
}

pub(crate) fn lower_generators_in_lowered_blockpy_module_bundle_plan(
    context: &Context,
    plan: LoweredBlockPyModuleBundlePlan,
) -> CfgModule<LoweredBlockPyFunction> {
    let LoweredBlockPyModuleBundlePlan {
        callable_defs: blockpy_callable_defs,
        mut next_block_id,
        mut next_function_id,
    } = plan;
    let mut callable_defs = Vec::new();
    for callable_def in blockpy_callable_defs {
        let callable_facts = callable_def.facts.clone();
        let mut reserved_temp_names = callable_facts.outer_scope_names.clone();
        let lowered_function = build_lowered_blockpy_function_bundle(
            context,
            callable_def,
            &mut next_block_id,
            &mut next_function_id,
            &mut |func_def| {
                build_exec_function_def_binding_stmts(
                    func_def,
                    &callable_facts.cell_slots,
                    &callable_facts.outer_scope_names,
                )
            },
            &mut |prefix, next_block_id| {
                next_temp_from_reserved_names(&mut reserved_temp_names, prefix, next_block_id)
            },
            &mut |callable_def| {
                if !callable_facts.deleted_names.is_empty() {
                    rewrite_deleted_name_loads(
                        &mut callable_def.blocks,
                        &callable_facts.deleted_names,
                        &callable_facts.unbound_local_names,
                    );
                } else if !callable_facts.unbound_local_names.is_empty() {
                    rewrite_deleted_name_loads(
                        &mut callable_def.blocks,
                        &HashSet::new(),
                        &callable_facts.unbound_local_names,
                    );
                }
            },
        );
        callable_defs.push(lowered_function);
    }
    CfgModule { callable_defs }
}

pub(crate) fn lowered_blockpy_module_bundle_plan_to_semantic_blockpy_module(
    plan: &LoweredBlockPyModuleBundlePlan,
) -> BlockPyModule<Expr> {
    BlockPyModule {
        callable_defs: plan.callable_defs.clone(),
    }
}

pub(crate) fn lower_blockpy_module_plan_to_bundle(
    context: &Context,
    plan: LoweredBlockPyModuleBundlePlan,
) -> CfgModule<LoweredBlockPyFunction> {
    lower_generators_in_lowered_blockpy_module_bundle_plan(context, plan)
}

pub fn project_lowered_module_callable_defs<T, U: Clone>(
    module: &CfgModule<T>,
    project: impl Fn(&T) -> &U,
) -> CfgModule<U> {
    module.map_callable_defs(|lowered_function| project(lowered_function).clone())
}

pub(crate) fn simplify_lowered_blockpy_module_bundle_exprs(
    module: &CfgModule<LoweredBlockPyFunction>,
) -> CfgModule<LoweredCoreBlockPyFunction> {
    module.map_callable_defs(simplify_lowered_blockpy_function_exprs)
}

fn lower_core_blockpy_function_without_await(
    lowered: &LoweredCoreBlockPyFunction,
) -> LoweredCoreBlockPyFunctionWithoutAwait {
    lowered.map_callable_def(|callable_def| {
        lower_awaits_in_core_blockpy_callable_def(callable_def.clone())
    })
}

fn lower_core_callable_def_without_await_or_yield(
    callable_def: &BlockPyCallableDef<CoreBlockPyExprWithoutAwait>,
) -> BlockPyCallableDef<CoreBlockPyExprWithoutAwaitOrYield> {
    let qualname = callable_def.qualname.as_str();
    callable_def.clone().try_into().unwrap_or_else(|_| {
        panic!(
            "core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for {}",
            qualname
        )
    })
}

fn lower_core_blockpy_function_without_await_or_yield(
    lowered: &LoweredCoreBlockPyFunctionWithoutAwait,
) -> LoweredCoreBlockPyFunctionWithoutAwaitOrYield {
    lowered.map_callable_def(lower_core_callable_def_without_await_or_yield)
}

pub(crate) fn lower_awaits_in_lowered_core_blockpy_module_bundle(
    module: CfgModule<LoweredCoreBlockPyFunction>,
) -> CfgModule<LoweredCoreBlockPyFunctionWithoutAwait> {
    module.map_callable_defs(lower_core_blockpy_function_without_await)
}

pub(crate) fn lower_yield_in_lowered_core_blockpy_module_bundle(
    module: CfgModule<LoweredCoreBlockPyFunctionWithoutAwait>,
) -> CfgModule<LoweredCoreBlockPyFunctionWithoutAwaitOrYield> {
    module.map_callable_defs(lower_core_blockpy_function_without_await_or_yield)
}

pub(crate) fn lower_core_blockpy_module_bundle_to_bb_module(
    module: &CfgModule<LoweredCoreBlockPyFunctionWithoutAwaitOrYield>,
) -> BbModule {
    module.map_callable_defs(lower_core_blockpy_function_to_bb_function)
}

fn simplify_lowered_blockpy_function_exprs(
    lowered: &LoweredBlockPyFunction,
) -> LoweredCoreBlockPyFunction {
    lowered.map_callable_def(simplify_blockpy_callable_def_exprs)
}

pub(crate) fn lower_core_blockpy_function_to_bb_function(
    lowered: &LoweredCoreBlockPyFunctionWithoutAwaitOrYield,
) -> BbFunction {
    LoweredFunction {
        callable_def: CfgCallableDef {
            function_id: lowered.callable_def.function_id,
            bind_name: lowered.callable_def.bind_name.clone(),
            display_name: lowered.callable_def.display_name.clone(),
            qualname: lowered.callable_def.qualname.clone(),
            kind: lowered.lowered_kind().clone(),
            params: lowered.callable_def.params.clone(),
            param_defaults: lowered.callable_def.param_defaults.clone(),
            entry_liveins: lowered.callable_def.entry_liveins.clone(),
            blocks: lower_blockpy_blocks_to_bb_blocks(
                &lowered.callable_def.blocks,
                lowered.block_params(),
                lowered.exception_edges(),
            ),
        },
        extra: LoweredCfgMetadata {
            closure_layout: lowered.runtime_closure_layout().clone(),
            local_cell_slots: lowered.callable_def.local_cell_slots.clone(),
        },
    }
}

pub(crate) fn lower_blockpy_blocks_to_bb_blocks(
    blocks: &[BlockPyBlock<CoreBlockPyExprWithoutAwaitOrYield>],
    block_params: &HashMap<String, Vec<String>>,
    exception_edges: &HashMap<String, Option<String>>,
) -> Vec<BbBlock> {
    let (linear_blocks, linear_block_params, linear_exception_edges) =
        linearize_structured_ifs(blocks, block_params, exception_edges);
    let block_exc_params = linear_blocks
        .iter()
        .map(|block| {
            (
                block.label.as_str().to_string(),
                block.meta.exc_param.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    linear_blocks
        .iter()
        .map(|block| {
            let current_exception_name = block.meta.exc_param.as_deref();
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
            let exc_target_label = linear_exception_edges
                .get(block.label.as_str())
                .cloned()
                .flatten()
                .map(crate::basic_block::block_py::BlockPyLabel::from);
            let exc_name = exc_target_label.as_ref().and_then(|target_label| {
                block_exc_params
                    .get(target_label.as_str())
                    .cloned()
                    .flatten()
                    .or_else(|| {
                        linear_block_params
                            .get(target_label.as_str())
                            .and_then(|params| exception_param_from_block_params(params))
                    })
            });
            let ops = normalized_body
                .into_iter()
                .map(bb_stmt_from_blockpy_stmt)
                .collect::<Vec<_>>();
            let mut params = linear_block_params
                .get(block.label.as_str())
                .cloned()
                .unwrap_or_default();
            if let Some(exc_param) = block.meta.exc_param.as_ref() {
                if !params.iter().any(|param| param == exc_param) {
                    params.push(exc_param.clone());
                }
            }
            BbBlock {
                label: block.label.clone(),
                body: ops,
                term: bb_term_from_blockpy_term(&normalized_term),
                meta: BbBlockMeta {
                    params,
                    exc_target_label,
                    exc_name,
                },
            }
        })
        .collect()
}

fn exception_param_from_block_params(params: &[String]) -> Option<String> {
    params.iter().find_map(|name| {
        (name.starts_with("_dp_try_exc_") || name.starts_with("_dp_uncaught_exc_"))
            .then(|| name.clone())
    })
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
            Some(value.value.to_str().to_string())
        }
        CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::BytesLiteral(bytes)) => {
            let value: std::borrow::Cow<[u8]> = (&bytes.value).into();
            String::from_utf8(value.into_owned()).ok()
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
                )) => {
                    let value: std::borrow::Cow<[u8]> = (&bytes.value).into();
                    String::from_utf8(value.into_owned()).ok()
                }
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

fn bb_term_from_blockpy_term(terminal: &BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>) -> BbTerm {
    match terminal {
        BlockPyTerm::Jump(target) => BbTerm::Jump(target.clone()),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BbTerm::BrIf {
            test: test.clone(),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        },
        BlockPyTerm::BranchTable(branch) => BbTerm::BrTable {
            index: branch.index.clone(),
            targets: branch.targets.clone(),
            default_label: branch.default_label.clone(),
        },
        BlockPyTerm::Raise(raise_stmt) => BbTerm::Raise {
            exc: raise_stmt.exc.clone(),
            cause: None,
        },
        BlockPyTerm::TryJump(try_jump) => BbTerm::Jump(try_jump.body_label.clone()),
        BlockPyTerm::Return(value) => BbTerm::Ret(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    use crate::basic_block::bb_ir::BbTerm;
    use crate::basic_block::block_py::{
        BlockPyAssign, BlockPyBlock, BlockPyIf, BlockPyLabel, BlockPyStmt, BlockPyStmtFragment,
        BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExprWithoutAwaitOrYield,
    };
    use crate::basic_block::blockpy_to_bb::lower_blockpy_blocks_to_bb_blocks;
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
            meta: Default::default(),
        };

        let blocks = lower_blockpy_blocks_to_bb_blocks(&[block], &HashMap::new(), &HashMap::new());

        assert_eq!(blocks.len(), 4, "{blocks:?}");
        assert!(matches!(blocks[0].term, BbTerm::BrIf { .. }));
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
            meta: crate::basic_block::block_py::BlockPyBlockMeta {
                exc_param: Some("_dp_try_exc_0".to_string()),
            },
        };

        let lowered = lower_blockpy_blocks_to_bb_blocks(&[block], &HashMap::new(), &HashMap::new());
        let block = &lowered[0];

        let BlockPyStmt::Expr(body_expr) = &block.body[0] else {
            panic!("expected expr stmt in lowered BB block");
        };
        assert!(matches!(
            body_expr,
            CoreBlockPyExprWithoutAwaitOrYield::Name(name) if name.id.as_str() == "_dp_try_exc_0"
        ));

        let BbTerm::Ret(Some(CoreBlockPyExprWithoutAwaitOrYield::Call(call))) = &block.term else {
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

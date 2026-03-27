use crate::block_py::intrinsics::{
    Intrinsic, CELL_REF_INTRINSIC, DEL_DEREF_INTRINSIC, DEL_DEREF_QUIETLY_INTRINSIC,
    DEL_QUIETLY_INTRINSIC, LOAD_CELL_INTRINSIC, LOAD_GLOBAL_INTRINSIC, MAKE_CELL_INTRINSIC,
    STORE_CELL_INTRINSIC, STORE_GLOBAL_INTRINSIC,
};
use crate::block_py::{
    core_positional_call_expr_with_meta, core_positional_intrinsic_expr_with_meta,
    map_intrinsic_args_with, BbStmt, BindingTarget, BlockPyAssign, BlockPyBindingKind,
    BlockPyBindingPurpose, BlockPyCallableScopeKind, BlockPyCallableSemanticInfo,
    BlockPyCellBindingKind, BlockPyClassBodyFallback, BlockPyEffectiveBinding, BlockPyFunction,
    BlockPyFunctionKind, BlockPyModule, BlockPyModuleMap, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    ClosureInit, ClosureSlot, CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExpr,
    CoreBlockPyLiteral, CoreNumberLiteral, CoreNumberLiteralValue, CoreStringLiteral,
    IntrinsicCall, LocatedName, NameLocation,
};
use crate::passes::ruff_to_blockpy::{
    populate_exception_edge_args, recompute_lowered_block_params,
    rewrite_current_exception_in_core_blocks, should_include_closure_storage_aliases,
};
use crate::passes::{BbBlockPyPass, CoreBlockPyPass};
use ruff_python_ast::{self as ast, ExprName};
use std::collections::{HashMap, HashSet};

fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name.starts_with("__dp_") || name == "__dp__"
}

fn should_late_bind_name(name: &str, semantic: &BlockPyCallableSemanticInfo) -> bool {
    !is_internal_symbol(name) || semantic.honors_internal_binding(name)
}

fn core_string_expr(
    value: String,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(CoreStringLiteral {
        node_index,
        range,
        value,
    }))
}

fn globals_expr(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    core_positional_call_expr_with_meta("__dp_globals", node_index, range, Vec::new())
}

fn rewrite_global_name_load(name: ExprName) -> CoreBlockPyExpr {
    let node_index = name.node_index.clone();
    let range = name.range;
    let bind_name = name.id.to_string();
    core_positional_intrinsic_expr_with_meta(
        &LOAD_GLOBAL_INTRINSIC,
        node_index.clone(),
        range,
        vec![
            globals_expr(node_index.clone(), range),
            core_string_expr(bind_name, node_index, range),
        ],
    )
}

fn cell_expr_for_name(
    name: &str,
    semantic: &BlockPyCallableSemanticInfo,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    core_name_expr(
        semantic.cell_storage_name(name).as_str(),
        ast::ExprContext::Load,
        node_index,
        range,
    )
}

fn rewrite_cell_name_load(
    name: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
) -> CoreBlockPyExpr {
    let node_index = name.node_index.clone();
    let range = name.range;
    core_positional_intrinsic_expr_with_meta(
        &LOAD_CELL_INTRINSIC,
        node_index.clone(),
        range,
        vec![cell_expr_for_name(
            name.id.as_str(),
            semantic,
            node_index,
            range,
        )],
    )
}

fn rewrite_cell_ref_expr(
    logical_name: &str,
    semantic: &BlockPyCallableSemanticInfo,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    core_positional_intrinsic_expr_with_meta(
        &CELL_REF_INTRINSIC,
        node_index.clone(),
        range,
        vec![core_name_expr(
            semantic.cell_ref_source_name(logical_name).as_str(),
            ast::ExprContext::Load,
            node_index,
            range,
        )],
    )
}

fn rewrite_global_binding_assign(
    assign: BlockPyAssign<CoreBlockPyExpr>,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = assign.target.node_index.clone();
    let range = assign.target.range;
    let bind_name = assign.target.id.to_string();
    BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
        &STORE_GLOBAL_INTRINSIC,
        node_index.clone(),
        range,
        vec![
            globals_expr(node_index.clone(), range),
            core_string_expr(bind_name, node_index, range),
            assign.value,
        ],
    ))
}

fn rewrite_class_namespace_binding_assign(
    assign: BlockPyAssign<CoreBlockPyExpr>,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = assign.target.node_index.clone();
    let range = assign.target.range;
    let bind_name = assign.target.id.to_string();
    BlockPyStmt::Expr(core_positional_call_expr_with_meta(
        "__dp_setitem",
        node_index.clone(),
        range,
        vec![
            class_namespace_expr(node_index.clone(), range),
            core_string_expr(bind_name, node_index, range),
            assign.value,
        ],
    ))
}

fn rewrite_cell_binding_assign(
    assign: BlockPyAssign<CoreBlockPyExpr>,
    semantic: &BlockPyCallableSemanticInfo,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = assign.target.node_index.clone();
    let range = assign.target.range;
    BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
        &STORE_CELL_INTRINSIC,
        node_index.clone(),
        range,
        vec![
            cell_expr_for_name(assign.target.id.as_str(), semantic, node_index, range),
            assign.value,
        ],
    ))
}

fn rewrite_global_binding_delete_by_name(
    bind_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> BlockPyStmt<CoreBlockPyExpr> {
    BlockPyStmt::Expr(core_positional_call_expr_with_meta(
        "__dp_delitem",
        node_index.clone(),
        range,
        vec![
            globals_expr(node_index.clone(), range),
            core_string_expr(bind_name.to_string(), node_index, range),
        ],
    ))
}

fn rewrite_binding_delete(
    target: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = target.node_index.clone();
    let range = target.range;
    let bind_name = target.id.to_string();
    if semantic.is_cell_binding(bind_name.as_str()) {
        return BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
            &DEL_DEREF_INTRINSIC,
            node_index.clone(),
            range,
            vec![cell_expr_for_name(
                bind_name.as_str(),
                semantic,
                node_index,
                range,
            )],
        ));
    }
    match semantic.binding_target_for_name(bind_name.as_str(), BlockPyBindingPurpose::Store) {
        BindingTarget::Local => BlockPyStmt::Assign(BlockPyAssign {
            target: ast::ExprName {
                id: target.id,
                ctx: ast::ExprContext::Store,
                node_index: node_index.clone(),
                range,
            },
            value: deleted_sentinel_expr(node_index, range),
        }),
        BindingTarget::ModuleGlobal => {
            rewrite_global_binding_delete_by_name(bind_name.as_str(), node_index, range)
        }
        BindingTarget::ClassNamespace => BlockPyStmt::Expr(core_positional_call_expr_with_meta(
            "__dp_delitem",
            node_index.clone(),
            range,
            vec![
                class_namespace_expr(node_index.clone(), range),
                core_string_expr(bind_name, node_index, range),
            ],
        )),
    }
}

fn rewrite_deleted_name_load_expr(
    name: ExprName,
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) -> CoreBlockPyExpr {
    let always_unbound = always_unbound_names.contains(name.id.as_str());
    let deleted = deleted_names.contains(name.id.as_str());
    if !always_unbound && !deleted {
        return CoreBlockPyExpr::Name(name);
    }
    let node_index = name.node_index.clone();
    let range = name.range;
    core_positional_call_expr_with_meta(
        "__dp_load_deleted_name",
        node_index.clone(),
        range,
        vec![
            core_string_expr(name.id.to_string(), node_index.clone(), range),
            if always_unbound {
                deleted_sentinel_expr(node_index, range)
            } else {
                CoreBlockPyExpr::Name(name)
            },
        ],
    )
}

fn expr_meta(expr: &CoreBlockPyExpr) -> (ast::AtomicNodeIndex, ruff_text_size::TextRange) {
    match expr {
        CoreBlockPyExpr::Name(name) => (name.node_index.clone(), name.range),
        CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(literal)) => {
            (literal.node_index.clone(), literal.range)
        }
        CoreBlockPyExpr::Literal(CoreBlockPyLiteral::BytesLiteral(literal)) => {
            (literal.node_index.clone(), literal.range)
        }
        CoreBlockPyExpr::Literal(CoreBlockPyLiteral::NumberLiteral(literal)) => {
            (literal.node_index.clone(), literal.range)
        }
        CoreBlockPyExpr::Call(call) => (call.node_index.clone(), call.range),
        CoreBlockPyExpr::Intrinsic(call) => (call.node_index.clone(), call.range),
    }
}

fn rewrite_deleted_name_loads_in_expr(
    expr: &mut CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    if let Some(logical_name) = cell_load_logical_name(expr, semantic) {
        if deleted_names.contains(logical_name.as_str())
            || always_unbound_names.contains(logical_name.as_str())
        {
            let (node_index, range) = expr_meta(expr);
            *expr = core_positional_call_expr_with_meta(
                "__dp_load_deleted_name",
                node_index.clone(),
                range,
                vec![
                    core_string_expr(logical_name, node_index.clone(), range),
                    expr.clone(),
                ],
            );
            return;
        }
    }
    match expr {
        CoreBlockPyExpr::Name(name) if matches!(name.ctx, ast::ExprContext::Load) => {
            *expr =
                rewrite_deleted_name_load_expr(name.clone(), deleted_names, always_unbound_names);
        }
        CoreBlockPyExpr::Call(CoreBlockPyCall {
            func,
            args,
            keywords,
            ..
        }) => {
            rewrite_deleted_name_loads_in_expr(
                func.as_mut(),
                semantic,
                deleted_names,
                always_unbound_names,
            );
            for arg in args {
                rewrite_deleted_name_loads_in_expr(
                    arg.expr_mut(),
                    semantic,
                    deleted_names,
                    always_unbound_names,
                );
            }
            for keyword in keywords {
                rewrite_deleted_name_loads_in_expr(
                    keyword.expr_mut(),
                    semantic,
                    deleted_names,
                    always_unbound_names,
                );
            }
        }
        CoreBlockPyExpr::Intrinsic(IntrinsicCall {
            intrinsic, args, ..
        }) => match intrinsic.name() {
            name if name == LOAD_CELL_INTRINSIC.name()
                || name == DEL_DEREF_INTRINSIC.name()
                || name == DEL_DEREF_QUIETLY_INTRINSIC.name()
                || name == CELL_REF_INTRINSIC.name() => {}
            name if name == STORE_CELL_INTRINSIC.name() => {
                if let Some(value_expr) = args.get_mut(1) {
                    rewrite_deleted_name_loads_in_expr(
                        value_expr,
                        semantic,
                        deleted_names,
                        always_unbound_names,
                    );
                }
            }
            _ => {
                for arg in args {
                    rewrite_deleted_name_loads_in_expr(
                        arg,
                        semantic,
                        deleted_names,
                        always_unbound_names,
                    );
                }
            }
        },
        CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => {}
    }
}

fn core_name_expr(
    id: &str,
    ctx: ast::ExprContext,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Name(
        ast::ExprName {
            id: id.into(),
            ctx,
            node_index,
            range,
        }
        .into(),
    )
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> ruff_text_size::TextRange {
    ruff_text_size::TextRange::default()
}

fn class_namespace_expr(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    core_name_expr("_dp_class_ns", ast::ExprContext::Load, node_index, range)
}

fn deleted_sentinel_expr(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    core_name_expr("__dp_DELETED", ast::ExprContext::Load, node_index, range)
}

fn rewrite_class_name_load_global(name: ExprName) -> CoreBlockPyExpr {
    let node_index = name.node_index.clone();
    let range = name.range;
    let bind_name = name.id.to_string();
    core_positional_call_expr_with_meta(
        "__dp_class_lookup_global",
        node_index.clone(),
        range,
        vec![
            class_namespace_expr(node_index.clone(), range),
            core_string_expr(bind_name, node_index.clone(), range),
            globals_expr(node_index, range),
        ],
    )
}

fn rewrite_class_name_load_cell(
    name: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
) -> CoreBlockPyExpr {
    let node_index = name.node_index.clone();
    let range = name.range;
    let bind_name = name.id.to_string();
    core_positional_call_expr_with_meta(
        "__dp_class_lookup_cell",
        node_index.clone(),
        range,
        vec![
            class_namespace_expr(node_index.clone(), range),
            core_string_expr(bind_name, node_index.clone(), range),
            cell_expr_for_name(name.id.as_str(), semantic, node_index, range),
        ],
    )
}

fn rewrite_quiet_delete_marker(
    name: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = name.node_index.clone();
    let range = name.range;
    match semantic.binding_kind(name.id.as_str()) {
        Some(BlockPyBindingKind::Cell(_)) => {
            BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
                &DEL_DEREF_QUIETLY_INTRINSIC,
                node_index.clone(),
                range,
                vec![cell_expr_for_name(
                    name.id.as_str(),
                    semantic,
                    node_index,
                    range,
                )],
            ))
        }
        _ => match semantic.binding_target_for_name(name.id.as_str(), BlockPyBindingPurpose::Store)
        {
            BindingTarget::Local => BlockPyStmt::Assign(BlockPyAssign {
                target: ast::ExprName {
                    id: name.id,
                    ctx: ast::ExprContext::Store,
                    node_index: node_index.clone(),
                    range,
                },
                value: deleted_sentinel_expr(node_index, range),
            }),
            BindingTarget::ModuleGlobal => {
                BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
                    &DEL_QUIETLY_INTRINSIC,
                    node_index.clone(),
                    range,
                    vec![
                        globals_expr(node_index.clone(), range),
                        core_string_expr(name.id.to_string(), node_index, range),
                    ],
                ))
            }
            BindingTarget::ClassNamespace => {
                BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
                    &DEL_QUIETLY_INTRINSIC,
                    node_index.clone(),
                    range,
                    vec![
                        class_namespace_expr(node_index.clone(), range),
                        core_string_expr(name.id.to_string(), node_index, range),
                    ],
                ))
            }
        },
    }
}

fn quiet_delete_marker_target(expr: &CoreBlockPyExpr) -> Option<ExprName> {
    let CoreBlockPyExpr::Call(CoreBlockPyCall {
        func,
        args,
        keywords,
        ..
    }) = expr
    else {
        return None;
    };
    if !keywords.is_empty() || args.len() != 1 {
        return None;
    }
    let CoreBlockPyExpr::Name(func_name) = func.as_ref() else {
        return None;
    };
    if func_name.id.as_str() != "_dp_del_quietly" {
        return None;
    }
    match &args[0] {
        CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(name)) => Some(name.clone()),
        CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Call(CoreBlockPyCall {
            func,
            args,
            keywords,
            ..
        })) if keywords.is_empty()
            && args.len() == 2
            && matches!(
                func.as_ref(),
                CoreBlockPyExpr::Name(func_name)
                    if func_name.id.as_str() == "__dp_load_deleted_name"
            ) =>
        {
            match &args[1] {
                CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(name)) => Some(name.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_deleted_sentinel_expr(expr: &CoreBlockPyExpr) -> bool {
    matches!(expr, CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_DELETED")
}

fn cell_ref_marker_target(expr: &CoreBlockPyExpr) -> Option<String> {
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic, args, ..
    }) = expr
    else {
        return None;
    };
    if intrinsic.name() != CELL_REF_INTRINSIC.name() || args.len() != 1 {
        return None;
    }
    let CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(literal)) = &args[0] else {
        return None;
    };
    Some(literal.value.clone())
}

fn cell_load_logical_name(
    expr: &CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
) -> Option<String> {
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic, args, ..
    }) = expr
    else {
        return None;
    };
    if intrinsic.name() != LOAD_CELL_INTRINSIC.name() || args.len() != 1 {
        return None;
    }
    let CoreBlockPyExpr::Name(name) = &args[0] else {
        return None;
    };
    semantic.logical_name_for_cell_storage(name.id.as_str())
}

fn build_local_cell_init_assign(
    storage_name: &str,
    logical_name: &str,
    is_parameter: bool,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = compat_node_index();
    let range = compat_range();
    let init_expr = if is_parameter {
        core_name_expr(
            logical_name,
            ast::ExprContext::Load,
            node_index.clone(),
            range,
        )
    } else {
        deleted_sentinel_expr(node_index.clone(), range)
    };
    BlockPyStmt::Assign(BlockPyAssign {
        target: ast::ExprName {
            id: storage_name.into(),
            ctx: ast::ExprContext::Store,
            node_index: node_index.clone(),
            range,
        },
        value: core_positional_intrinsic_expr_with_meta(
            &MAKE_CELL_INTRINSIC,
            node_index,
            range,
            vec![init_expr],
        ),
    })
}

fn closure_slot_init_expr(slot: &ClosureSlot) -> CoreBlockPyExpr {
    let node_index = compat_node_index();
    let range = compat_range();
    match slot.init {
        ClosureInit::InheritedCapture => {
            panic!("inherited captures do not allocate new cells in outer callables")
        }
        ClosureInit::Parameter => core_name_expr(
            slot.logical_name.as_str(),
            ast::ExprContext::Load,
            node_index,
            range,
        ),
        ClosureInit::DeletedSentinel => deleted_sentinel_expr(node_index, range),
        ClosureInit::RuntimePcUnstarted => {
            CoreBlockPyExpr::Literal(CoreBlockPyLiteral::NumberLiteral(CoreNumberLiteral {
                node_index,
                range,
                value: CoreNumberLiteralValue::Int(ast::Int::ONE),
            }))
        }
        ClosureInit::RuntimeNone | ClosureInit::Deferred => {
            core_name_expr("__dp_NONE", ast::ExprContext::Load, node_index, range)
        }
    }
}

fn build_closure_slot_cell_init_assign(slot: &ClosureSlot) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = compat_node_index();
    let range = compat_range();
    BlockPyStmt::Assign(BlockPyAssign {
        target: ast::ExprName {
            id: slot.storage_name.as_str().into(),
            ctx: ast::ExprContext::Store,
            node_index: node_index.clone(),
            range,
        },
        value: core_positional_intrinsic_expr_with_meta(
            &MAKE_CELL_INTRINSIC,
            node_index,
            range,
            vec![closure_slot_init_expr(slot)],
        ),
    })
}

fn prepend_owned_cell_init_preamble(callable: &mut BlockPyFunction<CoreBlockPyPass>) {
    let init_stmts = match callable.kind {
        BlockPyFunctionKind::Function => {
            let mut storage_names = callable
                .semantic
                .local_cell_storage_names()
                .into_iter()
                .collect::<Vec<_>>();
            if storage_names.is_empty() {
                return;
            }
            storage_names.sort();
            let param_names = callable.params.names().into_iter().collect::<HashSet<_>>();
            storage_names
                .into_iter()
                .map(|storage_name| {
                    let logical_name = storage_name
                        .strip_prefix("_dp_cell_")
                        .expect("owned local cell storage should have _dp_cell_ prefix");
                    build_local_cell_init_assign(
                        storage_name.as_str(),
                        logical_name,
                        param_names.contains(logical_name),
                    )
                })
                .collect::<Vec<_>>()
        }
        BlockPyFunctionKind::Generator
        | BlockPyFunctionKind::Coroutine
        | BlockPyFunctionKind::AsyncGenerator => {
            let layout = callable
                .closure_layout
                .as_ref()
                .expect("generator-like visible function should have closure layout");
            layout
                .cellvars
                .iter()
                .chain(layout.runtime_cells.iter())
                .map(build_closure_slot_cell_init_assign)
                .collect::<Vec<_>>()
        }
    };
    callable
        .blocks
        .first_mut()
        .expect("BlockPyFunction should have at least one block")
        .body
        .splice(0..0, init_stmts.into_iter().map(Into::into));
}

fn store_cell_deleted_logical_name(
    expr: &CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
) -> Option<String> {
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic, args, ..
    }) = expr
    else {
        return None;
    };
    if intrinsic.name() != STORE_CELL_INTRINSIC.name() || args.len() != 2 {
        return None;
    }
    let CoreBlockPyExpr::Name(name) = &args[0] else {
        return None;
    };
    let value_expr = &args[1];
    if !is_deleted_sentinel_expr(value_expr) {
        return None;
    }
    semantic.logical_name_for_cell_storage(name.id.as_str())
}

fn del_deref_logical_name(
    expr: &CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
) -> Option<String> {
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic, args, ..
    }) = expr
    else {
        return None;
    };
    if intrinsic.name() != DEL_DEREF_INTRINSIC.name() || args.len() != 1 {
        return None;
    }
    let CoreBlockPyExpr::Name(name) = &args[0] else {
        return None;
    };
    semantic.logical_name_for_cell_storage(name.id.as_str())
}

fn store_cell_runtime_logical_name(
    expr: &CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
) -> Option<String> {
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic, args, ..
    }) = expr
    else {
        return None;
    };
    if intrinsic.name() != STORE_CELL_INTRINSIC.name() || args.len() != 2 {
        return None;
    }
    let CoreBlockPyExpr::Name(name) = &args[0] else {
        return None;
    };
    if is_deleted_sentinel_expr(&args[1]) {
        return None;
    }
    semantic.logical_name_for_cell_storage(name.id.as_str())
}

fn is_local_cell_init_assign(assign: &BlockPyAssign<CoreBlockPyExpr>) -> bool {
    let Some(logical_name) = assign.target.id.as_str().strip_prefix("_dp_cell_") else {
        return false;
    };
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic, args, ..
    }) = &assign.value
    else {
        return false;
    };
    if args.len() != 1 {
        return false;
    }
    if intrinsic.name() != MAKE_CELL_INTRINSIC.name() {
        return false;
    }
    matches!(
        &args[0],
        CoreBlockPyExpr::Name(name) if name.id.as_str() == logical_name
    )
}

struct NameBindingMapper<'a> {
    semantic: &'a BlockPyCallableSemanticInfo,
}

impl NameBindingMapper<'_> {}

fn rewrite_binding_assign_by_name(
    name: String,
    value: CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> BlockPyStmt<CoreBlockPyExpr> {
    let assign = BlockPyAssign {
        target: ast::ExprName {
            id: name.clone().into(),
            ctx: ast::ExprContext::Store,
            node_index: node_index.clone(),
            range,
        },
        value,
    };
    if semantic.is_cell_binding(name.as_str()) {
        if is_deleted_sentinel_expr(&assign.value) {
            return BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
                &DEL_DEREF_INTRINSIC,
                node_index.clone(),
                range,
                vec![cell_expr_for_name(
                    name.as_str(),
                    semantic,
                    node_index,
                    range,
                )],
            ));
        }
        return rewrite_cell_binding_assign(assign, semantic);
    }
    match semantic.binding_target_for_name(name.as_str(), BlockPyBindingPurpose::Store) {
        BindingTarget::ModuleGlobal => {
            if is_deleted_sentinel_expr(&assign.value) {
                return rewrite_global_binding_delete_by_name(name.as_str(), node_index, range);
            }
            rewrite_global_binding_assign(assign)
        }
        BindingTarget::ClassNamespace => {
            if is_deleted_sentinel_expr(&assign.value) {
                return BlockPyStmt::Expr(core_positional_call_expr_with_meta(
                    "__dp_delitem",
                    node_index.clone(),
                    range,
                    vec![
                        class_namespace_expr(node_index.clone(), range),
                        core_string_expr(name, node_index, range),
                    ],
                ));
            }
            rewrite_class_namespace_binding_assign(assign)
        }
        BindingTarget::Local => BlockPyStmt::Assign(assign),
    }
}

impl BlockPyModuleMap<CoreBlockPyPass, CoreBlockPyPass> for NameBindingMapper<'_> {
    fn map_stmt(&self, stmt: BlockPyStmt<CoreBlockPyExpr>) -> BlockPyStmt<CoreBlockPyExpr> {
        match stmt {
            BlockPyStmt::Expr(expr) => {
                if let Some(name) = quiet_delete_marker_target(&expr) {
                    return rewrite_quiet_delete_marker(name, self.semantic);
                }
                BlockPyStmt::Expr(self.map_expr(expr))
            }
            BlockPyStmt::Assign(assign) => self.map_assign(assign),
            BlockPyStmt::Delete(delete) => rewrite_binding_delete(delete.target, self.semantic),
            BlockPyStmt::If(_) => unreachable!("structured if should not reach name_binding"),
        }
    }

    fn map_assign(&self, assign: BlockPyAssign<CoreBlockPyExpr>) -> BlockPyStmt<CoreBlockPyExpr> {
        if is_local_cell_init_assign(&assign) {
            return BlockPyStmt::Assign(assign);
        }
        rewrite_binding_assign_by_name(
            assign.target.id.to_string(),
            self.map_expr(assign.value),
            self.semantic,
            assign.target.node_index,
            assign.target.range,
        )
    }

    fn map_expr(&self, expr: CoreBlockPyExpr) -> CoreBlockPyExpr {
        match expr {
            CoreBlockPyExpr::Name(name)
                if should_late_bind_name(name.id.as_str(), self.semantic)
                    && self.semantic.scope_kind == BlockPyCallableScopeKind::Class =>
            {
                match self
                    .semantic
                    .effective_binding(name.id.as_str(), BlockPyBindingPurpose::Load)
                {
                    Some(BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Cell)) => {
                        rewrite_class_name_load_cell(name, self.semantic)
                    }
                    Some(BlockPyEffectiveBinding::Cell(_)) => {
                        rewrite_cell_name_load(name, self.semantic)
                    }
                    Some(BlockPyEffectiveBinding::Global) => rewrite_global_name_load(name),
                    Some(BlockPyEffectiveBinding::Local) => CoreBlockPyExpr::Name(name),
                    Some(BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Global))
                    | None => rewrite_class_name_load_global(name),
                }
            }
            CoreBlockPyExpr::Name(name)
                if should_late_bind_name(name.id.as_str(), self.semantic)
                    && matches!(
                        self.semantic.resolved_load_binding_kind(name.id.as_str()),
                        BlockPyBindingKind::Cell(_)
                    ) =>
            {
                rewrite_cell_name_load(name, self.semantic)
            }
            CoreBlockPyExpr::Name(name)
                if should_late_bind_name(name.id.as_str(), self.semantic)
                    && self.semantic.resolved_load_binding_kind(name.id.as_str())
                        == BlockPyBindingKind::Global =>
            {
                rewrite_global_name_load(name)
            }
            CoreBlockPyExpr::Name(name) => CoreBlockPyExpr::Name(name),
            CoreBlockPyExpr::Literal(literal) => CoreBlockPyExpr::Literal(literal),
            expr if cell_ref_marker_target(&expr).is_some() => {
                let target_name = cell_ref_marker_target(&expr)
                    .expect("cell-ref marker target should exist after guard");
                let (node_index, range) = match &expr {
                    CoreBlockPyExpr::Intrinsic(call) => (call.node_index.clone(), call.range),
                    _ => unreachable!("cell-ref marker should be intrinsic"),
                };
                rewrite_cell_ref_expr(target_name.as_str(), self.semantic, node_index, range)
            }
            CoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index,
                range,
                func,
                args,
                keywords,
            }) => {
                if args.is_empty()
                    && keywords.is_empty()
                    && matches!(
                        func.as_ref(),
                        CoreBlockPyExpr::Name(name)
                            if name.id.as_str() == "globals"
                                && self.semantic.resolved_load_binding_kind("globals")
                                    == BlockPyBindingKind::Global
                    )
                {
                    return globals_expr(node_index, range);
                }
                CoreBlockPyExpr::Call(CoreBlockPyCall {
                    node_index,
                    range,
                    func: Box::new(self.map_expr(*func)),
                    args: self.map_call_args(args),
                    keywords: self.map_keyword_args(keywords),
                })
            }
            CoreBlockPyExpr::Intrinsic(call) => CoreBlockPyExpr::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: map_intrinsic_args_with(call.args, |expr| self.map_expr(expr)),
            }),
        }
    }
}

fn collect_deleted_names_in_stmt(
    stmt: &BbStmt<CoreBlockPyExpr, ExprName>,
    semantic: &BlockPyCallableSemanticInfo,
    names: &mut HashSet<String>,
) {
    match stmt {
        BbStmt::Assign(assign)
            if semantic.has_local_def(assign.target.id.as_str())
                && is_deleted_sentinel_expr(&assign.value) =>
        {
            names.insert(assign.target.id.to_string());
        }
        BbStmt::Expr(expr) => {
            if let Some(name) = store_cell_deleted_logical_name(expr, semantic) {
                names.insert(name);
            }
            if let Some(name) = del_deref_logical_name(expr, semantic) {
                names.insert(name);
            }
        }
        BbStmt::Delete(_) => {}
        _ => {}
    }
}

fn rewrite_deleted_name_loads_in_stmt(
    stmt: &mut BbStmt<CoreBlockPyExpr, ExprName>,
    semantic: &BlockPyCallableSemanticInfo,
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    match stmt {
        BbStmt::Assign(assign) => {
            rewrite_deleted_name_loads_in_expr(
                &mut assign.value,
                semantic,
                deleted_names,
                always_unbound_names,
            );
        }
        BbStmt::Expr(expr) => {
            rewrite_deleted_name_loads_in_expr(expr, semantic, deleted_names, always_unbound_names)
        }
        BbStmt::Delete(_) => {}
    }
}

fn rewrite_deleted_name_loads_in_term(
    term: &mut BlockPyTerm<CoreBlockPyExpr>,
    semantic: &BlockPyCallableSemanticInfo,
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => {
            rewrite_deleted_name_loads_in_expr(
                &mut if_term.test,
                semantic,
                deleted_names,
                always_unbound_names,
            );
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_deleted_name_loads_in_expr(
                &mut branch.index,
                semantic,
                deleted_names,
                always_unbound_names,
            );
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                rewrite_deleted_name_loads_in_expr(
                    exc,
                    semantic,
                    deleted_names,
                    always_unbound_names,
                );
            }
        }
        BlockPyTerm::Return(value) => {
            rewrite_deleted_name_loads_in_expr(value, semantic, deleted_names, always_unbound_names)
        }
    }
}

fn collect_deleted_names_in_blocks(
    blocks: &[crate::block_py::CfgBlock<
        <CoreBlockPyPass as crate::block_py::BlockPyPass>::Stmt,
        crate::block_py::BlockPyTerm<CoreBlockPyExpr>,
    >],
    semantic: &BlockPyCallableSemanticInfo,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            collect_deleted_names_in_stmt(stmt, semantic, &mut names);
        }
    }
    names
}

fn collect_runtime_bound_local_names_in_stmt(
    stmt: &BbStmt<CoreBlockPyExpr, ExprName>,
    semantic: &BlockPyCallableSemanticInfo,
    names: &mut HashSet<String>,
) {
    match stmt {
        BbStmt::Assign(assign)
            if semantic.has_local_def(assign.target.id.as_str())
                && !is_deleted_sentinel_expr(&assign.value) =>
        {
            names.insert(assign.target.id.to_string());
        }
        BbStmt::Expr(expr) => {
            if let Some(name) = store_cell_runtime_logical_name(expr, semantic) {
                names.insert(name);
            }
        }
        BbStmt::Delete(_) => {}
        _ => {}
    }
}

fn collect_runtime_bound_local_names(
    blocks: &[crate::block_py::CfgBlock<
        <CoreBlockPyPass as crate::block_py::BlockPyPass>::Stmt,
        crate::block_py::BlockPyTerm<CoreBlockPyExpr>,
    >],
    semantic: &BlockPyCallableSemanticInfo,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            collect_runtime_bound_local_names_in_stmt(stmt, semantic, &mut names);
        }
    }
    names
}

fn collect_always_unbound_local_names(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashSet<String> {
    let semantic = &callable.semantic;
    let param_names = callable.params.names().into_iter().collect::<HashSet<_>>();
    let runtime_bound_names = collect_runtime_bound_local_names(&callable.blocks, semantic);
    semantic
        .local_defs
        .iter()
        .filter(|name| !param_names.contains(*name))
        .filter(|name| !is_internal_symbol(name.as_str()))
        .filter(|name| !runtime_bound_names.contains(*name))
        .filter(|name| {
            matches!(
                semantic.effective_binding(name.as_str(), BlockPyBindingPurpose::Load),
                Some(BlockPyEffectiveBinding::Local | BlockPyEffectiveBinding::Cell(_))
            )
        })
        .cloned()
        .collect()
}

fn collect_remaining_names_in_expr(expr: &CoreBlockPyExpr, names: &mut HashSet<String>) {
    match expr {
        CoreBlockPyExpr::Name(name) => {
            names.insert(name.id.to_string());
        }
        CoreBlockPyExpr::Literal(_) => {}
        CoreBlockPyExpr::Call(CoreBlockPyCall {
            func,
            args,
            keywords,
            ..
        }) => {
            collect_remaining_names_in_expr(func, names);
            for arg in args {
                collect_remaining_names_in_expr(arg.expr(), names);
            }
            for keyword in keywords {
                collect_remaining_names_in_expr(keyword.expr(), names);
            }
        }
        CoreBlockPyExpr::Intrinsic(call) => {
            for arg in &call.args {
                collect_remaining_names_in_expr(arg, names);
            }
        }
    }
}

fn collect_remaining_names_in_stmt(
    stmt: &BbStmt<CoreBlockPyExpr, ExprName>,
    names: &mut HashSet<String>,
) {
    match stmt {
        BbStmt::Assign(assign) => {
            names.insert(assign.target.id.to_string());
            collect_remaining_names_in_expr(&assign.value, names);
        }
        BbStmt::Expr(expr) => collect_remaining_names_in_expr(expr, names),
        BbStmt::Delete(delete) => {
            names.insert(delete.target.id.to_string());
        }
    }
}

fn collect_remaining_names_in_term(
    term: &BlockPyTerm<CoreBlockPyExpr>,
    names: &mut HashSet<String>,
) {
    match term {
        BlockPyTerm::Jump(edge) => {
            for arg in &edge.args {
                if let crate::block_py::BlockArg::Name(name) = arg {
                    names.insert(name.clone());
                }
            }
        }
        BlockPyTerm::IfTerm(if_term) => collect_remaining_names_in_expr(&if_term.test, names),
        BlockPyTerm::BranchTable(branch) => collect_remaining_names_in_expr(&branch.index, names),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                collect_remaining_names_in_expr(exc, names);
            }
        }
        BlockPyTerm::Return(value) => collect_remaining_names_in_expr(value, names),
    }
}

fn resolve_cell_storage_name(semantic: &BlockPyCallableSemanticInfo, name: &str) -> Option<String> {
    semantic
        .logical_name_for_cell_capture_source(name)
        .map(|logical_name| semantic.cell_storage_name(logical_name.as_str()))
}

fn resolve_captured_cell_source_storage_name(
    semantic: &BlockPyCallableSemanticInfo,
    name: &str,
) -> Option<String> {
    let logical_name = semantic.logical_name_for_cell_capture_source(name)?;
    if semantic.binding_kind(logical_name.as_str())
        != Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    {
        return None;
    }
    let capture_source_name = semantic.cell_capture_source_name(logical_name.as_str());
    let storage_name = semantic.cell_storage_name(logical_name.as_str());
    (capture_source_name == name && capture_source_name != storage_name).then_some(storage_name)
}

fn collect_cell_slot_locations(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashMap<String, u32> {
    let mut slots = HashMap::new();
    if let Some(layout) = callable.closure_layout.as_ref() {
        for (slot, closure_slot) in layout
            .freevars
            .iter()
            .chain(layout.cellvars.iter())
            .chain(layout.runtime_cells.iter())
            .enumerate()
        {
            slots.insert(closure_slot.storage_name.clone(), slot as u32);
            slots.insert(closure_slot.logical_name.clone(), slot as u32);
        }
    }
    slots
}

fn collect_inherited_capture_names(callable: &BlockPyFunction<CoreBlockPyPass>) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Some(layout) = callable.closure_layout.as_ref() {
        for slot in layout
            .freevars
            .iter()
            .chain(layout.cellvars.iter())
            .chain(layout.runtime_cells.iter())
        {
            if matches!(slot.init, ClosureInit::InheritedCapture) {
                names.insert(slot.logical_name.clone());
                names.insert(slot.storage_name.clone());
            }
        }
    }
    names
}

fn is_remaining_local_name(name: &str, semantic: &BlockPyCallableSemanticInfo) -> bool {
    if resolve_cell_storage_name(semantic, name).is_some() {
        return false;
    }
    match semantic.binding_kind(name) {
        Some(BlockPyBindingKind::Local) => semantic.honors_internal_binding(name),
        Some(BlockPyBindingKind::Cell(_)) | Some(BlockPyBindingKind::Global) => false,
        None => semantic.has_local_def(name),
    }
}

fn collect_local_slot_locations(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashMap<String, u32> {
    let mut slots = HashMap::new();
    for (slot, param_name) in callable.params.names().into_iter().enumerate() {
        slots.insert(param_name, slot as u32);
    }

    let mut remaining = HashSet::new();
    for block in &callable.blocks {
        for stmt in &block.body {
            collect_remaining_names_in_stmt(stmt, &mut remaining);
        }
        collect_remaining_names_in_term(&block.term, &mut remaining);
    }

    let mut non_param_locals = remaining
        .into_iter()
        .filter(|name| !slots.contains_key(name))
        .filter(|name| is_remaining_local_name(name, &callable.semantic))
        .collect::<Vec<_>>();
    non_param_locals.sort();

    let next_slot = slots.len() as u32;
    for (offset, name) in non_param_locals.into_iter().enumerate() {
        slots.insert(name, next_slot + offset as u32);
    }
    slots
}

struct NameLocator<'a> {
    semantic: &'a BlockPyCallableSemanticInfo,
    local_slots: HashMap<String, u32>,
    cell_slots: HashMap<String, u32>,
    inherited_capture_names: HashSet<String>,
}

impl NameLocator<'_> {
    fn locate_name(&self, name: ExprName) -> LocatedName {
        let name_text = name.id.to_string();
        let location = if let Some(storage_name) =
            resolve_captured_cell_source_storage_name(self.semantic, name_text.as_str())
        {
            let slot = self.cell_slots.get(storage_name.as_str()).copied().unwrap_or_else(|| {
                panic!(
                    "missing closure slot for captured cell source {name_text} via storage name {storage_name}"
                )
            });
            NameLocation::CapturedCellSource { slot }
        } else if let Some(storage_name) =
            resolve_cell_storage_name(self.semantic, name_text.as_str())
        {
            let slot = self.cell_slots.get(storage_name.as_str()).copied().unwrap_or_else(|| {
                panic!(
                    "missing closure slot for storage name {storage_name} while locating {name_text}"
                )
            });
            NameLocation::ClosureCell { slot }
        } else if let Some(slot) = self.local_slots.get(name_text.as_str()).copied() {
            NameLocation::Local { slot }
        } else {
            NameLocation::Global
        };
        LocatedName::from(name).with_location(location)
    }

    fn mark_raw_cell_name(&self, name: LocatedName) -> LocatedName {
        match name.location {
            NameLocation::ClosureCell { slot }
                if self.inherited_capture_names.contains(name.id.as_str()) =>
            {
                name.with_location(NameLocation::CapturedCellSource { slot })
            }
            _ => name,
        }
    }

    fn mark_raw_cell_expr(
        &self,
        expr: CoreBlockPyExpr<LocatedName>,
    ) -> CoreBlockPyExpr<LocatedName> {
        match expr {
            CoreBlockPyExpr::Name(name) => CoreBlockPyExpr::Name(self.mark_raw_cell_name(name)),
            other => other,
        }
    }
}

impl BlockPyModuleMap<CoreBlockPyPass, BbBlockPyPass> for NameLocator<'_> {
    fn map_name(&self, name: ExprName) -> LocatedName {
        self.locate_name(name)
    }

    fn map_expr(&self, expr: CoreBlockPyExpr) -> CoreBlockPyExpr<LocatedName> {
        match expr {
            CoreBlockPyExpr::Name(name) => CoreBlockPyExpr::Name(self.locate_name(name)),
            CoreBlockPyExpr::Literal(literal) => CoreBlockPyExpr::Literal(literal),
            CoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index,
                range,
                func,
                args,
                keywords,
            }) => {
                let func = Box::new(self.map_expr(*func));
                let mut args = self.map_call_args(args);
                if matches!(
                    func.as_ref(),
                    CoreBlockPyExpr::Name(func_name)
                        if func_name.id.as_str() == "__dp_class_lookup_cell"
                ) && args.len() == 3
                {
                    if let Some(CoreBlockPyCallArg::Positional(expr)) = args.get_mut(2) {
                        *expr = self.mark_raw_cell_expr(expr.clone());
                    }
                }
                CoreBlockPyExpr::Call(CoreBlockPyCall {
                    node_index,
                    range,
                    func,
                    args,
                    keywords: self.map_keyword_args(keywords),
                })
            }
            CoreBlockPyExpr::Intrinsic(call) => {
                let intrinsic = call.intrinsic;
                let mut args = map_intrinsic_args_with(call.args, |expr| self.map_expr(expr));
                let marks_first_arg_as_raw_cell = matches!(
                    intrinsic.name(),
                    name if name == CELL_REF_INTRINSIC.name()
                        || name == LOAD_CELL_INTRINSIC.name()
                        || name == STORE_CELL_INTRINSIC.name()
                        || name == DEL_DEREF_INTRINSIC.name()
                        || name == DEL_DEREF_QUIETLY_INTRINSIC.name()
                );
                if marks_first_arg_as_raw_cell {
                    if let Some(first) = args.get_mut(0) {
                        *first = self.mark_raw_cell_expr(first.clone());
                    }
                }
                CoreBlockPyExpr::Intrinsic(IntrinsicCall {
                    intrinsic,
                    node_index: call.node_index,
                    range: call.range,
                    args,
                })
            }
        }
    }
}

fn locate_names_in_callable(
    callable: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<BbBlockPyPass> {
    let semantic = callable.semantic.clone();
    let local_slots = collect_local_slot_locations(&callable);
    let cell_slots = collect_cell_slot_locations(&callable);
    let inherited_capture_names = collect_inherited_capture_names(&callable);
    NameLocator {
        semantic: &semantic,
        local_slots,
        cell_slots,
        inherited_capture_names,
    }
    .map_fn(callable)
}

fn refresh_bb_callable_block_params(
    mut callable: BlockPyFunction<BbBlockPyPass>,
) -> BlockPyFunction<BbBlockPyPass> {
    rewrite_current_exception_in_core_blocks(&mut callable.blocks);
    let block_params = recompute_lowered_block_params(
        &callable,
        should_include_closure_storage_aliases(&callable),
    );
    let BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        closure_layout,
        semantic,
    } = callable;
    let mut blocks = blocks
        .into_iter()
        .map(|block| {
            let existing_param_names = block
                .param_names()
                .map(ToString::to_string)
                .collect::<HashSet<_>>();
            let mut params = block_params
                .get(block.label.as_str())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|param| !existing_param_names.contains(param))
                .map(|name| crate::block_py::BlockParam {
                    name,
                    role: crate::block_py::BlockParamRole::Local,
                })
                .collect::<Vec<_>>();
            params.extend(block.bb_params().cloned());
            crate::block_py::CfgBlock {
                label: block.label,
                body: block.body,
                term: block.term,
                params,
                exc_edge: block.exc_edge,
            }
        })
        .collect::<Vec<_>>();
    populate_exception_edge_args(&mut blocks);
    BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        closure_layout,
        semantic,
    }
}

fn lower_name_binding_callable(
    callable: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<BbBlockPyPass> {
    let semantic = callable.semantic.clone();
    let mut lowered = NameBindingMapper {
        semantic: &semantic,
    }
    .map_fn(callable);
    prepend_owned_cell_init_preamble(&mut lowered);
    let deleted_names = collect_deleted_names_in_blocks(&lowered.blocks, &semantic);
    let always_unbound_names = collect_always_unbound_local_names(&lowered);
    if !deleted_names.is_empty() || !always_unbound_names.is_empty() {
        for block in &mut lowered.blocks {
            for stmt in &mut block.body {
                rewrite_deleted_name_loads_in_stmt(
                    stmt,
                    &semantic,
                    &deleted_names,
                    &always_unbound_names,
                );
            }
            rewrite_deleted_name_loads_in_term(
                &mut block.term,
                &semantic,
                &deleted_names,
                &always_unbound_names,
            );
        }
    }
    refresh_bb_callable_block_params(locate_names_in_callable(lowered))
}

pub(crate) fn lower_name_binding_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPass>,
) -> BlockPyModule<BbBlockPyPass> {
    module.map_callable_defs(lower_name_binding_callable)
}

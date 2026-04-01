use crate::block_py::{
    build_storage_layout_from_capture_names, compute_storage_layout_from_semantics,
    core_runtime_positional_call_expr_with_meta, BindingTarget, BlockArg, BlockPyAssign,
    BlockPyBindingKind, BlockPyBindingPurpose, BlockPyCallableScopeKind,
    BlockPyCallableSemanticInfo, BlockPyCellBindingKind, BlockPyClassBodyFallback,
    BlockPyEffectiveBinding, BlockPyFunction, BlockPyFunctionKind, BlockPyModule, BlockPyModuleMap,
    BlockPyNameLike, BlockPyRaise, BlockPyStmt, BlockPyTerm, Call, CellLocation, CellRef,
    CellRefForName, ClosureInit, ClosureSlot, CoreBlockPyCallArg, CoreBlockPyExpr,
    CoreBlockPyLiteral, CoreNumberLiteral, CoreNumberLiteralValue, CoreStringLiteral, DelItem,
    DelLocation, DelName, FunctionId, HasMeta, LoadLocation, LoadName, LoadRuntime, LocalLocation,
    LocatedName, MakeCell, MakeFunction, NameLocation, OperationDetail, SetItem, StorageLayout,
    StoreLocation, StoreName, WithMeta,
};
use crate::passes::ruff_to_blockpy::{
    populate_exception_edge_args, rewrite_current_exception_in_core_blocks,
};
use crate::passes::{CoreBlockPyPass, ResolvedStorageBlockPyPass};
use ruff_python_ast::{self as ast, ExprName};
use std::collections::{HashMap, HashSet};

fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name == "__soac__"
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

fn core_int_expr(
    value: usize,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    let text = value.to_string();
    CoreBlockPyExpr::Literal(CoreBlockPyLiteral::NumberLiteral(CoreNumberLiteral {
        node_index,
        range,
        value: CoreNumberLiteralValue::Int(
            ast::Int::from_str_radix(text.as_str(), 10, text.as_str())
                .expect("function id should round-trip through Int"),
        ),
    }))
}

fn globals_expr(
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    core_runtime_positional_call_expr_with_meta("globals", node_index, range, Vec::new())
}

fn op_expr(operation: OperationDetail<CoreBlockPyExpr>) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Op(operation)
}

fn op_stmt(operation: OperationDetail<CoreBlockPyExpr>) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    BlockPyStmt::Expr(op_expr(operation))
}

fn rewrite_global_name_load(name: ExprName) -> CoreBlockPyExpr {
    let meta = name.meta();
    let bind_name = name.id.to_string();
    op_expr(OperationDetail::from(LoadName::new(bind_name)).with_meta(meta))
}

fn rewrite_local_name_load(name: ExprName, resolver: &NameBindingMapper<'_>) -> CoreBlockPyExpr {
    let meta = name.meta();
    op_expr(
        OperationDetail::from(LoadLocation::new(NameLocation::Local(
            resolver.resolve_raw_local_location(name.id.as_str()),
        )))
        .with_meta(meta),
    )
}

fn cell_name_for_name(
    name: &str,
    semantic: &BlockPyCallableSemanticInfo,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> ExprName {
    cell_named_expr(semantic.cell_storage_name(name).as_str(), node_index, range)
}

fn cell_storage_name_for_name(name: &str, semantic: &BlockPyCallableSemanticInfo) -> String {
    semantic.cell_storage_name(name)
}

fn cell_named_expr(
    storage_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> ExprName {
    ExprName {
        id: storage_name.into(),
        ctx: ast::ExprContext::Load,
        node_index,
        range,
    }
}

fn cell_expr_for_name(
    name: &str,
    semantic: &BlockPyCallableSemanticInfo,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Name(cell_name_for_name(name, semantic, node_index, range))
}

fn rewrite_cell_name_load(
    name: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) -> CoreBlockPyExpr {
    let meta = name.meta();
    let location = resolver
        .resolve_raw_cell_location(cell_storage_name_for_name(name.id.as_str(), semantic).as_str());
    op_expr(OperationDetail::from(LoadLocation::new(NameLocation::Cell(location))).with_meta(meta))
}

fn rewrite_raw_cell_storage_name_load(
    name: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) -> Option<CoreBlockPyExpr> {
    let meta = name.meta();
    let storage_name = resolve_cell_storage_name(semantic, name.id.as_str())?;
    let location = resolver.resolve_raw_cell_location(storage_name.as_str());
    Some(op_expr(
        OperationDetail::from(LoadLocation::new(NameLocation::Cell(location))).with_meta(meta),
    ))
}

fn raw_load_name<N>(expr: &CoreBlockPyExpr<N>) -> Option<String>
where
    N: BlockPyNameLike,
{
    match expr {
        CoreBlockPyExpr::Name(name) => Some(name.id_str().to_string()),
        CoreBlockPyExpr::Op(operation) => match operation {
            OperationDetail::LoadRuntime(op) => Some(op.name.clone()),
            OperationDetail::LoadName(op) => Some(op.name.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn rewrite_name_load(
    name: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) -> CoreBlockPyExpr {
    if is_internal_symbol(name.id.as_str()) && !semantic.honors_internal_binding(name.id.as_str()) {
        return CoreBlockPyExpr::Name(name);
    }

    if semantic.scope_kind == BlockPyCallableScopeKind::Class {
        return match semantic.effective_binding(name.id.as_str(), BlockPyBindingPurpose::Load) {
            Some(BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Cell)) => {
                rewrite_class_name_load_cell(name, semantic)
            }
            Some(BlockPyEffectiveBinding::Cell(_)) => {
                rewrite_cell_name_load(name, semantic, resolver)
            }
            Some(BlockPyEffectiveBinding::Global) => rewrite_global_name_load(name),
            Some(BlockPyEffectiveBinding::Local) => rewrite_local_name_load(name, resolver),
            Some(BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Global)) | None => {
                rewrite_class_name_load_global(name)
            }
        };
    }

    match semantic.resolved_load_binding_kind(name.id.as_str()) {
        BlockPyBindingKind::Cell(_) => rewrite_cell_name_load(name, semantic, resolver),
        BlockPyBindingKind::Global => rewrite_global_name_load(name),
        BlockPyBindingKind::Local => rewrite_local_name_load(name, resolver),
    }
}

fn should_rewrite_raw_name_load(name: &str, semantic: &BlockPyCallableSemanticInfo) -> bool {
    if !is_internal_symbol(name) && should_late_bind_name(name, semantic) {
        return true;
    }

    matches!(
        semantic.effective_binding(name, BlockPyBindingPurpose::Load),
        Some(
            BlockPyEffectiveBinding::Global
                | BlockPyEffectiveBinding::Cell(_)
                | BlockPyEffectiveBinding::ClassBody(_)
        )
    )
}

fn rewrite_cell_ref_expr(
    logical_name: &str,
    _semantic: &BlockPyCallableSemanticInfo,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    op_expr(
        OperationDetail::from(CellRefForName::new(logical_name.to_string()))
            .with_meta(crate::block_py::Meta::new(node_index.clone(), range)),
    )
}

fn rewrite_global_binding_assign(
    assign: BlockPyAssign<CoreBlockPyExpr>,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    let meta = assign.target.meta();
    let bind_name = assign.target.id.to_string();
    op_stmt(
        OperationDetail::from(StoreName::new(bind_name, Box::new(assign.value))).with_meta(meta),
    )
}

fn rewrite_class_namespace_binding_assign(
    assign: BlockPyAssign<CoreBlockPyExpr>,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    let meta = assign.target.meta();
    let bind_name = assign.target.id.to_string();
    op_stmt(
        OperationDetail::from(SetItem::new(
            Box::new(class_namespace_expr(meta.node_index.clone(), meta.range)),
            Box::new(core_string_expr(
                bind_name,
                meta.node_index.clone(),
                meta.range,
            )),
            Box::new(assign.value),
        ))
        .with_meta(meta),
    )
}

fn rewrite_cell_binding_assign(
    assign: BlockPyAssign<CoreBlockPyExpr>,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    let meta = assign.target.meta();
    op_stmt(
        OperationDetail::from(StoreLocation::new(
            NameLocation::Cell(resolver.resolve_raw_cell_location(
                cell_storage_name_for_name(assign.target.id.as_str(), semantic).as_str(),
            )),
            Box::new(assign.value),
        ))
        .with_meta(meta),
    )
}

fn rewrite_global_binding_delete_by_name(
    bind_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    op_stmt(
        OperationDetail::from(DelName::new(bind_name.to_string(), false))
            .with_meta(crate::block_py::Meta::new(node_index.clone(), range)),
    )
}

fn rewrite_binding_delete(
    target: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    let meta = target.meta();
    let bind_name = target.id.to_string();
    if semantic.is_cell_binding(bind_name.as_str()) {
        return op_stmt(
            OperationDetail::from(DelLocation::new(
                NameLocation::Cell(resolver.resolve_raw_cell_location(
                    cell_storage_name_for_name(bind_name.as_str(), semantic).as_str(),
                )),
                false,
            ))
            .with_meta(meta),
        );
    }
    match semantic.binding_target_for_name(bind_name.as_str(), BlockPyBindingPurpose::Store) {
        BindingTarget::Local => BlockPyStmt::Assign(BlockPyAssign {
            target: ast::ExprName {
                id: target.id,
                ctx: ast::ExprContext::Store,
                node_index: meta.node_index.clone(),
                range: meta.range,
            },
            value: deleted_sentinel_expr(meta.node_index, meta.range),
        }),
        BindingTarget::ModuleGlobal => {
            rewrite_global_binding_delete_by_name(bind_name.as_str(), meta.node_index, meta.range)
        }
        BindingTarget::ClassNamespace => op_stmt(
            OperationDetail::from(DelItem::new(
                Box::new(class_namespace_expr(meta.node_index.clone(), meta.range)),
                Box::new(core_string_expr(
                    bind_name,
                    meta.node_index.clone(),
                    meta.range,
                )),
            ))
            .with_meta(meta),
        ),
    }
}

fn rewrite_deleted_name_load_expr(
    name: ExprName,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) -> CoreBlockPyExpr {
    let always_unbound = always_unbound_names.contains(name.id.as_str());
    let deleted = deleted_names.contains(name.id.as_str());
    if !always_unbound && !deleted {
        return rewrite_name_load(name, semantic, resolver);
    }
    let node_index = name.node_index.clone();
    let range = name.range;
    core_runtime_positional_call_expr_with_meta(
        "load_deleted_name",
        node_index.clone(),
        range,
        vec![
            core_string_expr(name.id.to_string(), node_index.clone(), range),
            if always_unbound {
                deleted_sentinel_expr(node_index, range)
            } else {
                rewrite_name_load(name, semantic, resolver)
            },
        ],
    )
}

fn wrap_deleted_name_load_expr(
    logical_name: String,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    value: CoreBlockPyExpr,
) -> CoreBlockPyExpr {
    core_runtime_positional_call_expr_with_meta(
        "load_deleted_name",
        node_index.clone(),
        range,
        vec![
            core_string_expr(logical_name, node_index.clone(), range),
            value,
        ],
    )
}

fn operation_expr<N: BlockPyNameLike + Clone>(
    expr: &CoreBlockPyExpr<N>,
) -> Option<&OperationDetail<CoreBlockPyExpr<N>>> {
    match expr {
        CoreBlockPyExpr::Op(operation) => Some(operation),
        _ => None,
    }
}

fn operation_expr_mut<N: BlockPyNameLike + Clone>(
    expr: &mut CoreBlockPyExpr<N>,
) -> Option<&mut OperationDetail<CoreBlockPyExpr<N>>> {
    match expr {
        CoreBlockPyExpr::Op(operation) => Some(operation),
        _ => None,
    }
}

fn with_helper_arg_mut<N: BlockPyNameLike + Clone>(
    expr: &mut CoreBlockPyExpr<N>,
    index: usize,
    f: &mut impl FnMut(&mut CoreBlockPyExpr<N>),
) -> bool {
    match expr {
        CoreBlockPyExpr::Op(operation) => {
            let mut current = 0;
            let mut applied = false;
            operation.walk_args_mut(&mut |arg| {
                if current == index && !applied {
                    f(arg);
                    applied = true;
                }
                current += 1;
            });
            applied
        }
        _ => false,
    }
}

fn walk_helper_args_mut<N: BlockPyNameLike + Clone>(
    expr: &mut CoreBlockPyExpr<N>,
    f: &mut impl FnMut(&mut CoreBlockPyExpr<N>),
) {
    match expr {
        CoreBlockPyExpr::Op(operation) => operation.walk_args_mut(f),
        _ => unreachable!("helper arg walker only applies to op-like expressions"),
    }
}

fn rewrite_deleted_name_loads_in_expr(
    expr: &mut CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
    resolver: &NameBindingMapper<'_>,
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    if let Some(logical_name) = cell_load_logical_name(expr, semantic, storage_layout) {
        if deleted_names.contains(logical_name.as_str())
            || always_unbound_names.contains(logical_name.as_str())
        {
            let meta = expr.meta();
            *expr = core_runtime_positional_call_expr_with_meta(
                "load_deleted_name",
                meta.node_index.clone(),
                meta.range,
                vec![
                    core_string_expr(logical_name, meta.node_index.clone(), meta.range),
                    expr.clone(),
                ],
            );
            return;
        }
    }
    match expr {
        CoreBlockPyExpr::Name(name) if matches!(name.ctx, ast::ExprContext::Load) => {
            *expr = rewrite_deleted_name_load_expr(
                name.clone(),
                semantic,
                resolver,
                deleted_names,
                always_unbound_names,
            );
        }
        CoreBlockPyExpr::Op(operation) if matches!(operation, OperationDetail::LoadName(_)) => {
            let meta = operation.meta();
            let OperationDetail::LoadName(op) = operation else {
                unreachable!("load-name guard should ensure LoadName detail");
            };
            let always_unbound = always_unbound_names.contains(op.name.as_str());
            let deleted = deleted_names.contains(op.name.as_str());
            if always_unbound || deleted {
                *expr = wrap_deleted_name_load_expr(
                    op.name.clone(),
                    meta.node_index.clone(),
                    meta.range,
                    if always_unbound {
                        deleted_sentinel_expr(meta.node_index, meta.range)
                    } else {
                        expr.clone()
                    },
                );
            }
        }
        CoreBlockPyExpr::Op(operation)
            if matches!(
                operation,
                OperationDetail::LoadLocation(LoadLocation {
                    location: NameLocation::Local(_),
                    ..
                })
            ) =>
        {
            let OperationDetail::LoadLocation(op) = operation else {
                unreachable!("load-location guard should ensure LoadLocation detail");
            };
            let Some(logical_name) =
                logical_name_for_local_location(storage_layout, op.location.as_local().unwrap())
            else {
                return;
            };
            let always_unbound = always_unbound_names.contains(logical_name.as_str());
            let deleted = deleted_names.contains(logical_name.as_str());
            if always_unbound || deleted {
                let meta = operation.meta();
                *expr = wrap_deleted_name_load_expr(
                    logical_name,
                    meta.node_index.clone(),
                    meta.range,
                    if always_unbound {
                        deleted_sentinel_expr(meta.node_index, meta.range)
                    } else {
                        expr.clone()
                    },
                );
            }
        }
        CoreBlockPyExpr::Op(_) => {
            let Some(operation) = operation_expr(expr) else {
                unreachable!("op-like branch should have operation view");
            };
            match operation {
                OperationDetail::LoadName(_) => {}
                OperationDetail::LoadLocation(_)
                | OperationDetail::DelLocation(_)
                | OperationDetail::CellRefForName(_)
                | OperationDetail::CellRef(_) => {}
                OperationDetail::StoreName(_) | OperationDetail::StoreLocation(_) => {
                    with_helper_arg_mut(expr, 1, &mut |value_expr| {
                        rewrite_deleted_name_loads_in_expr(
                            value_expr,
                            semantic,
                            storage_layout,
                            resolver,
                            deleted_names,
                            always_unbound_names,
                        );
                    });
                }
                _ => walk_helper_args_mut(expr, &mut |arg| {
                    rewrite_deleted_name_loads_in_expr(
                        arg,
                        semantic,
                        storage_layout,
                        resolver,
                        deleted_names,
                        always_unbound_names,
                    )
                }),
            }
        }
        CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => {}
    }
}

fn core_name_expr(
    id: &str,
    ctx: ast::ExprContext,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    if matches!(ctx, ast::ExprContext::Load)
        && matches!(
            id,
            "DELETED"
                | "NONE"
                | "TRUE"
                | "FALSE"
                | "ELLIPSIS"
                | "globals"
                | "load_deleted_name"
                | "class_lookup_global"
                | "class_lookup_cell"
                | "tuple"
                | "make_function"
        )
    {
        return CoreBlockPyExpr::Op(
            OperationDetail::from(LoadRuntime::new(id.to_string()))
                .with_meta(crate::block_py::Meta::new(node_index, range)),
        );
    }
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
    core_name_expr("DELETED", ast::ExprContext::Load, node_index, range)
}

fn rewrite_class_name_load_global(name: ExprName) -> CoreBlockPyExpr {
    let node_index = name.node_index.clone();
    let range = name.range;
    let bind_name = name.id.to_string();
    core_runtime_positional_call_expr_with_meta(
        "class_lookup_global",
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
    core_runtime_positional_call_expr_with_meta(
        "class_lookup_cell",
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
    resolver: &NameBindingMapper<'_>,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    let node_index = name.node_index.clone();
    let range = name.range;
    let meta = crate::block_py::Meta::new(node_index.clone(), range);
    match semantic.binding_kind(name.id.as_str()) {
        Some(BlockPyBindingKind::Cell(_)) => op_stmt(
            OperationDetail::from(DelLocation::new(
                NameLocation::Cell(resolver.resolve_raw_cell_location(
                    cell_storage_name_for_name(name.id.as_str(), semantic).as_str(),
                )),
                true,
            ))
            .with_meta(meta),
        ),
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
            BindingTarget::ModuleGlobal => op_stmt(
                OperationDetail::from(DelName::new(name.id.to_string(), true)).with_meta(meta),
            ),
            BindingTarget::ClassNamespace => op_stmt(
                OperationDetail::from(DelItem::new(
                    Box::new(class_namespace_expr(node_index.clone(), range)),
                    Box::new(core_string_expr(
                        name.id.to_string(),
                        node_index.clone(),
                        range,
                    )),
                ))
                .with_meta(meta),
            ),
        },
    }
}

fn quiet_delete_marker_target(expr: &CoreBlockPyExpr) -> Option<ExprName> {
    let meta = expr.meta();
    let operation = operation_expr(expr)?;
    let OperationDetail::Call(call) = operation else {
        return None;
    };
    let Call {
        func,
        args,
        keywords,
        ..
    } = call;
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
        CoreBlockPyCallArg::Positional(expr) => {
            let nested = operation_expr(expr)?;
            let OperationDetail::Call(nested_call) = nested else {
                return raw_load_name(expr).map(|name| ExprName {
                    id: name.into(),
                    ctx: ast::ExprContext::Load,
                    node_index: meta.node_index,
                    range: meta.range,
                });
            };
            if !nested_call.keywords.is_empty()
                || nested_call.args.len() != 2
                || !raw_load_name(nested_call.func.as_ref())
                    .as_ref()
                    .is_some_and(|name| name == "load_deleted_name")
            {
                return raw_load_name(expr).map(|name| ExprName {
                    id: name.into(),
                    ctx: ast::ExprContext::Load,
                    node_index: meta.node_index,
                    range: meta.range,
                });
            }
            match &nested_call.args[1] {
                CoreBlockPyCallArg::Positional(expr) => raw_load_name(expr).map(|name| ExprName {
                    id: name.into(),
                    ctx: ast::ExprContext::Load,
                    node_index: meta.node_index.clone(),
                    range: meta.range,
                }),
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_deleted_sentinel_expr(expr: &CoreBlockPyExpr) -> bool {
    matches!(
        expr,
        CoreBlockPyExpr::Op(operation)
            if matches!(operation, OperationDetail::LoadRuntime(op) if op.name == "DELETED")
    )
}

fn cell_ref_marker_target(expr: &CoreBlockPyExpr) -> Option<String> {
    let operation = operation_expr(expr)?;
    let OperationDetail::CellRefForName(CellRefForName { logical_name, .. }) = operation else {
        return None;
    };
    Some(logical_name.clone())
}

fn make_function_kind_name(kind: BlockPyFunctionKind) -> &'static str {
    match kind {
        BlockPyFunctionKind::Function => "function",
        BlockPyFunctionKind::Coroutine => "coroutine",
        BlockPyFunctionKind::Generator => "generator",
        BlockPyFunctionKind::AsyncGenerator => "async_generator",
    }
}

fn cell_load_logical_name(
    expr: &CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
) -> Option<String> {
    let operation = operation_expr(expr)?;
    let OperationDetail::LoadLocation(LoadLocation { location, .. }) = operation else {
        return None;
    };
    logical_name_for_cell_location(semantic, storage_layout, location.as_cell()?)
}

fn build_local_cell_init_assign(
    storage_name: &str,
    logical_name: &str,
    is_parameter: bool,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
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
        value: op_expr(
            OperationDetail::from(MakeCell::new(Box::new(init_expr)))
                .with_meta(crate::block_py::Meta::new(node_index, range)),
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
        ClosureInit::RuntimeAbruptKindFallthrough => {
            CoreBlockPyExpr::Literal(CoreBlockPyLiteral::NumberLiteral(CoreNumberLiteral {
                node_index,
                range,
                value: CoreNumberLiteralValue::Int(
                    ast::Int::from_str_radix("0", 10, "0")
                        .expect("zero should parse as an integer literal"),
                ),
            }))
        }
        ClosureInit::RuntimeNone | ClosureInit::Deferred => {
            core_name_expr("NONE", ast::ExprContext::Load, node_index, range)
        }
    }
}

fn build_closure_slot_cell_init_assign(
    slot: &ClosureSlot,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    let node_index = compat_node_index();
    let range = compat_range();
    BlockPyStmt::Assign(BlockPyAssign {
        target: ast::ExprName {
            id: slot.storage_name.as_str().into(),
            ctx: ast::ExprContext::Store,
            node_index: node_index.clone(),
            range,
        },
        value: op_expr(
            OperationDetail::from(MakeCell::new(Box::new(closure_slot_init_expr(slot))))
                .with_meta(crate::block_py::Meta::new(node_index, range)),
        ),
    })
}

fn prepend_owned_cell_init_preamble(callable: &mut BlockPyFunction<CoreBlockPyPass>) {
    let init_stmts = match callable.kind {
        BlockPyFunctionKind::Function => {
            let mut storage_names = callable
                .semantic
                .owned_cell_storage_names()
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
                    let logical_name = callable
                        .semantic
                        .logical_name_for_cell_storage(storage_name.as_str())
                        .unwrap_or_else(|| storage_name.clone());
                    build_local_cell_init_assign(
                        storage_name.as_str(),
                        logical_name.as_str(),
                        param_names.contains(logical_name.as_str()),
                    )
                })
                .collect::<Vec<_>>()
        }
        BlockPyFunctionKind::Generator
        | BlockPyFunctionKind::Coroutine
        | BlockPyFunctionKind::AsyncGenerator => {
            let layout = callable
                .storage_layout
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

fn storage_name_for_cell_location(layout: &StorageLayout, location: CellLocation) -> Option<&str> {
    match location {
        CellLocation::Owned(slot) => layout
            .local_cell_slot(slot)
            .map(|slot| slot.storage_name.as_str()),
        CellLocation::Closure(slot) | CellLocation::CapturedSource(slot) => layout
            .freevar_slot(slot)
            .map(|slot| slot.storage_name.as_str()),
    }
}

fn logical_name_for_cell_location(
    semantic: &BlockPyCallableSemanticInfo,
    layout: &StorageLayout,
    location: CellLocation,
) -> Option<String> {
    let storage_name = storage_name_for_cell_location(layout, location)?;
    semantic.logical_name_for_cell_storage(storage_name)
}

fn logical_name_for_local_location(
    layout: &StorageLayout,
    location: LocalLocation,
) -> Option<String> {
    layout.stack_slots().get(location.slot() as usize).cloned()
}

fn store_cell_deleted_logical_name(
    expr: &CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
) -> Option<String> {
    let operation = operation_expr(expr)?;
    let OperationDetail::StoreLocation(StoreLocation {
        location, value, ..
    }) = operation
    else {
        return None;
    };
    if !is_deleted_sentinel_expr(value) {
        return None;
    }
    logical_name_for_cell_location(semantic, storage_layout, location.as_cell()?)
}

fn del_deref_logical_name(
    expr: &CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
) -> Option<String> {
    let operation = operation_expr(expr)?;
    let OperationDetail::DelLocation(DelLocation {
        location, quietly, ..
    }) = operation
    else {
        return None;
    };
    if *quietly {
        return None;
    }
    logical_name_for_cell_location(semantic, storage_layout, location.as_cell()?)
}

fn store_cell_runtime_logical_name(
    expr: &CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
) -> Option<String> {
    let operation = operation_expr(expr)?;
    let OperationDetail::StoreLocation(StoreLocation {
        location, value, ..
    }) = operation
    else {
        return None;
    };
    if is_deleted_sentinel_expr(value) {
        return None;
    }
    logical_name_for_cell_location(semantic, storage_layout, location.as_cell()?)
}

fn is_local_cell_init_assign(assign: &BlockPyAssign<CoreBlockPyExpr>) -> bool {
    let Some(logical_name) = assign.target.id.as_str().strip_prefix("_dp_cell_") else {
        return false;
    };
    let Some(operation) = operation_expr(&assign.value) else {
        return false;
    };
    let OperationDetail::MakeCell(MakeCell { initial_value, .. }) = operation else {
        return false;
    };
    matches!(raw_load_name(initial_value.as_ref()), Some(name) if name == logical_name)
}

struct NameBindingMapper<'a> {
    semantic: &'a BlockPyCallableSemanticInfo,
    callee_make_function_capture_names: &'a HashMap<crate::block_py::FunctionId, Vec<String>>,
    local_slots: HashMap<String, u32>,
    captured_cell_slots: HashMap<String, u32>,
    owned_cell_slots: HashMap<String, u32>,
    cell_bindings: HashMap<String, (String, BlockPyCellBindingKind)>,
}

impl NameBindingMapper<'_> {
    fn resolve_raw_local_location(&self, name_text: &str) -> LocalLocation {
        let slot = self.local_slots.get(name_text).copied().unwrap_or_else(|| {
            panic!("missing local slot for raw local target {name_text}");
        });
        LocalLocation(slot)
    }

    fn resolve_raw_cell_location(&self, name_text: &str) -> CellLocation {
        if let Some(storage_name) =
            resolve_captured_cell_source_storage_name(self.semantic, name_text)
        {
            let slot = self
                .captured_cell_slots
                .get(storage_name.as_str())
                .copied()
                .unwrap_or_else(|| {
                    panic!(
                        "missing closure slot for captured raw cell source {name_text} via storage name {storage_name}"
                    )
                });
            return CellLocation::CapturedSource(slot);
        }

        if let Some((storage_name, binding_kind)) = self.cell_bindings.get(name_text) {
            return match binding_kind {
                BlockPyCellBindingKind::Owner => {
                    let slot = self
                        .owned_cell_slots
                        .get(storage_name.as_str())
                        .copied()
                        .unwrap_or_else(|| {
                            panic!(
                                "missing owned cell slot for raw cell target {name_text} via storage name {storage_name}"
                            )
                        });
                    CellLocation::Owned(slot)
                }
                BlockPyCellBindingKind::Capture => {
                    let slot = self
                        .captured_cell_slots
                        .get(storage_name.as_str())
                        .copied()
                        .unwrap_or_else(|| {
                            panic!(
                                "missing closure slot for raw captured cell target {name_text} via storage name {storage_name}"
                            )
                        });
                    CellLocation::CapturedSource(slot)
                }
            };
        }

        panic!("raw cell target {name_text} did not resolve to a cell-backed location");
    }

    fn materialize_make_function_expr(
        &self,
        meta: crate::block_py::Meta,
        op: MakeFunction<CoreBlockPyExpr>,
    ) -> CoreBlockPyExpr {
        let captures = self
            .callee_make_function_capture_names
            .get(&op.function_id)
            .into_iter()
            .flat_map(|capture_names| capture_names.iter())
            .map(|logical_name| {
                core_runtime_positional_call_expr_with_meta(
                    "tuple_values",
                    meta.node_index.clone(),
                    meta.range,
                    vec![
                        core_string_expr(logical_name.clone(), meta.node_index.clone(), meta.range),
                        rewrite_cell_ref_expr(
                            logical_name.as_str(),
                            self.semantic,
                            meta.node_index.clone(),
                            meta.range,
                        ),
                    ],
                )
            })
            .collect::<Vec<_>>();
        let captures_expr = core_runtime_positional_call_expr_with_meta(
            "tuple_values",
            meta.node_index.clone(),
            meta.range,
            captures,
        );
        core_runtime_positional_call_expr_with_meta(
            "make_function",
            meta.node_index.clone(),
            meta.range,
            vec![
                core_int_expr(op.function_id.0, meta.node_index.clone(), meta.range),
                core_string_expr(
                    make_function_kind_name(op.kind).to_string(),
                    meta.node_index.clone(),
                    meta.range,
                ),
                captures_expr,
                self.map_expr(*op.param_defaults),
                self.map_expr(*op.annotate_fn),
            ],
        )
    }
}

fn rewrite_binding_assign_by_name(
    name: String,
    value: CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
    let meta = crate::block_py::Meta::new(node_index.clone(), range);
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
            return op_stmt(
                OperationDetail::from(DelLocation::new(
                    NameLocation::Cell(resolver.resolve_raw_cell_location(
                        cell_storage_name_for_name(name.as_str(), semantic).as_str(),
                    )),
                    false,
                ))
                .with_meta(meta),
            );
        }
        return rewrite_cell_binding_assign(assign, semantic, resolver);
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
                return op_stmt(
                    OperationDetail::from(DelItem::new(
                        Box::new(class_namespace_expr(node_index.clone(), range)),
                        Box::new(core_string_expr(name, node_index, range)),
                    ))
                    .with_meta(meta),
                );
            }
            rewrite_class_namespace_binding_assign(assign)
        }
        BindingTarget::Local => BlockPyStmt::Assign(assign),
    }
}

impl BlockPyModuleMap<CoreBlockPyPass, CoreBlockPyPass> for NameBindingMapper<'_> {
    fn map_stmt(
        &self,
        stmt: BlockPyStmt<CoreBlockPyExpr, ExprName>,
    ) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
        match stmt {
            BlockPyStmt::Expr(expr) => {
                if let Some(name) = quiet_delete_marker_target(&expr) {
                    return rewrite_quiet_delete_marker(name, self.semantic, self);
                }
                BlockPyStmt::Expr(self.map_expr(expr))
            }
            BlockPyStmt::Assign(assign) => self.map_assign(assign),
            BlockPyStmt::Delete(delete) => {
                rewrite_binding_delete(delete.target, self.semantic, self)
            }
        }
    }

    fn map_expr(&self, expr: CoreBlockPyExpr) -> CoreBlockPyExpr {
        match expr {
            CoreBlockPyExpr::Op(operation) if matches!(operation, OperationDetail::LoadName(_)) => {
                let meta = operation.meta();
                let detail = operation;
                let OperationDetail::LoadName(op) = detail else {
                    unreachable!("load-name guard should ensure LoadName detail");
                };
                rewrite_name_load(
                    ExprName {
                        id: op.name.into(),
                        ctx: ast::ExprContext::Load,
                        node_index: meta.node_index,
                        range: meta.range,
                    },
                    self.semantic,
                    self,
                )
            }
            CoreBlockPyExpr::Name(name)
                if matches!(name.ctx, ast::ExprContext::Load)
                    && resolve_cell_storage_name(self.semantic, name.id.as_str()).is_some() =>
            {
                rewrite_raw_cell_storage_name_load(name, self.semantic, self)
                    .expect("raw cell-storage load guard should ensure rewrite target")
            }
            CoreBlockPyExpr::Name(name)
                if should_rewrite_raw_name_load(name.id.as_str(), self.semantic) =>
            {
                rewrite_name_load(name, self.semantic, self)
            }
            CoreBlockPyExpr::Name(name) => CoreBlockPyExpr::Name(name),
            CoreBlockPyExpr::Literal(literal) => CoreBlockPyExpr::Literal(literal),
            expr if cell_ref_marker_target(&expr).is_some() => {
                let target_name = cell_ref_marker_target(&expr)
                    .expect("cell-ref marker target should exist after guard");
                let meta = expr.meta();
                rewrite_cell_ref_expr(
                    target_name.as_str(),
                    self.semantic,
                    meta.node_index,
                    meta.range,
                )
            }
            CoreBlockPyExpr::Op(operation) => {
                let meta = operation.meta();
                let detail = operation;
                match detail {
                    OperationDetail::MakeFunction(op) => {
                        self.materialize_make_function_expr(meta, op)
                    }
                    OperationDetail::Call(call)
                        if call.args.is_empty()
                            && call.keywords.is_empty()
                            && raw_load_name(call.func.as_ref())
                                .as_ref()
                                .is_some_and(|name| {
                                    name == "globals"
                                        && self.semantic.resolved_load_binding_kind("globals")
                                            == BlockPyBindingKind::Global
                                }) =>
                    {
                        globals_expr(meta.node_index, meta.range)
                    }
                    OperationDetail::Call(call)
                        if call.keywords.is_empty()
                            && call.args.len() == 3
                            && raw_load_name(call.func.as_ref())
                                .as_ref()
                                .is_some_and(|name| name == "class_lookup_cell") =>
                    {
                        let mut mapped_args = Vec::with_capacity(3);
                        for (index, arg) in call.args.into_iter().enumerate() {
                            match (index, arg) {
                                (2, arg) => mapped_args.push(arg),
                                (_, CoreBlockPyCallArg::Positional(expr)) => mapped_args
                                    .push(CoreBlockPyCallArg::Positional(self.map_expr(expr))),
                                (_, CoreBlockPyCallArg::Starred(expr)) => mapped_args
                                    .push(CoreBlockPyCallArg::Starred(self.map_expr(expr))),
                            }
                        }
                        CoreBlockPyExpr::Op(
                            OperationDetail::from(Call::new(
                                self.map_expr(*call.func),
                                mapped_args,
                                call.keywords,
                            ))
                            .with_meta(meta),
                        )
                    }
                    other => self.map_nested_expr(CoreBlockPyExpr::Op(
                        OperationDetail::from(other).with_meta(meta),
                    )),
                }
            }
        }
    }
}

impl NameBindingMapper<'_> {
    fn map_assign(
        &self,
        assign: BlockPyAssign<CoreBlockPyExpr>,
    ) -> BlockPyStmt<CoreBlockPyExpr, ExprName> {
        if is_local_cell_init_assign(&assign) {
            return BlockPyStmt::Assign(assign);
        }
        rewrite_binding_assign_by_name(
            assign.target.id.to_string(),
            self.map_expr(assign.value),
            self.semantic,
            self,
            assign.target.node_index,
            assign.target.range,
        )
    }
}

fn collect_deleted_names_in_stmt(
    stmt: &BlockPyStmt<CoreBlockPyExpr, ExprName>,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
    names: &mut HashSet<String>,
) {
    match stmt {
        BlockPyStmt::Assign(assign)
            if semantic.has_local_def(assign.target.id.as_str())
                && is_deleted_sentinel_expr(&assign.value) =>
        {
            names.insert(assign.target.id.to_string());
        }
        BlockPyStmt::Expr(expr) => {
            if let Some(name) = store_cell_deleted_logical_name(expr, semantic, storage_layout) {
                names.insert(name);
            }
            if let Some(name) = del_deref_logical_name(expr, semantic, storage_layout) {
                names.insert(name);
            }
        }
        BlockPyStmt::Delete(_) => {}
        _ => {}
    }
}

fn rewrite_deleted_name_loads_in_stmt(
    stmt: &mut BlockPyStmt<CoreBlockPyExpr, ExprName>,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
    resolver: &NameBindingMapper<'_>,
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            rewrite_deleted_name_loads_in_expr(
                &mut assign.value,
                semantic,
                storage_layout,
                resolver,
                deleted_names,
                always_unbound_names,
            );
        }
        BlockPyStmt::Expr(expr) => rewrite_deleted_name_loads_in_expr(
            expr,
            semantic,
            storage_layout,
            resolver,
            deleted_names,
            always_unbound_names,
        ),
        BlockPyStmt::Delete(_) => {}
    }
}

fn rewrite_deleted_name_loads_in_term(
    term: &mut BlockPyTerm<CoreBlockPyExpr>,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
    resolver: &NameBindingMapper<'_>,
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => {
            rewrite_deleted_name_loads_in_expr(
                &mut if_term.test,
                semantic,
                storage_layout,
                resolver,
                deleted_names,
                always_unbound_names,
            );
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_deleted_name_loads_in_expr(
                &mut branch.index,
                semantic,
                storage_layout,
                resolver,
                deleted_names,
                always_unbound_names,
            );
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                rewrite_deleted_name_loads_in_expr(
                    exc,
                    semantic,
                    storage_layout,
                    resolver,
                    deleted_names,
                    always_unbound_names,
                );
            }
        }
        BlockPyTerm::Return(value) => rewrite_deleted_name_loads_in_expr(
            value,
            semantic,
            storage_layout,
            resolver,
            deleted_names,
            always_unbound_names,
        ),
    }
}

fn rewrite_raw_cell_loads_in_expr(
    expr: &mut CoreBlockPyExpr,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) {
    match expr {
        CoreBlockPyExpr::Name(name)
            if matches!(name.ctx, ast::ExprContext::Load)
                && matches!(
                    semantic.binding_kind(name.id.as_str()),
                    Some(BlockPyBindingKind::Cell(_))
                ) =>
        {
            *expr = rewrite_cell_name_load(name.clone(), semantic, resolver);
        }
        CoreBlockPyExpr::Op(operation) => {
            if let OperationDetail::Call(call) = operation {
                if call.keywords.is_empty()
                    && call.args.len() == 3
                    && raw_load_name(call.func.as_ref())
                        .as_ref()
                        .is_some_and(|name| name == "class_lookup_cell")
                {
                    rewrite_raw_cell_loads_in_expr(call.func.as_mut(), semantic, resolver);
                    if let Some(arg) = call.args.get_mut(0) {
                        rewrite_raw_cell_loads_in_expr(arg.expr_mut(), semantic, resolver);
                    }
                    if let Some(arg) = call.args.get_mut(1) {
                        rewrite_raw_cell_loads_in_expr(arg.expr_mut(), semantic, resolver);
                    }
                    return;
                }
            }
            operation
                .walk_args_mut(&mut |arg| rewrite_raw_cell_loads_in_expr(arg, semantic, resolver));
        }
        CoreBlockPyExpr::Literal(_) => {}
        CoreBlockPyExpr::Name(_) => {}
    }
}

fn rewrite_raw_cell_loads_in_stmt(
    stmt: &mut BlockPyStmt<CoreBlockPyExpr, ExprName>,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            if is_local_cell_init_assign(assign) {
                return;
            }
            rewrite_raw_cell_loads_in_expr(&mut assign.value, semantic, resolver)
        }
        BlockPyStmt::Expr(expr) => rewrite_raw_cell_loads_in_expr(expr, semantic, resolver),
        BlockPyStmt::Delete(_) => {}
    }
}

fn rewrite_raw_cell_loads_in_term(
    term: &mut BlockPyTerm<CoreBlockPyExpr>,
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => {
            rewrite_raw_cell_loads_in_expr(&mut if_term.test, semantic, resolver);
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_raw_cell_loads_in_expr(&mut branch.index, semantic, resolver);
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                rewrite_raw_cell_loads_in_expr(exc, semantic, resolver);
            }
        }
        BlockPyTerm::Return(value) => rewrite_raw_cell_loads_in_expr(value, semantic, resolver),
    }
}

fn normal_successor_labels(
    term: &BlockPyTerm<CoreBlockPyExpr>,
) -> Vec<&crate::block_py::BlockPyLabel> {
    match term {
        BlockPyTerm::Jump(edge) => vec![&edge.target],
        BlockPyTerm::IfTerm(if_term) => vec![&if_term.then_label, &if_term.else_label],
        BlockPyTerm::BranchTable(branch) => {
            let mut targets = branch.targets.iter().collect::<Vec<_>>();
            targets.push(&branch.default_label);
            targets
        }
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => Vec::new(),
    }
}

fn normal_predecessor_exc_param_names(
    blocks: &[crate::block_py::CfgBlock<
        <CoreBlockPyPass as crate::block_py::BlockPyPass>::Stmt,
        crate::block_py::BlockPyTerm<CoreBlockPyExpr>,
    >],
) -> HashMap<crate::block_py::BlockPyLabel, Vec<Option<String>>> {
    let mut predecessors = HashMap::new();
    for block in blocks {
        let exc_name = block.exception_param().map(ToString::to_string);
        for target in normal_successor_labels(&block.term) {
            predecessors
                .entry(target.clone())
                .or_insert_with(Vec::new)
                .push(exc_name.clone());
        }
    }
    predecessors
}

fn sync_exception_param_cell_in_block(
    block: &mut crate::block_py::CfgBlock<
        <CoreBlockPyPass as crate::block_py::BlockPyPass>::Stmt,
        crate::block_py::BlockPyTerm<CoreBlockPyExpr>,
    >,
    normal_predecessor_exc_names: &[Option<String>],
    semantic: &BlockPyCallableSemanticInfo,
    resolver: &NameBindingMapper<'_>,
) {
    let Some(exc_name) = block.exception_param() else {
        return;
    };
    if !matches!(
        semantic.binding_kind(exc_name),
        Some(BlockPyBindingKind::Cell(_))
    ) {
        return;
    }
    if normal_predecessor_exc_names.iter().any(|pred_exc_name| {
        pred_exc_name
            .as_deref()
            .is_some_and(|pred_exc_name| pred_exc_name != exc_name)
    }) {
        return;
    }

    let node_index = compat_node_index();
    let range = compat_range();
    let exc_load = ExprName {
        id: exc_name.into(),
        ctx: ast::ExprContext::Load,
        node_index: node_index.clone(),
        range,
    };
    let sync_stmt = op_stmt(
        OperationDetail::from(StoreLocation::new(
            NameLocation::Cell(resolver.resolve_raw_cell_location(
                cell_storage_name_for_name(exc_name, semantic).as_str(),
            )),
            Box::new(rewrite_local_name_load(exc_load, resolver)),
        ))
        .with_meta(crate::block_py::Meta::new(node_index, range)),
    );
    block.body.insert(0, sync_stmt);
}

fn collect_deleted_names_in_blocks(
    blocks: &[crate::block_py::CfgBlock<
        <CoreBlockPyPass as crate::block_py::BlockPyPass>::Stmt,
        crate::block_py::BlockPyTerm<CoreBlockPyExpr>,
    >],
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            collect_deleted_names_in_stmt(stmt, semantic, storage_layout, &mut names);
        }
    }
    names
}

fn collect_runtime_bound_local_names_in_stmt(
    stmt: &BlockPyStmt<CoreBlockPyExpr, ExprName>,
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
    names: &mut HashSet<String>,
) {
    match stmt {
        BlockPyStmt::Assign(assign)
            if semantic.has_local_def(assign.target.id.as_str())
                && !is_deleted_sentinel_expr(&assign.value) =>
        {
            names.insert(assign.target.id.to_string());
        }
        BlockPyStmt::Expr(expr) => {
            if let Some(name) = store_cell_runtime_logical_name(expr, semantic, storage_layout) {
                names.insert(name);
            }
        }
        BlockPyStmt::Delete(_) => {}
        _ => {}
    }
}

fn collect_runtime_bound_local_names(
    blocks: &[crate::block_py::CfgBlock<
        <CoreBlockPyPass as crate::block_py::BlockPyPass>::Stmt,
        crate::block_py::BlockPyTerm<CoreBlockPyExpr>,
    >],
    semantic: &BlockPyCallableSemanticInfo,
    storage_layout: &StorageLayout,
) -> HashSet<String> {
    let mut names = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            collect_runtime_bound_local_names_in_stmt(stmt, semantic, storage_layout, &mut names);
        }
    }
    names
}

fn collect_always_unbound_local_names(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashSet<String> {
    let semantic = &callable.semantic;
    let storage_layout = callable
        .storage_layout
        .as_ref()
        .expect("name binding should have storage layout before local-name analysis");
    let param_names = callable.params.names().into_iter().collect::<HashSet<_>>();
    let runtime_bound_names =
        collect_runtime_bound_local_names(&callable.blocks, semantic, storage_layout);
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
        CoreBlockPyExpr::Op(operation) => {
            match operation {
                OperationDetail::LoadName(op) => {
                    names.insert(op.name.clone());
                }
                _ => {}
            }
            operation.walk_args(&mut |arg| collect_remaining_names_in_expr(arg, names));
        }
    }
}

fn collect_remaining_names_in_stmt(
    stmt: &BlockPyStmt<CoreBlockPyExpr, ExprName>,
    names: &mut HashSet<String>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            names.insert(assign.target.id.to_string());
            collect_remaining_names_in_expr(&assign.value, names);
        }
        BlockPyStmt::Expr(expr) => collect_remaining_names_in_expr(expr, names),
        BlockPyStmt::Delete(delete) => {
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

fn collect_captured_cell_slot_locations(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashMap<String, u32> {
    let mut slots = HashMap::new();
    if let Some(layout) = callable.storage_layout.as_ref() {
        for (slot, closure_slot) in layout.freevars.iter().enumerate() {
            slots.insert(closure_slot.storage_name.clone(), slot as u32);
            slots.insert(closure_slot.logical_name.clone(), slot as u32);
        }
    }
    slots
}

fn collect_owned_cell_storage_bindings(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> Vec<(String, String)> {
    if let Some(layout) = callable
        .storage_layout
        .as_ref()
        .filter(|layout| !layout.cellvars.is_empty() || !layout.runtime_cells.is_empty())
    {
        return layout
            .cellvars
            .iter()
            .chain(layout.runtime_cells.iter())
            .map(|slot| (slot.logical_name.clone(), slot.storage_name.clone()))
            .collect();
    }

    let mut storage_names = callable
        .semantic
        .owned_cell_storage_names()
        .into_iter()
        .collect::<Vec<_>>();
    storage_names.sort();
    storage_names
        .into_iter()
        .map(|storage_name| {
            let logical_name = callable
                .semantic
                .logical_name_for_cell_storage(storage_name.as_str())
                .unwrap_or_else(|| storage_name.clone());
            (logical_name, storage_name)
        })
        .collect()
}

fn collect_owned_cell_slot_locations(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashMap<String, u32> {
    let mut slots = HashMap::new();
    for (slot, (logical_name, storage_name)) in collect_owned_cell_storage_bindings(callable)
        .into_iter()
        .enumerate()
    {
        slots.insert(storage_name, slot as u32);
        slots.insert(logical_name, slot as u32);
    }
    slots
}

fn collect_cell_bindings(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashMap<String, (String, BlockPyCellBindingKind)> {
    let mut bindings = HashMap::new();
    let Some(layout) = callable.storage_layout.as_ref() else {
        return bindings;
    };

    let mut add_binding = |name: &str, storage_name: &str, binding_kind: BlockPyCellBindingKind| {
        bindings.insert(name.to_string(), (storage_name.to_string(), binding_kind));
    };

    for slot in &layout.freevars {
        add_binding(
            slot.logical_name.as_str(),
            slot.storage_name.as_str(),
            BlockPyCellBindingKind::Capture,
        );
        add_binding(
            slot.storage_name.as_str(),
            slot.storage_name.as_str(),
            BlockPyCellBindingKind::Capture,
        );
        let capture_source_name = callable
            .semantic
            .cell_capture_source_name(slot.logical_name.as_str());
        add_binding(
            capture_source_name.as_str(),
            slot.storage_name.as_str(),
            BlockPyCellBindingKind::Capture,
        );
    }

    for (logical_name, storage_name) in collect_owned_cell_storage_bindings(callable) {
        add_binding(
            logical_name.as_str(),
            storage_name.as_str(),
            BlockPyCellBindingKind::Owner,
        );
        add_binding(
            storage_name.as_str(),
            storage_name.as_str(),
            BlockPyCellBindingKind::Owner,
        );
    }

    bindings
}

fn is_remaining_local_name(
    name: &str,
    semantic: &BlockPyCallableSemanticInfo,
    has_explicit_store: bool,
) -> bool {
    if resolve_cell_storage_name(semantic, name).is_some() {
        return false;
    }
    if has_explicit_store {
        return !matches!(
            semantic.binding_kind(name),
            Some(BlockPyBindingKind::Cell(_)) | Some(BlockPyBindingKind::Global)
        ) && matches!(
            semantic.binding_target_for_name(name, BlockPyBindingPurpose::Store),
            BindingTarget::Local
        );
    }
    match semantic.binding_kind(name) {
        Some(BlockPyBindingKind::Local) => semantic.honors_internal_binding(name),
        Some(BlockPyBindingKind::Cell(_)) | Some(BlockPyBindingKind::Global) => false,
        None => semantic.has_local_def(name),
    }
}

fn compute_local_slot_locations_from_analysis(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashMap<String, u32> {
    let mut slots = HashMap::new();
    for (slot, param_name) in callable.params.names().into_iter().enumerate() {
        slots.insert(param_name, slot as u32);
    }
    let mut next_slot = slots.len() as u32;
    let mut owned_cell_storage_names = callable
        .semantic
        .owned_cell_storage_names()
        .into_iter()
        .collect::<Vec<_>>();
    owned_cell_storage_names.sort();
    for storage_name in owned_cell_storage_names {
        if slots.contains_key(storage_name.as_str()) {
            continue;
        }
        slots.insert(storage_name, next_slot);
        next_slot += 1;
    }
    for block in &callable.blocks {
        for param_name in block.param_names() {
            if slots.contains_key(param_name) {
                continue;
            }
            slots.insert(param_name.to_string(), next_slot);
            next_slot += 1;
        }
    }

    let mut remaining = HashSet::new();
    let mut explicitly_stored = HashSet::new();
    for block in &callable.blocks {
        for stmt in &block.body {
            collect_remaining_names_in_stmt(stmt, &mut remaining);
            match stmt {
                BlockPyStmt::Assign(assign) => {
                    explicitly_stored.insert(assign.target.id.to_string());
                }
                BlockPyStmt::Delete(delete) => {
                    explicitly_stored.insert(delete.target.id.to_string());
                }
                BlockPyStmt::Expr(_) => {}
            }
        }
        collect_remaining_names_in_term(&block.term, &mut remaining);
    }

    let mut non_param_locals = remaining
        .into_iter()
        .filter(|name| !slots.contains_key(name))
        .filter(|name| {
            is_remaining_local_name(
                name,
                &callable.semantic,
                explicitly_stored.contains(name.as_str()),
            )
        })
        .collect::<Vec<_>>();
    non_param_locals.sort();

    for name in non_param_locals {
        slots.insert(name, next_slot);
        next_slot += 1;
    }
    slots
}

fn ordered_slot_names_from_local_slots(local_slots: HashMap<String, u32>) -> Vec<String> {
    let mut slots = local_slots.into_iter().collect::<Vec<_>>();
    slots.sort_by_key(|(_, slot)| *slot);
    slots.into_iter().map(|(name, _)| name).collect()
}

fn collect_local_slot_locations(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> HashMap<String, u32> {
    if let Some(layout) = callable
        .storage_layout
        .as_ref()
        .filter(|layout| !layout.stack_slots().is_empty())
    {
        return layout
            .stack_slots()
            .iter()
            .enumerate()
            .map(|(slot, name)| (name.clone(), slot as u32))
            .collect();
    }

    compute_local_slot_locations_from_analysis(callable)
}

fn populate_stack_slots_in_storage_layout<P: crate::block_py::BlockPyPass>(
    callable: &mut BlockPyFunction<P>,
    local_slots: HashMap<String, u32>,
) {
    let stack_slots = ordered_slot_names_from_local_slots(local_slots);
    callable
        .storage_layout
        .get_or_insert_with(StorageLayout::default)
        .set_stack_slots(stack_slots);
}

fn ensure_storage_layout_covers_block_params<P: crate::block_py::BlockPyPass>(
    callable: &mut BlockPyFunction<P>,
) {
    let Some(layout) = callable.storage_layout.as_mut() else {
        return;
    };
    for block in &callable.blocks {
        for name in block.param_names() {
            layout.ensure_stack_slot(name.to_string());
        }
    }
}

struct NameLocator<'a> {
    semantic: &'a BlockPyCallableSemanticInfo,
    exception_param_names: HashSet<String>,
    local_slots: HashMap<String, u32>,
    captured_cell_slots: HashMap<String, u32>,
    owned_cell_slots: HashMap<String, u32>,
    cell_bindings: HashMap<String, (String, BlockPyCellBindingKind)>,
}

impl NameLocator<'_> {
    fn resolve_raw_cell_location(&self, name_text: &str) -> CellLocation {
        if let Some(storage_name) =
            resolve_captured_cell_source_storage_name(self.semantic, name_text)
        {
            let slot = self
                .captured_cell_slots
                .get(storage_name.as_str())
                .copied()
                .unwrap_or_else(|| {
                    panic!(
                        "missing closure slot for captured raw cell source {name_text} via storage name {storage_name}"
                    )
                });
            return CellLocation::CapturedSource(slot);
        }

        if let Some((storage_name, binding_kind)) = self.cell_bindings.get(name_text) {
            return match binding_kind {
                BlockPyCellBindingKind::Owner => {
                    let slot = self
                        .owned_cell_slots
                        .get(storage_name.as_str())
                        .copied()
                        .unwrap_or_else(|| {
                            panic!(
                                "missing owned cell slot for raw cell target {name_text} via storage name {storage_name}"
                            )
                        });
                    CellLocation::Owned(slot)
                }
                BlockPyCellBindingKind::Capture => {
                    let slot = self
                        .captured_cell_slots
                        .get(storage_name.as_str())
                        .copied()
                        .unwrap_or_else(|| {
                            panic!(
                                "missing closure slot for raw captured cell target {name_text} via storage name {storage_name}"
                            )
                        });
                    CellLocation::CapturedSource(slot)
                }
            };
        }

        panic!("raw cell target {name_text} did not resolve to a cell-backed location");
    }

    fn resolve_cell_ref_location(&self, logical_name: &str) -> CellLocation {
        let source_name = self.semantic.cell_ref_source_name(logical_name);
        self.resolve_raw_cell_location(source_name.as_str())
    }

    fn locate_name(&self, name: ExprName) -> LocatedName {
        let name_text = name.id.to_string();
        let location = if self.exception_param_names.contains(name_text.as_str()) {
            let slot = self
                .local_slots
                .get(name_text.as_str())
                .copied()
                .unwrap_or_else(|| {
                    panic!("missing local slot for exception param {name_text}");
                });
            NameLocation::local(slot)
        } else if let Some(storage_name) =
            resolve_captured_cell_source_storage_name(self.semantic, name_text.as_str())
        {
            let slot = self
                .captured_cell_slots
                .get(storage_name.as_str())
                .copied()
                .unwrap_or_else(|| {
                panic!(
                    "missing closure slot for captured cell source {name_text} via storage name {storage_name}"
                )
            });
            NameLocation::captured_source_cell(slot)
        } else if let Some((storage_name, binding_kind)) =
            self.cell_bindings.get(name_text.as_str()).cloned()
        {
            match binding_kind {
                BlockPyCellBindingKind::Owner => {
                    if name_text != storage_name {
                        if let Some(slot) = self.local_slots.get(name_text.as_str()).copied() {
                            NameLocation::local(slot)
                        } else {
                            let slot = self
                                .owned_cell_slots
                                .get(storage_name.as_str())
                                .copied()
                                .unwrap_or_else(|| {
                                    panic!(
                                        "missing owned cell slot for storage name {storage_name} while locating {name_text}"
                                    )
                                });
                            NameLocation::owned_cell(slot)
                        }
                    } else {
                        let slot = self
                            .owned_cell_slots
                            .get(storage_name.as_str())
                            .copied()
                            .unwrap_or_else(|| {
                                panic!(
                                    "missing owned cell slot for storage name {storage_name} while locating {name_text}"
                                )
                            });
                        NameLocation::owned_cell(slot)
                    }
                }
                BlockPyCellBindingKind::Capture => {
                    let slot = self
                        .captured_cell_slots
                        .get(storage_name.as_str())
                        .copied()
                        .unwrap_or_else(|| {
                            panic!(
                                "missing closure slot for storage name {storage_name} while locating {name_text}"
                            )
                        });
                    NameLocation::closure_cell(slot)
                }
            }
        } else if let Some(slot) = self.local_slots.get(name_text.as_str()).copied() {
            NameLocation::local(slot)
        } else {
            NameLocation::Global
        };
        LocatedName::from(name).with_location(location)
    }

    fn mark_raw_cell_name(&self, name: LocatedName) -> LocatedName {
        let name_text = name.id.to_string();
        if name.location.is_global() {
            let location = self.resolve_raw_cell_location(name_text.as_str());
            return name.with_location(NameLocation::Cell(location));
        }

        match name.cell_location() {
            Some(location) if location.is_closure() => {
                name.with_location(NameLocation::captured_source_cell(location.slot()))
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

impl BlockPyModuleMap<CoreBlockPyPass, ResolvedStorageBlockPyPass> for NameLocator<'_> {
    fn map_name(&self, name: ExprName) -> LocatedName {
        self.locate_name(name)
    }

    fn map_expr(&self, expr: CoreBlockPyExpr) -> CoreBlockPyExpr<LocatedName> {
        match expr {
            CoreBlockPyExpr::Name(name) => CoreBlockPyExpr::Name(self.locate_name(name)),
            CoreBlockPyExpr::Literal(literal) => CoreBlockPyExpr::Literal(literal),
            CoreBlockPyExpr::Op(operation) => {
                let mut expr = self.map_nested_expr(CoreBlockPyExpr::Op(operation));
                let operation = operation_expr_mut(&mut expr)
                    .expect("op expression should remain op after nested mapping");
                let meta = operation.meta();
                if let OperationDetail::CellRefForName(op) = operation {
                    let location = self.resolve_cell_ref_location(op.logical_name.as_str());
                    *operation = OperationDetail::from(CellRef::new(location)).with_meta(meta);
                    return expr;
                }
                if let OperationDetail::Call(call) = operation {
                    if raw_load_name(call.func.as_ref())
                        .as_ref()
                        .is_some_and(|name| name == "class_lookup_cell")
                        && call.args.len() == 3
                    {
                        if let Some(CoreBlockPyCallArg::Positional(expr)) = call.args.get_mut(2) {
                            *expr = self.mark_raw_cell_expr(expr.clone());
                        }
                    }
                }
                expr
            }
        }
    }
}

fn locate_names_in_callable(
    callable: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<ResolvedStorageBlockPyPass> {
    let semantic = callable.semantic.clone();
    let exception_param_names = callable
        .blocks
        .iter()
        .filter_map(|block| block.exception_param().map(ToString::to_string))
        .collect::<HashSet<_>>();
    let local_slots = collect_local_slot_locations(&callable);
    let captured_cell_slots = collect_captured_cell_slot_locations(&callable);
    let owned_cell_slots = collect_owned_cell_slot_locations(&callable);
    let cell_bindings = collect_cell_bindings(&callable);
    NameLocator {
        semantic: &semantic,
        exception_param_names,
        local_slots,
        captured_cell_slots,
        owned_cell_slots,
        cell_bindings,
    }
    .map_fn(callable)
}

fn collect_make_function_callee_ids_in_expr(expr: &CoreBlockPyExpr, out: &mut Vec<FunctionId>) {
    match expr {
        CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => {}
        CoreBlockPyExpr::Op(operation) => {
            if let OperationDetail::MakeFunction(op) = operation {
                out.push(op.function_id);
                return;
            }
            operation.walk_args(&mut |arg| collect_make_function_callee_ids_in_expr(arg, out));
        }
    }
}

fn collect_make_function_callee_ids(
    callable: &BlockPyFunction<CoreBlockPyPass>,
) -> Vec<FunctionId> {
    let mut out = Vec::new();
    for block in &callable.blocks {
        for stmt in &block.body {
            collect_make_function_callee_ids_in_stmt(stmt, &mut out);
        }
        collect_make_function_callee_ids_in_term(&block.term, &mut out);
    }
    out.sort_by_key(|id| id.0);
    out.dedup();
    out
}

fn collect_make_function_callee_ids_in_stmt(
    stmt: &BlockPyStmt<CoreBlockPyExpr, ExprName>,
    out: &mut Vec<FunctionId>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            collect_make_function_callee_ids_in_expr(&assign.value, out);
        }
        BlockPyStmt::Expr(expr) => collect_make_function_callee_ids_in_expr(expr, out),
        BlockPyStmt::Delete(_) => {}
    }
}

fn collect_make_function_callee_ids_in_term(
    term: &BlockPyTerm<CoreBlockPyExpr>,
    out: &mut Vec<FunctionId>,
) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => {
            collect_make_function_callee_ids_in_expr(&if_term.test, out)
        }
        BlockPyTerm::BranchTable(branch) => {
            collect_make_function_callee_ids_in_expr(&branch.index, out)
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                collect_make_function_callee_ids_in_expr(exc, out);
            }
        }
        BlockPyTerm::Return(expr) => collect_make_function_callee_ids_in_expr(expr, out),
    }
}

fn compute_callable_storage_layout_for_name_binding(
    function_id: FunctionId,
    callable_by_id: &HashMap<FunctionId, &BlockPyFunction<CoreBlockPyPass>>,
    make_function_callees: &HashMap<FunctionId, Vec<FunctionId>>,
    memo: &mut HashMap<FunctionId, Option<StorageLayout>>,
    visiting: &mut HashSet<FunctionId>,
) -> Option<StorageLayout> {
    if let Some(layout) = memo.get(&function_id) {
        return layout.clone();
    }
    let callable = callable_by_id
        .get(&function_id)
        .unwrap_or_else(|| panic!("missing callable for function id {:?}", function_id));
    if let Some(layout) = callable.storage_layout.clone() {
        memo.insert(function_id, Some(layout.clone()));
        return Some(layout);
    }
    if !visiting.insert(function_id) {
        return compute_storage_layout_from_semantics(callable);
    }

    let base_layout = compute_storage_layout_from_semantics(callable);
    let mut capture_names = base_layout
        .as_ref()
        .map(|layout| {
            layout
                .freevars
                .iter()
                .map(|slot| slot.logical_name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let base_cellvar_names = base_layout
        .as_ref()
        .map(|layout| {
            layout
                .cellvars
                .iter()
                .map(|slot| slot.logical_name.clone())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let base_cellvar_storage_names = base_layout
        .as_ref()
        .map(|layout| {
            layout
                .cellvars
                .iter()
                .map(|slot| slot.storage_name.clone())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if let Some(callee_ids) = make_function_callees.get(&function_id) {
        for callee_id in callee_ids {
            let Some(callee_layout) = compute_callable_storage_layout_for_name_binding(
                *callee_id,
                callable_by_id,
                make_function_callees,
                memo,
                visiting,
            ) else {
                continue;
            };
            for slot in &callee_layout.freevars {
                let capture_source_name = callable
                    .semantic
                    .cell_capture_source_name(slot.logical_name.as_str());
                if base_cellvar_names.contains(slot.logical_name.as_str())
                    || base_cellvar_storage_names.contains(capture_source_name.as_str())
                {
                    continue;
                }
                capture_names.push(slot.logical_name.clone());
            }
        }
    }
    visiting.remove(&function_id);

    let param_name_set = callable.params.names().into_iter().collect::<HashSet<_>>();
    let mut local_cell_slots = callable
        .semantic
        .owned_cell_storage_names()
        .into_iter()
        .collect::<Vec<_>>();
    local_cell_slots.sort();
    let layout = build_storage_layout_from_capture_names(
        callable,
        capture_names,
        &param_name_set,
        &local_cell_slots,
    );
    memo.insert(function_id, layout.clone());
    layout
}

fn ensure_module_storage_layouts(
    callable_defs: Vec<BlockPyFunction<CoreBlockPyPass>>,
) -> Vec<BlockPyFunction<CoreBlockPyPass>> {
    let computed_layouts = {
        let callable_by_id = callable_defs
            .iter()
            .map(|callable| (callable.function_id, callable))
            .collect::<HashMap<_, _>>();
        let make_function_callees = callable_defs
            .iter()
            .map(|callable| {
                (
                    callable.function_id,
                    collect_make_function_callee_ids(callable),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut memo = HashMap::new();
        let mut visiting = HashSet::new();
        for function_id in callable_by_id.keys().copied().collect::<Vec<_>>() {
            compute_callable_storage_layout_for_name_binding(
                function_id,
                &callable_by_id,
                &make_function_callees,
                &mut memo,
                &mut visiting,
            );
        }
        memo
    };

    callable_defs
        .into_iter()
        .map(|mut callable| {
            if callable.storage_layout.is_none() {
                callable.storage_layout = computed_layouts
                    .get(&callable.function_id)
                    .cloned()
                    .flatten();
            }
            callable
        })
        .collect()
}

fn compute_module_make_function_capture_names(
    callable_defs: &[BlockPyFunction<CoreBlockPyPass>],
) -> HashMap<FunctionId, Vec<String>> {
    callable_defs
        .iter()
        .map(|callable| {
            let capture_names = callable
                .storage_layout
                .as_ref()
                .map(|layout| {
                    layout
                        .freevars
                        .iter()
                        .map(|slot| slot.logical_name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (callable.function_id, capture_names)
        })
        .collect()
}

fn refresh_bb_callable_block_params(
    callable: BlockPyFunction<ResolvedStorageBlockPyPass>,
) -> BlockPyFunction<ResolvedStorageBlockPyPass> {
    let BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        storage_layout,
        semantic,
    } = callable;
    let mut blocks = blocks
        .into_iter()
        .map(|block| {
            let params = block.bb_params().cloned().collect();
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
    populate_jump_edge_args(&mut blocks);
    BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        storage_layout,
        semantic,
    }
}

fn populate_jump_edge_args(
    blocks: &mut [crate::block_py::CfgBlock<
        BlockPyStmt<crate::block_py::LocatedCoreBlockPyExpr, LocatedName>,
        BlockPyTerm<crate::block_py::LocatedCoreBlockPyExpr>,
    >],
) {
    let label_to_index = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label, index))
        .collect::<HashMap<_, _>>();
    for block_index in 0..blocks.len() {
        let BlockPyTerm::Jump(edge) = &blocks[block_index].term else {
            continue;
        };
        let Some(target_index) = label_to_index.get(&edge.target).copied() else {
            continue;
        };
        let target_params = blocks[target_index].params.clone();
        if target_params.is_empty() {
            continue;
        }
        let source_params = blocks[block_index].params.clone();
        let explicit_args = edge.args.clone();
        let explicit_start = target_params.len().saturating_sub(explicit_args.len());
        let new_args = target_params
            .iter()
            .enumerate()
            .map(|(param_index, target_param)| {
                if param_index >= explicit_start {
                    return explicit_args[param_index - explicit_start].clone();
                }
                if source_params
                    .iter()
                    .any(|source_param| source_param.name == target_param.name)
                {
                    return BlockArg::Name(target_param.name.clone());
                }
                if let Some(source_same_role) = source_params
                    .iter()
                    .find(|source_param| source_param.role == target_param.role)
                {
                    return BlockArg::Name(source_same_role.name.clone());
                }
                BlockArg::None
            })
            .collect::<Vec<_>>();
        if let BlockPyTerm::Jump(edge) = &mut blocks[block_index].term {
            edge.args = new_args;
        }
    }
}

fn lower_name_binding_callable(
    callable: BlockPyFunction<CoreBlockPyPass>,
    callee_make_function_capture_names: &HashMap<crate::block_py::FunctionId, Vec<String>>,
) -> BlockPyFunction<ResolvedStorageBlockPyPass> {
    let semantic = callable.semantic.clone();
    let local_slots = collect_local_slot_locations(&callable);
    let captured_cell_slots = collect_captured_cell_slot_locations(&callable);
    let owned_cell_slots = collect_owned_cell_slot_locations(&callable);
    let cell_bindings = collect_cell_bindings(&callable);
    let mapper = NameBindingMapper {
        semantic: &semantic,
        callee_make_function_capture_names,
        local_slots: local_slots.clone(),
        captured_cell_slots,
        owned_cell_slots,
        cell_bindings,
    };
    let mut lowered = mapper.map_fn(callable);
    prepend_owned_cell_init_preamble(&mut lowered);
    populate_stack_slots_in_storage_layout(&mut lowered, local_slots);
    let storage_layout = lowered
        .storage_layout
        .as_ref()
        .expect("name binding should have storage layout before cell-location analysis");
    let deleted_names = collect_deleted_names_in_blocks(&lowered.blocks, &semantic, storage_layout);
    let always_unbound_names = collect_always_unbound_local_names(&lowered);
    if !deleted_names.is_empty() || !always_unbound_names.is_empty() {
        for block in &mut lowered.blocks {
            for stmt in &mut block.body {
                rewrite_deleted_name_loads_in_stmt(
                    stmt,
                    &semantic,
                    storage_layout,
                    &mapper,
                    &deleted_names,
                    &always_unbound_names,
                );
            }
            rewrite_deleted_name_loads_in_term(
                &mut block.term,
                &semantic,
                storage_layout,
                &mapper,
                &deleted_names,
                &always_unbound_names,
            );
        }
    }
    rewrite_current_exception_in_core_blocks(&mut lowered.blocks);
    let normal_predecessors = normal_predecessor_exc_param_names(&lowered.blocks);
    for block in &mut lowered.blocks {
        let predecessor_exc_names = normal_predecessors
            .get(&block.label)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        sync_exception_param_cell_in_block(block, predecessor_exc_names, &semantic, &mapper);
        for stmt in &mut block.body {
            rewrite_raw_cell_loads_in_stmt(stmt, &semantic, &mapper);
        }
        rewrite_raw_cell_loads_in_term(&mut block.term, &semantic, &mapper);
    }
    let mut lowered = refresh_bb_callable_block_params(locate_names_in_callable(lowered));
    ensure_storage_layout_covers_block_params(&mut lowered);
    lowered
}

pub(crate) fn lower_name_binding_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPass>,
) -> BlockPyModule<ResolvedStorageBlockPyPass> {
    let callable_defs = ensure_module_storage_layouts(module.callable_defs);
    let callee_make_function_capture_names =
        compute_module_make_function_capture_names(&callable_defs);
    BlockPyModule {
        callable_defs: callable_defs
            .into_iter()
            .map(|callable| {
                lower_name_binding_callable(callable, &callee_make_function_capture_names)
            })
            .collect(),
    }
}

use crate::block_py::intrinsics::{
    Intrinsic, DEL_DEREF_INTRINSIC, DEL_DEREF_QUIETLY_INTRINSIC, DEL_QUIETLY_INTRINSIC,
    LOAD_CELL_INTRINSIC, LOAD_GLOBAL_INTRINSIC, MAKE_CELL_INTRINSIC, STORE_CELL_INTRINSIC,
    STORE_GLOBAL_INTRINSIC,
};
use crate::block_py::{
    core_positional_call_expr_with_meta, core_positional_intrinsic_expr_with_meta, BindingTarget,
    BlockPyAssign, BlockPyBindingKind, BlockPyBindingPurpose, BlockPyCallableScopeKind,
    BlockPyCallableSemanticInfo, BlockPyClassBodyFallback, BlockPyEffectiveBinding,
    BlockPyFunction, BlockPyFunctionKind, BlockPyIf, BlockPyModule, BlockPyModuleMap, BlockPyRaise,
    BlockPyStmt, BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExpr,
    CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreStringLiteral, IntrinsicCall,
};
use crate::passes::ast_to_ast::scope_helpers::cell_name;
use crate::passes::CoreBlockPyPass;
use ruff_python_ast::{self as ast, ExprName};
use std::collections::HashSet;

fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name.starts_with("__dp_") || name == "__dp__"
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
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> CoreBlockPyExpr {
    core_name_expr(
        cell_name(name).as_str(),
        ast::ExprContext::Load,
        node_index,
        range,
    )
}

fn rewrite_cell_name_load(name: ExprName) -> CoreBlockPyExpr {
    let node_index = name.node_index.clone();
    let range = name.range;
    core_positional_intrinsic_expr_with_meta(
        &LOAD_CELL_INTRINSIC,
        node_index.clone(),
        range,
        vec![cell_expr_for_name(name.id.as_str(), node_index, range)],
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
) -> BlockPyStmt<CoreBlockPyExpr> {
    let node_index = assign.target.node_index.clone();
    let range = assign.target.range;
    BlockPyStmt::Expr(core_positional_intrinsic_expr_with_meta(
        &STORE_CELL_INTRINSIC,
        node_index.clone(),
        range,
        vec![
            cell_expr_for_name(assign.target.id.as_str(), node_index, range),
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

fn rewrite_deleted_name_load_expr(
    name: ExprName,
    deleted_names: &HashSet<String>,
) -> CoreBlockPyExpr {
    if !deleted_names.contains(name.id.as_str()) {
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
            CoreBlockPyExpr::Name(name),
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

fn rewrite_deleted_name_loads_in_expr(expr: &mut CoreBlockPyExpr, deleted_names: &HashSet<String>) {
    if let Some(logical_name) = cell_load_logical_name(expr) {
        if deleted_names.contains(logical_name.as_str()) {
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
            *expr = rewrite_deleted_name_load_expr(name.clone(), deleted_names);
        }
        CoreBlockPyExpr::Call(CoreBlockPyCall {
            func,
            args,
            keywords,
            ..
        }) => {
            rewrite_deleted_name_loads_in_expr(func.as_mut(), deleted_names);
            for arg in args {
                match arg {
                    CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                        rewrite_deleted_name_loads_in_expr(value, deleted_names);
                    }
                }
            }
            for keyword in keywords {
                match keyword {
                    CoreBlockPyKeywordArg::Named { value, .. }
                    | CoreBlockPyKeywordArg::Starred(value) => {
                        rewrite_deleted_name_loads_in_expr(value, deleted_names);
                    }
                }
            }
        }
        CoreBlockPyExpr::Intrinsic(IntrinsicCall { args, keywords, .. }) => {
            for arg in args {
                match arg {
                    CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                        rewrite_deleted_name_loads_in_expr(value, deleted_names);
                    }
                }
            }
            for keyword in keywords {
                match keyword {
                    CoreBlockPyKeywordArg::Named { value, .. }
                    | CoreBlockPyKeywordArg::Starred(value) => {
                        rewrite_deleted_name_loads_in_expr(value, deleted_names);
                    }
                }
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
    CoreBlockPyExpr::Name(ast::ExprName {
        id: id.into(),
        ctx,
        node_index,
        range,
    })
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

fn rewrite_class_name_load_cell(name: ExprName) -> CoreBlockPyExpr {
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
            cell_expr_for_name(name.id.as_str(), node_index, range),
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
                vec![cell_expr_for_name(name.id.as_str(), node_index, range)],
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

fn cell_load_logical_name(expr: &CoreBlockPyExpr) -> Option<String> {
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic,
        args,
        keywords,
        ..
    }) = expr
    else {
        return None;
    };
    if intrinsic.name() != LOAD_CELL_INTRINSIC.name() || !keywords.is_empty() || args.len() != 1 {
        return None;
    }
    let CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(name)) = &args[0] else {
        return None;
    };
    name.id
        .as_str()
        .strip_prefix("_dp_cell_")
        .map(str::to_string)
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

fn prepend_owned_cell_init_preamble(callable: &mut BlockPyFunction<CoreBlockPyPass>) {
    if callable.kind != BlockPyFunctionKind::Function || callable.names.fn_name == "_dp_resume" {
        return;
    }
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
    let init_stmts = storage_names
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
        .collect::<Vec<_>>();
    callable
        .blocks
        .first_mut()
        .expect("BlockPyFunction should have at least one block")
        .body
        .splice(0..0, init_stmts);
}

fn store_cell_deleted_logical_name(expr: &CoreBlockPyExpr) -> Option<String> {
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic,
        args,
        keywords,
        ..
    }) = expr
    else {
        return None;
    };
    if intrinsic.name() != STORE_CELL_INTRINSIC.name() || !keywords.is_empty() || args.len() != 2 {
        return None;
    }
    let CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(name)) = &args[0] else {
        return None;
    };
    let CoreBlockPyCallArg::Positional(value_expr) = &args[1] else {
        return None;
    };
    if !is_deleted_sentinel_expr(value_expr) {
        return None;
    }
    name.id
        .as_str()
        .strip_prefix("_dp_cell_")
        .map(str::to_string)
}

fn is_local_cell_init_assign(assign: &BlockPyAssign<CoreBlockPyExpr>) -> bool {
    let Some(logical_name) = assign.target.id.as_str().strip_prefix("_dp_cell_") else {
        return false;
    };
    let CoreBlockPyExpr::Intrinsic(IntrinsicCall {
        intrinsic,
        args,
        keywords,
        ..
    }) = &assign.value
    else {
        return false;
    };
    if !keywords.is_empty() || args.len() != 1 {
        return false;
    }
    if intrinsic.name() != MAKE_CELL_INTRINSIC.name() {
        return false;
    }
    matches!(
        &args[0],
        CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(name))
            if name.id.as_str() == logical_name
    )
}

struct NameBindingMapper<'a> {
    semantic: &'a BlockPyCallableSemanticInfo,
}

impl NameBindingMapper<'_> {
    fn rewrite_args(
        &self,
        args: Vec<CoreBlockPyCallArg<CoreBlockPyExpr>>,
    ) -> Vec<CoreBlockPyCallArg<CoreBlockPyExpr>> {
        args.into_iter()
            .map(|arg| match arg {
                CoreBlockPyCallArg::Positional(value) => {
                    CoreBlockPyCallArg::Positional(self.map_expr(value))
                }
                CoreBlockPyCallArg::Starred(value) => {
                    CoreBlockPyCallArg::Starred(self.map_expr(value))
                }
            })
            .collect()
    }

    fn rewrite_keywords(
        &self,
        keywords: Vec<CoreBlockPyKeywordArg<CoreBlockPyExpr>>,
    ) -> Vec<CoreBlockPyKeywordArg<CoreBlockPyExpr>> {
        keywords
            .into_iter()
            .map(|keyword| match keyword {
                CoreBlockPyKeywordArg::Named { arg, value } => CoreBlockPyKeywordArg::Named {
                    arg,
                    value: self.map_expr(value),
                },
                CoreBlockPyKeywordArg::Starred(value) => {
                    CoreBlockPyKeywordArg::Starred(self.map_expr(value))
                }
            })
            .collect()
    }
}

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
                vec![cell_expr_for_name(name.as_str(), node_index, range)],
            ));
        }
        return rewrite_cell_binding_assign(assign);
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
            BlockPyStmt::Delete(delete) => BlockPyStmt::Delete(delete),
            BlockPyStmt::If(if_stmt) => BlockPyStmt::If(crate::block_py::BlockPyIf {
                test: self.map_expr(if_stmt.test),
                body: self.map_fragment(if_stmt.body),
                orelse: self.map_fragment(if_stmt.orelse),
            }),
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
                if !is_internal_symbol(name.id.as_str())
                    && self.semantic.scope_kind == BlockPyCallableScopeKind::Class =>
            {
                match self
                    .semantic
                    .effective_binding(name.id.as_str(), BlockPyBindingPurpose::Load)
                {
                    Some(BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Cell)) => {
                        rewrite_class_name_load_cell(name)
                    }
                    Some(BlockPyEffectiveBinding::Cell(_)) => rewrite_cell_name_load(name),
                    Some(BlockPyEffectiveBinding::Global) => rewrite_global_name_load(name),
                    Some(BlockPyEffectiveBinding::Local) => CoreBlockPyExpr::Name(name),
                    Some(BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Global))
                    | None => rewrite_class_name_load_global(name),
                }
            }
            CoreBlockPyExpr::Name(name)
                if !is_internal_symbol(name.id.as_str())
                    && matches!(
                        self.semantic.resolved_load_binding_kind(name.id.as_str()),
                        BlockPyBindingKind::Cell(_)
                    ) =>
            {
                rewrite_cell_name_load(name)
            }
            CoreBlockPyExpr::Name(name)
                if !is_internal_symbol(name.id.as_str())
                    && self.semantic.resolved_load_binding_kind(name.id.as_str())
                        == BlockPyBindingKind::Global =>
            {
                rewrite_global_name_load(name)
            }
            CoreBlockPyExpr::Name(name) => CoreBlockPyExpr::Name(name),
            CoreBlockPyExpr::Literal(literal) => CoreBlockPyExpr::Literal(literal),
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
                    args: self.rewrite_args(args),
                    keywords: self.rewrite_keywords(keywords),
                })
            }
            CoreBlockPyExpr::Intrinsic(call) => CoreBlockPyExpr::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: self.rewrite_args(call.args),
                keywords: self.rewrite_keywords(call.keywords),
            }),
        }
    }
}

fn collect_deleted_names_in_fragment(
    fragment: &crate::block_py::BlockPyStmtFragment<CoreBlockPyExpr>,
    names: &mut HashSet<String>,
) {
    for stmt in &fragment.body {
        collect_deleted_names_in_stmt(stmt, names);
    }
}

fn collect_deleted_names_in_stmt(stmt: &BlockPyStmt<CoreBlockPyExpr>, names: &mut HashSet<String>) {
    match stmt {
        BlockPyStmt::Assign(assign) if is_deleted_sentinel_expr(&assign.value) => {
            names.insert(assign.target.id.to_string());
        }
        BlockPyStmt::Expr(expr) => {
            if let Some(name) = store_cell_deleted_logical_name(expr) {
                names.insert(name);
            }
        }
        BlockPyStmt::If(if_stmt) => {
            collect_deleted_names_in_fragment(&if_stmt.body, names);
            collect_deleted_names_in_fragment(&if_stmt.orelse, names);
        }
        _ => {}
    }
}

fn rewrite_deleted_name_loads_in_fragment(
    fragment: &mut crate::block_py::BlockPyStmtFragment<CoreBlockPyExpr>,
    deleted_names: &HashSet<String>,
) {
    for stmt in &mut fragment.body {
        rewrite_deleted_name_loads_in_stmt(stmt, deleted_names);
    }
    if let Some(term) = &mut fragment.term {
        rewrite_deleted_name_loads_in_term(term, deleted_names);
    }
}

fn rewrite_deleted_name_loads_in_stmt(
    stmt: &mut BlockPyStmt<CoreBlockPyExpr>,
    deleted_names: &HashSet<String>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            rewrite_deleted_name_loads_in_expr(&mut assign.value, deleted_names);
        }
        BlockPyStmt::Expr(expr) => rewrite_deleted_name_loads_in_expr(expr, deleted_names),
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            rewrite_deleted_name_loads_in_expr(test, deleted_names);
            rewrite_deleted_name_loads_in_fragment(body, deleted_names);
            rewrite_deleted_name_loads_in_fragment(orelse, deleted_names);
        }
    }
}

fn rewrite_deleted_name_loads_in_term(
    term: &mut BlockPyTerm<CoreBlockPyExpr>,
    deleted_names: &HashSet<String>,
) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => {
            rewrite_deleted_name_loads_in_expr(&mut if_term.test, deleted_names);
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_deleted_name_loads_in_expr(&mut branch.index, deleted_names);
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                rewrite_deleted_name_loads_in_expr(exc, deleted_names);
            }
        }
        BlockPyTerm::Return(value) => rewrite_deleted_name_loads_in_expr(value, deleted_names),
    }
}

fn collect_deleted_names_in_blocks(
    blocks: &[crate::block_py::CfgBlock<
        BlockPyStmt<CoreBlockPyExpr>,
        crate::block_py::BlockPyTerm<CoreBlockPyExpr>,
    >],
) -> HashSet<String> {
    let mut names = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            collect_deleted_names_in_stmt(stmt, &mut names);
        }
    }
    names
}

fn lower_name_binding_callable(
    callable: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<CoreBlockPyPass> {
    let semantic = callable.semantic.clone();
    let mut lowered = NameBindingMapper {
        semantic: &semantic,
    }
    .map_fn(callable);
    prepend_owned_cell_init_preamble(&mut lowered);
    let deleted_names = collect_deleted_names_in_blocks(&lowered.blocks);
    if !deleted_names.is_empty() {
        for block in &mut lowered.blocks {
            for stmt in &mut block.body {
                rewrite_deleted_name_loads_in_stmt(stmt, &deleted_names);
            }
            rewrite_deleted_name_loads_in_term(&mut block.term, &deleted_names);
        }
    }
    lowered
}

pub(crate) fn lower_name_binding_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPass>,
) -> BlockPyModule<CoreBlockPyPass> {
    module.map_callable_defs(lower_name_binding_callable)
}

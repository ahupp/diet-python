use crate::block_py::{
    derive_effective_binding_for_name, BlockPyBindingKind, BlockPyBindingPurpose,
    BlockPyCallableScopeKind, BlockPyCallableSemanticInfo, BlockPyCellBindingKind, FunctionName,
};
use crate::passes::ast_symbol_analysis::{collect_bound_names, collect_loaded_names};
use crate::passes::ast_to_ast::semantic::{
    SemanticAstState, SemanticBindingKind, SemanticScope, SemanticScopeKind,
};
use crate::passes::ast_to_ast::util::{
    strip_synthetic_class_namespace_qualname, strip_synthetic_module_init_qualname,
};
use ruff_python_ast::{self as ast, Stmt};
use std::collections::{HashMap, HashSet};

fn is_module_init_name(name: &str) -> bool {
    name == "_dp_module_init" || name.starts_with("_dp_fn__dp_module_init_")
}

fn display_name_for_function(raw_name: &str) -> &str {
    if raw_name.starts_with("_dp_lambda_") {
        "<lambda>"
    } else if raw_name.starts_with("_dp_genexpr_") {
        "<genexpr>"
    } else if raw_name.starts_with("_dp_listcomp_") {
        "<listcomp>"
    } else if raw_name.starts_with("_dp_setcomp_") {
        "<setcomp>"
    } else if raw_name.starts_with("_dp_dictcomp_") {
        "<dictcomp>"
    } else {
        raw_name
    }
}

fn normalize_qualname(raw_qualname: &str, raw_name: &str, display_name: &str) -> String {
    let raw_qualname = strip_synthetic_module_init_qualname(raw_qualname);
    let raw_qualname = strip_synthetic_class_namespace_qualname(&raw_qualname);
    let should_replace_tail = matches!(display_name, "<lambda>" | "<genexpr>");
    if raw_name == display_name || !should_replace_tail {
        return raw_qualname;
    }
    match raw_qualname.rsplit_once('.') {
        Some((prefix, _)) => format!("{prefix}.{display_name}"),
        None => display_name.to_string(),
    }
}

fn blockpy_binding_kind_for_name(
    name: &str,
    binding: SemanticBindingKind,
    local_cell_bindings: &HashSet<String>,
    has_local_def: bool,
    scope_kind: BlockPyCallableScopeKind,
    type_param_names: &HashSet<String>,
) -> BlockPyBindingKind {
    if scope_kind == BlockPyCallableScopeKind::Class
        && has_local_def
        && !type_param_names.contains(name)
    {
        return BlockPyBindingKind::Local;
    }
    match binding {
        SemanticBindingKind::Local if local_cell_bindings.contains(name) => {
            BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner)
        }
        SemanticBindingKind::Local => BlockPyBindingKind::Local,
        SemanticBindingKind::Nonlocal if has_local_def && local_cell_bindings.contains(name) => {
            BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner)
        }
        SemanticBindingKind::Nonlocal => BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture),
        SemanticBindingKind::Global => BlockPyBindingKind::Global,
    }
}

fn parameters_contain_name(parameters: &ast::Parameters, expected: &str) -> bool {
    parameters
        .posonlyargs
        .iter()
        .chain(parameters.args.iter())
        .chain(parameters.kwonlyargs.iter())
        .any(|param| param.parameter.name.id.as_str() == expected)
        || parameters
            .vararg
            .as_ref()
            .is_some_and(|param| param.name.id.as_str() == expected)
        || parameters
            .kwarg
            .as_ref()
            .is_some_and(|param| param.name.id.as_str() == expected)
}

fn callable_owns_synthetic_classcell(func: Option<&ast::StmtFunctionDef>) -> bool {
    func.is_some_and(|func| parameters_contain_name(func.parameters.as_ref(), "_dp_classcell_arg"))
}

pub(super) fn callable_semantic_info(
    semantic_state: &SemanticAstState,
    parent_scope: Option<&SemanticScope>,
    function_scope: Option<&SemanticScope>,
    func: Option<&ast::StmtFunctionDef>,
    body: &[Stmt],
) -> BlockPyCallableSemanticInfo {
    let Some(function_scope) = function_scope else {
        return BlockPyCallableSemanticInfo::default();
    };
    let scope_kind = match function_scope.kind() {
        SemanticScopeKind::Function => BlockPyCallableScopeKind::Function,
        SemanticScopeKind::Class => BlockPyCallableScopeKind::Class,
        SemanticScopeKind::Module => BlockPyCallableScopeKind::Module,
    };
    let local_cell_bindings = function_scope.local_cell_bindings();
    let local_defs = function_scope.local_def_names();
    let type_param_names = function_scope.type_param_names();
    let mut bindings = function_scope
        .bindings()
        .into_iter()
        .map(|(name, binding)| {
            (
                name.clone(),
                blockpy_binding_kind_for_name(
                    name.as_str(),
                    binding,
                    &local_cell_bindings,
                    function_scope.has_local_def(name.as_str()),
                    scope_kind,
                    &type_param_names,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut relevant_names = collect_bound_names(body);
    relevant_names.extend(collect_loaded_names(body));
    for name in relevant_names {
        bindings.entry(name.clone()).or_insert_with(|| {
            blockpy_binding_kind_for_name(
                name.as_str(),
                function_scope.resolved_load_binding(name.as_str()),
                &local_cell_bindings,
                function_scope.has_local_def(name.as_str()),
                scope_kind,
                &type_param_names,
            )
        });
    }
    let effective_load_bindings = bindings
        .iter()
        .map(|(name, binding)| {
            (
                name.clone(),
                derive_effective_binding_for_name(
                    name.as_str(),
                    *binding,
                    scope_kind,
                    &type_param_names,
                    BlockPyBindingPurpose::Load,
                    false,
                ),
            )
        })
        .collect();
    let effective_store_bindings = bindings
        .iter()
        .map(|(name, binding)| {
            (
                name.clone(),
                derive_effective_binding_for_name(
                    name.as_str(),
                    *binding,
                    scope_kind,
                    &type_param_names,
                    BlockPyBindingPurpose::Store,
                    false,
                ),
            )
        })
        .collect();
    let names = match func {
        Some(func) => {
            let raw_bind_name = func.name.id.to_string();
            let bind_name = if is_module_init_name(raw_bind_name.as_str()) {
                "_dp_module_init".to_string()
            } else {
                raw_bind_name.clone()
            };
            let display_name = display_name_for_function(bind_name.as_str()).to_string();
            let qualname = if is_module_init_name(raw_bind_name.as_str()) {
                "_dp_module_init".to_string()
            } else if semantic_state.has_function_scope_override(func) {
                normalize_qualname(
                    parent_scope
                        .expect("missing parent scope for function scope override")
                        .child_function_qualname(raw_bind_name.as_str())
                        .as_str(),
                    bind_name.as_str(),
                    display_name.as_str(),
                )
            } else {
                normalize_qualname(
                    function_scope.qualname(),
                    bind_name.as_str(),
                    display_name.as_str(),
                )
            };
            FunctionName::new(bind_name, raw_bind_name, display_name, qualname)
        }
        None => FunctionName::default(),
    };
    let mut info = BlockPyCallableSemanticInfo {
        names,
        scope_kind,
        bindings,
        local_defs,
        cell_storage_names: function_scope.cell_storage_names(),
        semantic_internal_names: HashSet::new(),
        type_param_names,
        effective_load_bindings,
        effective_store_bindings,
    };
    if callable_owns_synthetic_classcell(func) && !info.bindings.contains_key("__class__") {
        info.local_defs.insert("__class__".to_string());
        info.insert_binding(
            "__class__",
            BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner),
            false,
            Some("_dp_classcell".to_string()),
        );
    }
    info
}

use crate::block_py::BindingTarget;
use crate::passes::ast_to_ast::body::Suite;
use crate::passes::ast_to_ast::scope::is_internal_symbol;
use crate::passes::ast_to_ast::scope::{BindingKind, BindingUse, Scope, ScopeKind};
use crate::passes::ast_to_ast::util::{
    strip_synthetic_class_namespace_qualname, strip_synthetic_module_init_qualname,
};
use crate::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, NodeIndex};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct FunctionIdentity {
    pub bind_name: String,
    pub display_name: String,
    pub qualname: String,
    pub binding_target: BindingTarget,
}

pub(crate) fn is_module_init_temp_name(name: &str) -> bool {
    name == "_dp_module_init" || name.starts_with("_dp_fn__dp_module_init_")
}

pub(crate) fn display_name_for_function(raw_name: &str) -> &str {
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

pub(crate) fn default_binding_target_for_parent(
    current_parent: Option<&str>,
    bind_name: &str,
) -> BindingTarget {
    match current_parent {
        Some(parent) if is_module_init_temp_name(parent) => {
            if is_internal_symbol(bind_name) {
                BindingTarget::Local
            } else {
                BindingTarget::ModuleGlobal
            }
        }
        Some(parent) if parent.starts_with("_dp_class_ns_") => {
            if is_internal_symbol(bind_name) {
                BindingTarget::Local
            } else {
                BindingTarget::ClassNamespace
            }
        }
        _ => BindingTarget::Local,
    }
}

pub(crate) fn resolve_runtime_function_identity(
    func: &ast::StmtFunctionDef,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    current_parent: Option<&str>,
) -> FunctionIdentity {
    if is_module_init_temp_name(func.name.id.as_str()) {
        return FunctionIdentity {
            bind_name: "_dp_module_init".to_string(),
            display_name: "_dp_module_init".to_string(),
            qualname: "_dp_module_init".to_string(),
            binding_target: BindingTarget::ModuleGlobal,
        };
    }
    let node_index = func.node_index.load();
    if let Some(identity) = function_identity_by_node.get(&node_index) {
        let mut identity = identity.clone();
        if current_parent.is_some_and(|parent| parent.starts_with("_dp_class_ns_"))
            && !is_internal_symbol(func.name.id.as_str())
        {
            identity.binding_target = BindingTarget::ClassNamespace;
        }
        return identity;
    }
    let bind_name = func.name.id.to_string();
    let display_name = display_name_for_function(bind_name.as_str()).to_string();
    FunctionIdentity {
        bind_name: bind_name.clone(),
        display_name,
        qualname: bind_name.clone(),
        binding_target: default_binding_target_for_parent(current_parent, bind_name.as_str()),
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

pub(crate) fn collect_function_identity_private(
    module: &mut Suite,
    module_scope: Arc<Scope>,
) -> HashMap<NodeIndex, FunctionIdentity> {
    fn binding_target_for_scope(scope: &Scope, bind_name: &str) -> BindingTarget {
        if is_internal_symbol(bind_name) {
            return BindingTarget::Local;
        }
        let binding = scope.binding_in_scope(bind_name, BindingUse::Load);
        match (scope.kind(), binding) {
            (ScopeKind::Class, BindingKind::Local) => BindingTarget::ClassNamespace,
            (_, BindingKind::Global) => BindingTarget::ModuleGlobal,
            _ => BindingTarget::Local,
        }
    }

    struct Collector {
        scope_stack: Vec<Arc<Scope>>,
        out: HashMap<NodeIndex, FunctionIdentity>,
    }

    impl Transformer for Collector {
        fn visit_stmt(&mut self, stmt: &mut ast::Stmt) {
            match stmt {
                ast::Stmt::FunctionDef(func) => {
                    let node_index = func.node_index.load();
                    if node_index != NodeIndex::NONE {
                        let raw_bind_name = func.name.id.to_string();
                        let bind_name = if is_module_init_temp_name(raw_bind_name.as_str()) {
                            "_dp_module_init".to_string()
                        } else {
                            raw_bind_name.clone()
                        };
                        let display_name =
                            display_name_for_function(bind_name.as_str()).to_string();
                        let parent_scope = self
                            .scope_stack
                            .last()
                            .expect("missing scope while collecting function identity");
                        let child_scope = parent_scope.tree.scope_for_def(func).ok();
                        let qualname = if is_module_init_temp_name(raw_bind_name.as_str()) {
                            "_dp_module_init".to_string()
                        } else {
                            child_scope
                                .as_ref()
                                .map(|scope| {
                                    normalize_qualname(
                                        scope.qualnamer.qualname.as_str(),
                                        bind_name.as_str(),
                                        display_name.as_str(),
                                    )
                                })
                                .unwrap_or_else(|| bind_name.clone())
                        };
                        self.out.insert(
                            node_index,
                            FunctionIdentity {
                                bind_name: bind_name.clone(),
                                display_name,
                                qualname,
                                binding_target: binding_target_for_scope(
                                    parent_scope.as_ref(),
                                    raw_bind_name.as_str(),
                                ),
                            },
                        );
                        if let Some(child_scope) = child_scope {
                            self.scope_stack.push(child_scope);
                            walk_stmt(self, stmt);
                            self.scope_stack.pop();
                            return;
                        }
                    }
                    walk_stmt(self, stmt);
                }
                ast::Stmt::ClassDef(class_def) => {
                    let parent_scope = self
                        .scope_stack
                        .last()
                        .expect("missing scope while collecting class scope");
                    if let Ok(child_scope) = parent_scope.tree.scope_for_def(class_def) {
                        self.scope_stack.push(child_scope);
                        walk_stmt(self, stmt);
                        self.scope_stack.pop();
                        return;
                    }
                    walk_stmt(self, stmt);
                }
                _ => walk_stmt(self, stmt),
            }
        }
    }

    let mut module = module.clone();
    let mut collector = Collector {
        scope_stack: vec![module_scope],
        out: HashMap::new(),
    };
    collector.visit_body(&mut module);
    collector.out
}

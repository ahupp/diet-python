use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ruff_python_ast::{HasNodeIndex, NodeIndex, StmtClassDef, StmtFunctionDef};

use crate::passes::ast_to_ast::scope::{BindingUse, Scope, ScopeKind, ScopeTree};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticBindingKind {
    Local,
    Nonlocal,
    Global,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticBindingUse {
    Load,
    Modify,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticScopeKind {
    Function,
    Class,
    Module,
}

#[derive(Clone, Debug)]
pub(crate) struct SemanticScope {
    raw: Arc<Scope>,
}

impl SemanticScope {
    fn new(raw: Arc<Scope>) -> Self {
        Self { raw }
    }

    pub(crate) fn kind(&self) -> SemanticScopeKind {
        match self.raw.kind() {
            ScopeKind::Function => SemanticScopeKind::Function,
            ScopeKind::Class => SemanticScopeKind::Class,
            ScopeKind::Module => SemanticScopeKind::Module,
        }
    }

    pub(crate) fn binding_in_scope(
        &self,
        name: &str,
        use_kind: SemanticBindingUse,
    ) -> SemanticBindingKind {
        let use_kind = match use_kind {
            SemanticBindingUse::Load => BindingUse::Load,
            SemanticBindingUse::Modify => BindingUse::Modify,
        };
        match self.raw.binding_in_scope(name, use_kind) {
            crate::passes::ast_to_ast::scope::BindingKind::Local => SemanticBindingKind::Local,
            crate::passes::ast_to_ast::scope::BindingKind::Nonlocal => {
                SemanticBindingKind::Nonlocal
            }
            crate::passes::ast_to_ast::scope::BindingKind::Global => SemanticBindingKind::Global,
        }
    }

    pub(crate) fn binding_in_current_scope(&self, name: &str) -> Option<SemanticBindingKind> {
        self.raw
            .scope_bindings()
            .get(name)
            .copied()
            .map(|binding| match binding {
                crate::passes::ast_to_ast::scope::BindingKind::Local => SemanticBindingKind::Local,
                crate::passes::ast_to_ast::scope::BindingKind::Nonlocal => {
                    SemanticBindingKind::Nonlocal
                }
                crate::passes::ast_to_ast::scope::BindingKind::Global => {
                    SemanticBindingKind::Global
                }
            })
    }

    pub(crate) fn child_scope_for_function(
        &self,
        func_def: &StmtFunctionDef,
    ) -> Option<SemanticScope> {
        self.raw
            .child_scope_for_function(func_def)
            .ok()
            .map(SemanticScope::new)
    }

    pub(crate) fn child_scope_for_class(&self, class_def: &StmtClassDef) -> Option<SemanticScope> {
        self.raw
            .child_scope_for_class(class_def)
            .ok()
            .map(SemanticScope::new)
    }

    pub(crate) fn local_cell_bindings(&self) -> HashSet<String> {
        self.raw.local_cell_bindings()
    }

    pub(crate) fn has_binding(&self, name: &str) -> bool {
        self.raw.scope_bindings().contains_key(name)
    }

    pub(crate) fn parent_scope(&self) -> Option<SemanticScope> {
        self.raw.parent_scope().map(SemanticScope::new)
    }

    pub(crate) fn any_parent_scope<T>(
        &self,
        mut func: impl FnMut(&SemanticScope) -> Option<T>,
    ) -> Option<T> {
        let mut current = Some(self.clone());
        while let Some(scope) = current {
            if let Some(ret) = func(&scope) {
                return Some(ret);
            }
            current = scope.parent_scope();
        }
        None
    }

    pub(crate) fn qualname(&self) -> &str {
        self.raw.qualnamer.qualname.as_str()
    }

    pub(crate) fn child_function_qualname(&self, name: &str) -> String {
        self.raw
            .qualnamer
            .enter_scope(ScopeKind::Function, name.to_string())
            .qualname
    }
}

#[derive(Clone, Default)]
pub(crate) struct SemanticProvenance {
    function_scope_overrides: HashMap<NodeIndex, Arc<Scope>>,
}

impl SemanticProvenance {
    pub(crate) fn register_function_scope_override(
        &mut self,
        scope_tree: &Arc<ScopeTree>,
        func_def: &StmtFunctionDef,
        scope: Arc<Scope>,
    ) {
        let node_index = scope_tree.ensure_node_index(func_def);
        self.function_scope_overrides.insert(node_index, scope);
    }

    pub(crate) fn function_scope_override(&self, func_def: &StmtFunctionDef) -> Option<Arc<Scope>> {
        self.function_scope_overrides
            .get(&func_def.node_index().load())
            .cloned()
    }
}

#[derive(Clone)]
pub(crate) struct SemanticAstState {
    module_scope: Arc<Scope>,
    provenance: SemanticProvenance,
}

impl SemanticAstState {
    pub(crate) fn new(module_scope: Arc<Scope>) -> Self {
        Self {
            module_scope,
            provenance: SemanticProvenance::default(),
        }
    }

    pub(crate) fn module_scope(&self) -> SemanticScope {
        SemanticScope::new(self.module_scope.clone())
    }

    pub(crate) fn register_function_scope_override(
        &mut self,
        func_def: &StmtFunctionDef,
        scope: SemanticScope,
    ) {
        self.provenance.register_function_scope_override(
            &self.module_scope.tree,
            func_def,
            scope.raw,
        );
    }

    pub(crate) fn function_scope(&self, func_def: &StmtFunctionDef) -> Option<SemanticScope> {
        self.provenance
            .function_scope_override(func_def)
            .or_else(|| self.module_scope.tree.scope_for_def(func_def).ok())
            .map(SemanticScope::new)
    }

    pub(crate) fn has_function_scope_override(&self, func_def: &StmtFunctionDef) -> bool {
        self.provenance.function_scope_override(func_def).is_some()
    }
}

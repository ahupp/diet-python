use std::collections::HashMap;
use std::sync::Arc;

use ruff_python_ast::{HasNodeIndex, NodeIndex, StmtFunctionDef};

use crate::passes::ast_to_ast::scope::{Scope, ScopeTree};

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

    pub(crate) fn module_scope(&self) -> Arc<Scope> {
        self.module_scope.clone()
    }

    pub(crate) fn provenance_mut(&mut self) -> &mut SemanticProvenance {
        &mut self.provenance
    }

    pub(crate) fn function_scope(&self, func_def: &StmtFunctionDef) -> Option<Arc<Scope>> {
        self.provenance
            .function_scope_override(func_def)
            .or_else(|| self.module_scope.tree.scope_for_def(func_def).ok())
    }
}

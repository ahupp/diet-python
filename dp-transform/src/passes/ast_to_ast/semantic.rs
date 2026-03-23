use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ruff_python_ast::{
    self as ast, ExprContext, HasNodeIndex, NodeIndex, StmtClassDef, StmtFunctionDef,
};

use crate::passes::ast_to_ast::body::{suite_mut, Suite};
use crate::passes::ast_to_ast::scope::is_internal_symbol;
use crate::passes::ast_to_ast::scope::{BindingUse, Scope, ScopeKind, ScopeTree};
use crate::transformer::Transformer;

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

    pub(crate) fn class_scope(&self, class_def: &StmtClassDef) -> Option<SemanticScope> {
        self.module_scope
            .tree
            .scope_for_def(class_def)
            .ok()
            .map(SemanticScope::new)
    }

    pub(crate) fn has_function_scope_override(&self, func_def: &StmtFunctionDef) -> bool {
        self.provenance.function_scope_override(func_def).is_some()
    }
}

trait SemanticResolver {
    type Scope: Clone;

    fn module_scope(&self) -> Self::Scope;
    fn function_scope(&self, func_def: &StmtFunctionDef) -> Option<Self::Scope>;
    fn class_scope(&self, class_def: &StmtClassDef) -> Option<Self::Scope>;
    fn scope_kind(&self, scope: &Self::Scope) -> SemanticScopeKind;
    fn binding_in_scope_checked(
        &self,
        scope: &Self::Scope,
        name: &str,
        use_kind: SemanticBindingUse,
    ) -> Option<SemanticBindingKind>;
}

impl SemanticResolver for SemanticAstState {
    type Scope = SemanticScope;

    fn module_scope(&self) -> Self::Scope {
        SemanticAstState::module_scope(self)
    }

    fn function_scope(&self, func_def: &StmtFunctionDef) -> Option<Self::Scope> {
        SemanticAstState::function_scope(self, func_def)
    }

    fn class_scope(&self, class_def: &StmtClassDef) -> Option<Self::Scope> {
        SemanticAstState::class_scope(self, class_def)
    }

    fn scope_kind(&self, scope: &Self::Scope) -> SemanticScopeKind {
        scope.kind()
    }

    fn binding_in_scope_checked(
        &self,
        scope: &Self::Scope,
        name: &str,
        use_kind: SemanticBindingUse,
    ) -> Option<SemanticBindingKind> {
        match scope.binding_in_current_scope(name) {
            Some(binding) => Some(binding),
            None => match use_kind {
                SemanticBindingUse::Load => Some(SemanticBindingKind::Local),
                SemanticBindingUse::Modify => None,
            },
        }
    }
}

#[derive(Clone)]
struct ScopeTreeSemanticResolver {
    module_scope: Arc<Scope>,
    provenance: SemanticProvenance,
}

impl ScopeTreeSemanticResolver {
    fn from_semantic_state(semantic_state: &SemanticAstState) -> Self {
        Self {
            module_scope: semantic_state.module_scope.clone(),
            provenance: semantic_state.provenance.clone(),
        }
    }
}

impl SemanticResolver for ScopeTreeSemanticResolver {
    type Scope = Arc<Scope>;

    fn module_scope(&self) -> Self::Scope {
        self.module_scope.clone()
    }

    fn function_scope(&self, func_def: &StmtFunctionDef) -> Option<Self::Scope> {
        self.provenance
            .function_scope_override(func_def)
            .or_else(|| self.module_scope.tree.scope_for_def(func_def).ok())
    }

    fn class_scope(&self, class_def: &StmtClassDef) -> Option<Self::Scope> {
        self.module_scope.tree.scope_for_def(class_def).ok()
    }

    fn scope_kind(&self, scope: &Self::Scope) -> SemanticScopeKind {
        match scope.kind() {
            ScopeKind::Function => SemanticScopeKind::Function,
            ScopeKind::Class => SemanticScopeKind::Class,
            ScopeKind::Module => SemanticScopeKind::Module,
        }
    }

    fn binding_in_scope_checked(
        &self,
        scope: &Self::Scope,
        name: &str,
        use_kind: SemanticBindingUse,
    ) -> Option<SemanticBindingKind> {
        match scope.scope_bindings().get(name).copied() {
            Some(crate::passes::ast_to_ast::scope::BindingKind::Local) => {
                Some(SemanticBindingKind::Local)
            }
            Some(crate::passes::ast_to_ast::scope::BindingKind::Nonlocal) => {
                Some(SemanticBindingKind::Nonlocal)
            }
            Some(crate::passes::ast_to_ast::scope::BindingKind::Global) => {
                Some(SemanticBindingKind::Global)
            }
            None => match use_kind {
                SemanticBindingUse::Load => Some(SemanticBindingKind::Local),
                SemanticBindingUse::Modify => None,
            },
        }
    }
}

fn compare_semantic_resolvers<Expected, Actual>(
    module: &mut Suite,
    expected: &Expected,
    actual: &Actual,
) -> Vec<String>
where
    Expected: SemanticResolver,
    Actual: SemanticResolver,
{
    struct Comparator<'a, Expected: SemanticResolver, Actual: SemanticResolver> {
        expected: &'a Expected,
        actual: &'a Actual,
        expected_scope_stack: Vec<Expected::Scope>,
        actual_scope_stack: Vec<Actual::Scope>,
        issues: Vec<String>,
    }

    impl<Expected, Actual> Comparator<'_, Expected, Actual>
    where
        Expected: SemanticResolver,
        Actual: SemanticResolver,
    {
        fn compare_name(&mut self, name: &ast::ExprName) {
            let id = name.id.as_str();
            if id == "__class__" || is_internal_symbol(id) {
                return;
            }
            let use_kind = match name.ctx {
                ExprContext::Load => SemanticBindingUse::Load,
                ExprContext::Store | ExprContext::Del => SemanticBindingUse::Modify,
                ExprContext::Invalid => return,
            };
            let Some(expected_scope) = self.expected_scope_stack.last() else {
                self.issues
                    .push(format!("missing expected scope for name {id}"));
                return;
            };
            let Some(actual_scope) = self.actual_scope_stack.last() else {
                self.issues
                    .push(format!("missing actual scope for name {id}"));
                return;
            };
            let expected_binding =
                self.expected
                    .binding_in_scope_checked(expected_scope, id, use_kind);
            let actual_binding = self
                .actual
                .binding_in_scope_checked(actual_scope, id, use_kind);
            match (expected_binding, actual_binding) {
                (Some(expected_binding), Some(actual_binding)) => {
                    if expected_binding != actual_binding {
                        self.issues.push(format!(
                            "binding mismatch for name {id} at {:?}: expected {:?}, got {:?}",
                            name.node_index().load(),
                            expected_binding,
                            actual_binding
                        ));
                    }
                }
                (Some(expected_binding), None) => self.issues.push(format!(
                    "binding missing in actual resolver for name {id} at {:?}: expected {:?}",
                    name.node_index().load(),
                    expected_binding
                )),
                (None, Some(actual_binding)) => self.issues.push(format!(
                    "binding missing in expected resolver for name {id} at {:?}: got {:?}",
                    name.node_index().load(),
                    actual_binding
                )),
                (None, None) => {}
            }
        }

        fn compare_scope_entry<EScope, AScope>(
            &mut self,
            label: &str,
            name: &str,
            node_index: NodeIndex,
            expected_scope: Option<EScope>,
            actual_scope: Option<AScope>,
        ) where
            EScope: Into<Expected::Scope>,
            AScope: Into<Actual::Scope>,
        {
            match (expected_scope.map(Into::into), actual_scope.map(Into::into)) {
                (Some(expected_scope), Some(actual_scope)) => {
                    let expected_kind = self.expected.scope_kind(&expected_scope);
                    let actual_kind = self.actual.scope_kind(&actual_scope);
                    if expected_kind != actual_kind {
                        self.issues.push(format!(
                            "{label} scope mismatch for {name} at {:?}: expected {:?}, got {:?}",
                            node_index, expected_kind, actual_kind
                        ));
                    }
                    self.expected_scope_stack.push(expected_scope);
                    self.actual_scope_stack.push(actual_scope);
                }
                (None, None) => {}
                (Some(_), None) => self.issues.push(format!(
                    "{label} scope missing in actual resolver for {name} at {:?}",
                    node_index
                )),
                (None, Some(_)) => self.issues.push(format!(
                    "{label} scope missing in expected resolver for {name} at {:?}",
                    node_index
                )),
            }
        }

        fn pop_scope_pair(&mut self) {
            self.expected_scope_stack.pop();
            self.actual_scope_stack.pop();
        }
    }

    impl<Expected, Actual> Transformer for Comparator<'_, Expected, Actual>
    where
        Expected: SemanticResolver,
        Actual: SemanticResolver,
    {
        fn visit_expr(&mut self, expr: &mut ast::Expr) {
            if let ast::Expr::Name(name) = expr {
                self.compare_name(name);
            }
            crate::transformer::walk_expr(self, expr);
        }

        fn visit_stmt(&mut self, stmt: &mut ast::Stmt) {
            match stmt {
                ast::Stmt::FunctionDef(func_def) => {
                    for decorator in &mut func_def.decorator_list {
                        self.visit_decorator(decorator);
                    }
                    if let Some(type_params) = func_def.type_params.as_mut() {
                        self.visit_type_params(type_params);
                    }
                    self.visit_parameters(&mut func_def.parameters);
                    if let Some(returns) = func_def.returns.as_mut() {
                        self.visit_annotation(returns);
                    }
                    let previous_expected_len = self.expected_scope_stack.len();
                    self.compare_scope_entry(
                        "function",
                        func_def.name.id.as_str(),
                        func_def.node_index().load(),
                        self.expected.function_scope(func_def),
                        self.actual.function_scope(func_def),
                    );
                    if self.expected_scope_stack.len() == previous_expected_len + 1 {
                        self.visit_body(suite_mut(&mut func_def.body));
                        self.pop_scope_pair();
                    } else {
                        self.visit_body(suite_mut(&mut func_def.body));
                    }
                }
                ast::Stmt::ClassDef(class_def) => {
                    for decorator in &mut class_def.decorator_list {
                        self.visit_decorator(decorator);
                    }
                    if let Some(type_params) = class_def.type_params.as_mut() {
                        self.visit_type_params(type_params);
                    }
                    if let Some(arguments) = class_def.arguments.as_mut() {
                        self.visit_arguments(arguments);
                    }
                    let previous_expected_len = self.expected_scope_stack.len();
                    self.compare_scope_entry(
                        "class",
                        class_def.name.id.as_str(),
                        class_def.node_index().load(),
                        self.expected.class_scope(class_def),
                        self.actual.class_scope(class_def),
                    );
                    if self.expected_scope_stack.len() == previous_expected_len + 1 {
                        self.visit_body(suite_mut(&mut class_def.body));
                        self.pop_scope_pair();
                    } else {
                        self.visit_body(suite_mut(&mut class_def.body));
                    }
                }
                _ => crate::transformer::walk_stmt(self, stmt),
            }
        }
    }

    let mut cloned_module = module.clone();
    let mut comparator = Comparator {
        expected,
        actual,
        expected_scope_stack: vec![expected.module_scope()],
        actual_scope_stack: vec![actual.module_scope()],
        issues: Vec::new(),
    };
    comparator.visit_body(&mut cloned_module);
    comparator.issues
}

pub(crate) fn debug_assert_matches_scope_tree(
    module: &mut Suite,
    semantic_state: &SemanticAstState,
) {
    if !cfg!(debug_assertions) {
        return;
    }
    let expected = ScopeTreeSemanticResolver::from_semantic_state(semantic_state);
    let issues = compare_semantic_resolvers(module, &expected, semantic_state);
    assert!(
        issues.is_empty(),
        "semantic resolver mismatch:\n{}",
        issues.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::{compare_semantic_resolvers, ScopeTreeSemanticResolver, SemanticAstState};
    use crate::passes::ast_to_ast::context::Context;
    use crate::passes::ast_to_ast::rewrite_class_def::class_body::rewrite_class_body_scopes;
    use crate::passes::ast_to_ast::scope::analyze_module_scope;
    use crate::passes::ast_to_ast::Options;
    use ruff_python_parser::parse_module;

    #[test]
    fn semantic_comparison_accepts_class_helper_scope_overrides() {
        let source = concat!(
            "def outer():\n",
            "    shared = 1\n",
            "    class Box:\n",
            "        probe = shared\n",
            "        def get(self):\n",
            "            return shared\n",
            "    return Box\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let module_scope = analyze_module_scope(&mut module);
        let mut semantic_state = SemanticAstState::new(module_scope.clone());
        rewrite_class_body_scopes(&context, &mut semantic_state, &mut module);

        let expected = ScopeTreeSemanticResolver::from_semantic_state(&semantic_state);
        let issues = compare_semantic_resolvers(&mut module, &expected, &semantic_state);
        assert!(issues.is_empty(), "{issues:#?}");
    }

    #[test]
    fn semantic_comparison_detects_missing_class_helper_scope_overrides() {
        let source = concat!(
            "def outer():\n",
            "    shared = 1\n",
            "    class Box:\n",
            "        probe = shared\n",
            "        def get(self):\n",
            "            return shared\n",
            "    return Box\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let module_scope = analyze_module_scope(&mut module);
        let mut semantic_state = SemanticAstState::new(module_scope.clone());
        rewrite_class_body_scopes(&context, &mut semantic_state, &mut module);

        let expected = ScopeTreeSemanticResolver::from_semantic_state(&semantic_state);
        let broken_state = SemanticAstState::new(module_scope);
        let issues = compare_semantic_resolvers(&mut module, &expected, &broken_state);
        assert!(!issues.is_empty(), "expected missing override mismatch");
    }
}

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use ruff_python_ast::{
    self as ast, ExprContext, HasNodeIndex, NodeIndex, StmtClassDef, StmtFunctionDef,
};
use ruff_python_semantic::{
    BindingFlags as RuffBindingFlags, BindingKind as RuffBindingKind, Module as RuffModule,
    ModuleKind as RuffModuleKind, ModuleSource as RuffModuleSource, ScopeId as RuffScopeId,
    ScopeKind as RuffScopeKind, SemanticModel as RuffSemanticModel,
};
use ruff_text_size::{Ranged, TextRange};

use crate::passes::ast_to_ast::body::Suite;
use crate::passes::ast_to_ast::scope_helpers::is_internal_symbol;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct SemanticScopeId(usize);

#[derive(Clone, Debug)]
struct SemanticScopeData {
    kind: SemanticScopeKind,
    bindings: HashMap<String, SemanticBindingKind>,
    local_defs: HashSet<String>,
    local_cell_bindings: HashSet<String>,
    parent: Option<SemanticScopeId>,
    qualname: String,
    reuses_child_scopes: bool,
    function_children: HashMap<NodeIndex, SemanticScopeId>,
    class_children: HashMap<NodeIndex, SemanticScopeId>,
}

#[derive(Clone, Debug)]
struct SemanticSnapshot {
    scopes: Vec<SemanticScopeData>,
}

impl SemanticSnapshot {
    fn scope(&self, scope_id: SemanticScopeId) -> &SemanticScopeData {
        &self.scopes[scope_id.0]
    }

    fn scope_mut(&mut self, scope_id: SemanticScopeId) -> &mut SemanticScopeData {
        &mut self.scopes[scope_id.0]
    }
}

#[derive(Debug, Default)]
struct SemanticProvenance {
    function_scope_overrides: HashMap<NodeIndex, SemanticScopeId>,
    next_node_index: u32,
}

impl SemanticProvenance {
    fn ensure_node_index<T: HasNodeIndex>(&mut self, node: &T) -> NodeIndex {
        let node_index = node.node_index().load();
        if node_index != NodeIndex::NONE {
            if let Some(value) = node_index.as_u32() {
                self.next_node_index = self.next_node_index.max(value + 1);
            }
            return node_index;
        }

        let index = NodeIndex::from(self.next_node_index);
        self.next_node_index += 1;
        node.node_index().set(index);
        index
    }

    fn function_scope_override(&self, func_def: &StmtFunctionDef) -> Option<SemanticScopeId> {
        self.function_scope_overrides
            .get(&func_def.node_index().load())
            .copied()
    }
}

#[derive(Default)]
struct MaxNodeIndexCollector {
    max: u32,
}

impl Transformer for MaxNodeIndexCollector {
    fn visit_stmt(&mut self, stmt: &mut ast::Stmt) {
        if let Some(value) = stmt.node_index().load().as_u32() {
            self.max = self.max.max(value);
        }
        crate::transformer::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        if let Some(value) = expr.node_index().load().as_u32() {
            self.max = self.max.max(value);
        }
        crate::transformer::walk_expr(self, expr);
    }
}

fn next_node_index_for_suite(module: &mut Suite) -> u32 {
    let mut cloned = module.clone();
    let mut collector = MaxNodeIndexCollector::default();
    collector.visit_body(&mut cloned);
    collector.max.saturating_add(1)
}

struct MissingNodeIndexAssigner {
    next: u32,
}

impl Transformer for MissingNodeIndexAssigner {
    fn visit_stmt(&mut self, stmt: &mut ast::Stmt) {
        if stmt.node_index().load() == NodeIndex::NONE {
            stmt.node_index().set(NodeIndex::from(self.next));
            self.next += 1;
        }
        crate::transformer::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        if expr.node_index().load() == NodeIndex::NONE {
            expr.node_index().set(NodeIndex::from(self.next));
            self.next += 1;
        }
        crate::transformer::walk_expr(self, expr);
    }
}

fn ensure_node_indices_for_suite(module: &mut Suite) -> u32 {
    let next = next_node_index_for_suite(module);
    MissingNodeIndexAssigner { next }.visit_body(module);
    next_node_index_for_suite(module)
}

#[derive(Clone, Debug)]
struct SemanticStateInner {
    snapshot: SemanticSnapshot,
}

#[derive(Clone, Debug)]
pub(crate) struct SemanticAstState {
    inner: Arc<SemanticStateInner>,
    provenance: Arc<Mutex<SemanticProvenance>>,
}

#[derive(Clone, Debug)]
pub(crate) struct SemanticScope {
    state: SemanticAstState,
    scope_id: SemanticScopeId,
}

impl SemanticScope {
    fn new(state: SemanticAstState, scope_id: SemanticScopeId) -> Self {
        Self { state, scope_id }
    }

    fn data(&self) -> &SemanticScopeData {
        self.state.inner.snapshot.scope(self.scope_id)
    }

    pub(crate) fn kind(&self) -> SemanticScopeKind {
        self.data().kind
    }

    pub(crate) fn binding_in_scope(
        &self,
        name: &str,
        use_kind: SemanticBindingUse,
    ) -> SemanticBindingKind {
        match self.binding_in_current_scope(name) {
            Some(binding) => binding,
            None => match use_kind {
                SemanticBindingUse::Load => SemanticBindingKind::Local,
                SemanticBindingUse::Modify => {
                    panic!(
                        "Name not found in semantic scope: {name} {:?}",
                        self.scope_id
                    )
                }
            },
        }
    }

    pub(crate) fn binding_in_current_scope(&self, name: &str) -> Option<SemanticBindingKind> {
        self.data().bindings.get(name).copied()
    }

    pub(crate) fn local_binding_names(&self) -> HashSet<String> {
        self.data()
            .bindings
            .iter()
            .filter_map(|(name, kind)| {
                matches!(kind, SemanticBindingKind::Local).then(|| name.clone())
            })
            .collect()
    }

    pub(crate) fn child_scope_for_function(
        &self,
        func_def: &StmtFunctionDef,
    ) -> Option<SemanticScope> {
        if let Some(scope_id) = self.state.function_scope_id(func_def) {
            let child = self.state.inner.snapshot.scope(scope_id);
            if child.parent == Some(self.scope_id) || self.data().reuses_child_scopes {
                return Some(self.state.scope(scope_id));
            }
        }
        self.data()
            .function_children
            .get(&func_def.node_index().load())
            .copied()
            .filter(|scope_id| {
                self.data().reuses_child_scopes
                    || self.state.inner.snapshot.scope(*scope_id).parent == Some(self.scope_id)
            })
            .map(|scope_id| self.state.scope(scope_id))
    }

    pub(crate) fn child_scope_for_class(&self, class_def: &StmtClassDef) -> Option<SemanticScope> {
        self.data()
            .class_children
            .get(&class_def.node_index().load())
            .copied()
            .filter(|scope_id| {
                self.data().reuses_child_scopes
                    || self.state.inner.snapshot.scope(*scope_id).parent == Some(self.scope_id)
            })
            .map(|scope_id| self.state.scope(scope_id))
    }

    pub(crate) fn local_cell_bindings(&self) -> HashSet<String> {
        self.data().local_cell_bindings.clone()
    }

    pub(crate) fn has_binding(&self, name: &str) -> bool {
        self.data().bindings.contains_key(name)
    }

    pub(crate) fn parent_scope(&self) -> Option<SemanticScope> {
        self.data()
            .parent
            .map(|scope_id| self.state.scope(scope_id))
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
        self.data().qualname.as_str()
    }

    pub(crate) fn child_function_qualname(&self, name: &str) -> String {
        child_qualname(self.data(), name)
    }
}

fn child_qualname(parent: &SemanticScopeData, name: &str) -> String {
    if matches!(parent.bindings.get(name), Some(SemanticBindingKind::Global)) {
        return name.to_string();
    }
    match parent.kind {
        SemanticScopeKind::Module => name.to_string(),
        SemanticScopeKind::Function => format!("{}.<locals>.{name}", parent.qualname),
        SemanticScopeKind::Class => format!("{}.{}", parent.qualname, name),
    }
}

#[derive(Default)]
struct RuffScopeBindingCollector {
    bound_names: HashSet<String>,
    explicit_globals: Vec<(String, TextRange)>,
    explicit_nonlocals: Vec<(String, TextRange)>,
    load_names: HashSet<String>,
}

impl Transformer for RuffScopeBindingCollector {
    fn visit_stmt(&mut self, stmt: &mut ast::Stmt) {
        match stmt {
            ast::Stmt::Assign(assign) => {
                for target in &assign.targets {
                    collect_bound_target_names(target, &mut self.bound_names);
                }
                crate::transformer::walk_stmt(self, stmt);
            }
            ast::Stmt::AugAssign(aug) => {
                collect_bound_target_names(aug.target.as_ref(), &mut self.bound_names);
                crate::transformer::walk_stmt(self, stmt);
            }
            ast::Stmt::AnnAssign(ann) => {
                collect_bound_target_names(ann.target.as_ref(), &mut self.bound_names);
                crate::transformer::walk_stmt(self, stmt);
            }
            ast::Stmt::For(for_stmt) => {
                collect_bound_target_names(for_stmt.target.as_ref(), &mut self.bound_names);
                crate::transformer::walk_stmt(self, stmt);
            }
            ast::Stmt::With(with_stmt) => {
                for item in &with_stmt.items {
                    if let Some(optional_vars) = item.optional_vars.as_ref() {
                        collect_bound_target_names(optional_vars.as_ref(), &mut self.bound_names);
                    }
                }
                crate::transformer::walk_stmt(self, stmt);
            }
            ast::Stmt::Delete(delete_stmt) => {
                for target in &delete_stmt.targets {
                    collect_bound_target_names(target, &mut self.bound_names);
                }
                crate::transformer::walk_stmt(self, stmt);
            }
            ast::Stmt::Try(try_stmt) => {
                for handler in &try_stmt.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(name) = handler.name.as_ref() {
                        self.bound_names.insert(name.id.to_string());
                    }
                }
                crate::transformer::walk_stmt(self, stmt);
            }
            ast::Stmt::Import(import_stmt) => {
                for alias in &import_stmt.names {
                    self.bound_names
                        .insert(import_binding_name(alias).to_string());
                }
            }
            ast::Stmt::ImportFrom(import_stmt) => {
                for alias in &import_stmt.names {
                    if alias.name.as_str() == "*" {
                        continue;
                    }
                    self.bound_names
                        .insert(alias.asname.as_ref().unwrap_or(&alias.name).to_string());
                }
            }
            ast::Stmt::Global(global_stmt) => {
                for name in &global_stmt.names {
                    self.explicit_globals
                        .push((name.id.to_string(), name.range()));
                }
            }
            ast::Stmt::Nonlocal(nonlocal_stmt) => {
                for name in &nonlocal_stmt.names {
                    self.explicit_nonlocals
                        .push((name.id.to_string(), name.range()));
                }
            }
            ast::Stmt::FunctionDef(func_def) => {
                self.bound_names.insert(func_def.name.id.to_string());
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
            }
            ast::Stmt::ClassDef(class_def) => {
                self.bound_names.insert(class_def.name.id.to_string());
                for decorator in &mut class_def.decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = class_def.type_params.as_mut() {
                    self.visit_type_params(type_params);
                }
                if let Some(arguments) = class_def.arguments.as_mut() {
                    self.visit_arguments(arguments);
                }
            }
            _ => crate::transformer::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        match expr {
            ast::Expr::Name(name) if matches!(name.ctx, ExprContext::Store) => {
                self.bound_names.insert(name.id.to_string());
                return;
            }
            ast::Expr::Name(name) if matches!(name.ctx, ExprContext::Load) => {
                let id = name.id.as_str();
                if id != "__class__" && !is_internal_symbol(id) {
                    self.load_names.insert(id.to_string());
                }
                return;
            }
            ast::Expr::Named(named) => {
                collect_bound_target_names(named.target.as_ref(), &mut self.bound_names);
                self.visit_expr(named.value.as_mut());
                return;
            }
            ast::Expr::Lambda(_) | ast::Expr::Generator(_) => return,
            _ => {}
        }
        crate::transformer::walk_expr(self, expr);
    }
}

fn collect_bound_target_names(expr: &ast::Expr, names: &mut HashSet<String>) {
    match expr {
        ast::Expr::Name(name) => {
            names.insert(name.id.to_string());
        }
        ast::Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_bound_target_names(elt, names);
            }
        }
        ast::Expr::List(list) => {
            for elt in &list.elts {
                collect_bound_target_names(elt, names);
            }
        }
        ast::Expr::Starred(starred) => collect_bound_target_names(starred.value.as_ref(), names),
        _ => {}
    }
}

fn import_binding_name(alias: &ast::Alias) -> &str {
    alias.asname.as_ref().map_or_else(
        || alias.name.as_str().split('.').next().unwrap(),
        |asname| asname.as_str(),
    )
}

fn collect_scope_bindings(body: &mut Suite) -> RuffScopeBindingCollector {
    let mut collector = RuffScopeBindingCollector::default();
    collector.visit_body(body);
    collector
}

fn merge_semantic_binding(
    existing: SemanticBindingKind,
    incoming: SemanticBindingKind,
) -> SemanticBindingKind {
    match (existing, incoming) {
        (
            SemanticBindingKind::Global | SemanticBindingKind::Nonlocal,
            SemanticBindingKind::Local,
        ) => existing,
        (
            SemanticBindingKind::Local,
            SemanticBindingKind::Global | SemanticBindingKind::Nonlocal,
        ) => incoming,
        _ => existing,
    }
}

fn set_semantic_binding(
    bindings: &mut HashMap<String, SemanticBindingKind>,
    name: &str,
    binding: SemanticBindingKind,
) {
    let binding = if is_internal_symbol(name) {
        SemanticBindingKind::Local
    } else {
        binding
    };
    if let Some(existing) = bindings.get(name).copied() {
        let merged = merge_semantic_binding(existing, binding);
        if merged != existing {
            bindings.insert(name.to_string(), merged);
        }
    } else {
        bindings.insert(name.to_string(), binding);
    }
}

struct ScopePreparation {
    bindings: HashMap<String, SemanticBindingKind>,
    local_defs: HashSet<String>,
}

struct RuffSemanticSnapshotBuilder {
    semantic: RuffSemanticModel<'static>,
    snapshot: SemanticSnapshot,
    scope_stack: Vec<(SemanticScopeId, RuffScopeId)>,
    implicit_nonlocals_by_scope: HashMap<SemanticScopeId, HashSet<String>>,
    next_node_index: u32,
}

impl RuffSemanticSnapshotBuilder {
    fn build(module: &mut Suite) -> SemanticStateInner {
        let module_for_model = Box::leak(Box::new(module.clone()));
        let module_for_build = Box::leak(Box::new(module.clone()));
        let path = Path::new("<semantic-state>");
        let python_ast: &'static [ast::Stmt] = &*module_for_model;
        let module_info = RuffModule {
            kind: RuffModuleKind::Module,
            source: RuffModuleSource::File(path),
            python_ast,
            name: Some("<semantic-state>"),
        };
        let typing_modules: &[String] = &[];
        let semantic = RuffSemanticModel::new(typing_modules, path, module_info);
        let mut builder = Self {
            semantic,
            snapshot: SemanticSnapshot {
                scopes: vec![SemanticScopeData {
                    kind: SemanticScopeKind::Module,
                    bindings: HashMap::new(),
                    local_defs: HashSet::new(),
                    local_cell_bindings: HashSet::new(),
                    parent: None,
                    qualname: String::new(),
                    reuses_child_scopes: false,
                    function_children: HashMap::new(),
                    class_children: HashMap::new(),
                }],
            },
            scope_stack: vec![(SemanticScopeId(0), RuffScopeId::global())],
            implicit_nonlocals_by_scope: HashMap::new(),
            next_node_index: 1,
        };

        let module_preparation = builder.prepare_current_scope(module_for_build, &[]);
        {
            let module_scope = builder.snapshot.scope_mut(SemanticScopeId(0));
            module_scope.bindings = module_preparation.bindings;
            module_scope.local_defs = module_preparation.local_defs;
        }
        builder.visit_body(module_for_build);
        builder.propagate_nonlocal_roots();
        builder.compute_local_cell_bindings();

        SemanticStateInner {
            snapshot: builder.snapshot,
        }
    }

    fn current_ids(&self) -> (SemanticScopeId, RuffScopeId) {
        *self
            .scope_stack
            .last()
            .expect("missing current semantic scope")
    }

    fn ensure_node_index<T: HasNodeIndex>(&mut self, node: &T) -> NodeIndex {
        let node_index = node.node_index().load();
        if node_index != NodeIndex::NONE {
            if let Some(value) = node_index.as_u32() {
                self.next_node_index = self.next_node_index.max(value + 1);
            }
            return node_index;
        }

        let index = NodeIndex::from(self.next_node_index);
        self.next_node_index += 1;
        node.node_index().set(index);
        index
    }

    fn prepare_current_scope(
        &mut self,
        body: &mut Suite,
        parameters: &[(String, TextRange)],
    ) -> ScopePreparation {
        let collector = collect_scope_bindings(body);
        let explicit_globals = collector
            .explicit_globals
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<HashSet<_>>();
        let explicit_nonlocals = collector
            .explicit_nonlocals
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<HashSet<_>>();

        for (name, range) in collector.explicit_globals {
            if !self.semantic.scope_id.is_global() {
                let global_binding = self.semantic.global_scope().get(name.as_str());
                let binding_id = self.semantic.push_binding(
                    range,
                    RuffBindingKind::Global(global_binding),
                    RuffBindingFlags::GLOBAL,
                );
                let leaked_name = Box::leak(name.into_boxed_str());
                self.semantic
                    .current_scope_mut()
                    .add(leaked_name, binding_id);
            }
        }
        for (name, range) in collector.explicit_nonlocals {
            if let Some((scope_id, binding_id)) = self.semantic.nonlocal(name.as_str()) {
                let binding_id = self.semantic.push_binding(
                    range,
                    RuffBindingKind::Nonlocal(binding_id, scope_id),
                    RuffBindingFlags::NONLOCAL,
                );
                let leaked_name = Box::leak(name.into_boxed_str());
                self.semantic
                    .current_scope_mut()
                    .add(leaked_name, binding_id);
            }
        }
        for (name, range) in parameters {
            let binding_id = self.semantic.push_binding(
                *range,
                RuffBindingKind::Argument,
                RuffBindingFlags::empty(),
            );
            let leaked_name = Box::leak(name.clone().into_boxed_str());
            self.semantic
                .current_scope_mut()
                .add(leaked_name, binding_id);
        }
        for name in collector.bound_names.iter() {
            if self.semantic.current_scope().has(name.as_str()) {
                continue;
            }
            let binding_id = self.semantic.push_binding(
                TextRange::default(),
                RuffBindingKind::Assignment,
                RuffBindingFlags::empty(),
            );
            let leaked_name = Box::leak(name.clone().into_boxed_str());
            self.semantic
                .current_scope_mut()
                .add(leaked_name, binding_id);
        }

        let mut bindings = HashMap::new();
        let mut local_defs = HashSet::new();
        for name in &explicit_globals {
            set_semantic_binding(&mut bindings, name, SemanticBindingKind::Global);
        }
        for name in &explicit_nonlocals {
            set_semantic_binding(&mut bindings, name, SemanticBindingKind::Nonlocal);
        }
        for (name, _) in parameters {
            set_semantic_binding(&mut bindings, name.as_str(), SemanticBindingKind::Local);
            if !explicit_globals.contains(name) && !explicit_nonlocals.contains(name) {
                local_defs.insert(name.clone());
            }
        }
        for name in &collector.bound_names {
            set_semantic_binding(&mut bindings, name.as_str(), SemanticBindingKind::Local);
            if !explicit_globals.contains(name) && !explicit_nonlocals.contains(name) {
                local_defs.insert(name.clone());
            }
        }
        for name in collector.load_names {
            if bindings.contains_key(name.as_str()) {
                continue;
            }
            if self.resolves_to_enclosing_function(name.as_str()) {
                set_semantic_binding(&mut bindings, name.as_str(), SemanticBindingKind::Nonlocal);
                self.implicit_nonlocals_by_scope
                    .entry(self.current_ids().0)
                    .or_default()
                    .insert(name);
            }
        }

        ScopePreparation {
            bindings,
            local_defs,
        }
    }

    fn resolves_to_enclosing_function(&self, name: &str) -> bool {
        self.semantic
            .scopes
            .ancestor_ids(self.semantic.scope_id)
            .find_map(|scope_id| match self.semantic.scopes[scope_id].kind {
                RuffScopeKind::Function(_) => self.semantic.scopes[scope_id]
                    .get(name)
                    .map(|binding_id| !self.semantic.binding(binding_id).is_global()),
                RuffScopeKind::Module => Some(false),
                RuffScopeKind::Class(_) => None,
                _ => None,
            })
            .unwrap_or(false)
    }

    fn push_snapshot_scope(
        &mut self,
        kind: SemanticScopeKind,
        name: &str,
        node_index: NodeIndex,
        preparation: ScopePreparation,
    ) -> SemanticScopeId {
        let parent_id = self.current_ids().0;
        let qualname = child_qualname(self.snapshot.scope(parent_id), name);
        let scope_id = SemanticScopeId(self.snapshot.scopes.len());
        self.snapshot.scopes.push(SemanticScopeData {
            kind,
            bindings: preparation.bindings,
            local_defs: preparation.local_defs,
            local_cell_bindings: HashSet::new(),
            parent: Some(parent_id),
            qualname,
            reuses_child_scopes: false,
            function_children: HashMap::new(),
            class_children: HashMap::new(),
        });
        match kind {
            SemanticScopeKind::Function => {
                self.snapshot
                    .scope_mut(parent_id)
                    .function_children
                    .insert(node_index, scope_id);
            }
            SemanticScopeKind::Class => {
                self.snapshot
                    .scope_mut(parent_id)
                    .class_children
                    .insert(node_index, scope_id);
            }
            SemanticScopeKind::Module => {}
        }
        scope_id
    }

    fn propagate_nonlocal_roots(&mut self) {
        let scope_ids = (0..self.snapshot.scopes.len())
            .map(SemanticScopeId)
            .collect::<Vec<_>>();
        for scope_id in scope_ids {
            let nonlocals = self
                .snapshot
                .scope(scope_id)
                .bindings
                .iter()
                .filter_map(|(name, kind)| {
                    if matches!(kind, SemanticBindingKind::Nonlocal) {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            for name in nonlocals {
                let mut current = self.snapshot.scope(scope_id).parent;
                while let Some(parent_id) = current {
                    let parent = self.snapshot.scope(parent_id);
                    if matches!(parent.kind, SemanticScopeKind::Function)
                        && matches!(
                            parent.bindings.get(name.as_str()),
                            Some(SemanticBindingKind::Local)
                        )
                    {
                        set_semantic_binding(
                            &mut self.snapshot.scope_mut(parent_id).bindings,
                            name.as_str(),
                            SemanticBindingKind::Nonlocal,
                        );
                        break;
                    }
                    current = parent.parent;
                }
            }
        }
    }

    fn descendant_uses_nonlocal(&self, scope_id: SemanticScopeId, name: &str) -> bool {
        let scope = self.snapshot.scope(scope_id);
        let child_ids = scope
            .function_children
            .values()
            .chain(scope.class_children.values())
            .copied()
            .collect::<Vec<_>>();
        for child_id in child_ids {
            if matches!(
                self.snapshot.scope(child_id).bindings.get(name),
                Some(SemanticBindingKind::Nonlocal)
            ) {
                return true;
            }
            if self.descendant_uses_nonlocal(child_id, name) {
                return true;
            }
        }
        false
    }

    fn compute_local_cell_bindings(&mut self) {
        let scope_ids = (0..self.snapshot.scopes.len())
            .map(SemanticScopeId)
            .collect::<Vec<_>>();
        for scope_id in scope_ids {
            let local_defs = self.snapshot.scope(scope_id).local_defs.clone();
            let local_cell_bindings = local_defs
                .into_iter()
                .filter(|name| self.descendant_uses_nonlocal(scope_id, name.as_str()))
                .collect::<HashSet<_>>();
            self.snapshot.scope_mut(scope_id).local_cell_bindings = local_cell_bindings;
        }
    }
}

impl Transformer for RuffSemanticSnapshotBuilder {
    fn visit_stmt(&mut self, stmt: &mut ast::Stmt) {
        match stmt {
            ast::Stmt::FunctionDef(func_def) => {
                let node_index = self.ensure_node_index(func_def);
                let leaked_func = Box::leak(Box::new(func_def.clone()));
                self.semantic
                    .push_scope(RuffScopeKind::Function(leaked_func));
                let parameters = parameter_refs(&func_def.parameters);
                let preparation = self.prepare_current_scope(&mut func_def.body, &parameters);
                let scope_id = self.push_snapshot_scope(
                    SemanticScopeKind::Function,
                    func_def.name.id.as_str(),
                    node_index,
                    preparation,
                );
                let ruff_scope_id = self.semantic.scope_id;
                self.scope_stack.push((scope_id, ruff_scope_id));
                self.visit_body(&mut func_def.body);
                self.scope_stack.pop();
                self.semantic.pop_scope();
            }
            ast::Stmt::ClassDef(class_def) => {
                let node_index = self.ensure_node_index(class_def);
                let leaked_class = Box::leak(Box::new(class_def.clone()));
                self.semantic.push_scope(RuffScopeKind::Class(leaked_class));
                let preparation = self.prepare_current_scope(&mut class_def.body, &[]);
                let scope_id = self.push_snapshot_scope(
                    SemanticScopeKind::Class,
                    class_def.name.id.as_str(),
                    node_index,
                    preparation,
                );
                let ruff_scope_id = self.semantic.scope_id;
                self.scope_stack.push((scope_id, ruff_scope_id));
                self.visit_body(&mut class_def.body);
                self.scope_stack.pop();
                self.semantic.pop_scope();
            }
            _ => crate::transformer::walk_stmt(self, stmt),
        }
    }
}

fn parameter_refs(parameters: &ast::Parameters) -> Vec<(String, TextRange)> {
    let mut refs = Vec::new();
    for parameter in &parameters.posonlyargs {
        refs.push((
            parameter.parameter.name.id.to_string(),
            parameter.parameter.range(),
        ));
    }
    for parameter in &parameters.args {
        refs.push((
            parameter.parameter.name.id.to_string(),
            parameter.parameter.range(),
        ));
    }
    if let Some(vararg) = parameters.vararg.as_ref() {
        refs.push((vararg.name.id.to_string(), vararg.range()));
    }
    for parameter in &parameters.kwonlyargs {
        refs.push((
            parameter.parameter.name.id.to_string(),
            parameter.parameter.range(),
        ));
    }
    if let Some(kwarg) = parameters.kwarg.as_ref() {
        refs.push((kwarg.name.id.to_string(), kwarg.range()));
    }
    refs
}

impl SemanticAstState {
    pub(crate) fn from_ruff(module: &mut Suite) -> Self {
        let next_node_index = ensure_node_indices_for_suite(module);
        let inner = Arc::new(RuffSemanticSnapshotBuilder::build(module));
        let mut provenance = SemanticProvenance::default();
        provenance.next_node_index = next_node_index;
        Self {
            inner,
            provenance: Arc::new(Mutex::new(provenance)),
        }
    }

    fn scope(&self, scope_id: SemanticScopeId) -> SemanticScope {
        SemanticScope::new(self.clone(), scope_id)
    }

    fn function_scope_id(&self, func_def: &StmtFunctionDef) -> Option<SemanticScopeId> {
        if let Some(scope_id) = self
            .provenance
            .lock()
            .expect("semantic provenance mutex poisoned")
            .function_scope_override(func_def)
        {
            return Some(scope_id);
        }
        self.inner
            .snapshot
            .scope(SemanticScopeId(0))
            .function_children
            .get(&func_def.node_index().load())
            .copied()
            .or_else(|| {
                self.inner.snapshot.scopes.iter().find_map(|scope| {
                    scope
                        .function_children
                        .get(&func_def.node_index().load())
                        .copied()
                })
            })
    }

    pub(crate) fn module_scope(&self) -> SemanticScope {
        self.scope(SemanticScopeId(0))
    }

    pub(crate) fn synthesize_module_init_scope(
        &mut self,
        func_def: &StmtFunctionDef,
    ) -> SemanticScope {
        let module_scope = self.module_scope();
        let module_data = module_scope.data().clone();
        let translated_bindings = module_data
            .bindings
            .into_iter()
            .map(|(name, binding)| {
                let translated = if is_internal_symbol(name.as_str()) {
                    SemanticBindingKind::Local
                } else {
                    match binding {
                        SemanticBindingKind::Local => SemanticBindingKind::Global,
                        SemanticBindingKind::Nonlocal => SemanticBindingKind::Nonlocal,
                        SemanticBindingKind::Global => SemanticBindingKind::Global,
                    }
                };
                (name, translated)
            })
            .collect::<HashMap<_, _>>();

        let scope_id = {
            let inner = Arc::make_mut(&mut self.inner);
            let scope_id = SemanticScopeId(inner.snapshot.scopes.len());
            inner.snapshot.scopes.push(SemanticScopeData {
                kind: SemanticScopeKind::Function,
                bindings: translated_bindings,
                local_defs: HashSet::new(),
                local_cell_bindings: HashSet::new(),
                parent: Some(module_scope.scope_id),
                qualname: "_dp_module_init".to_string(),
                reuses_child_scopes: true,
                function_children: module_scope.data().function_children.clone(),
                class_children: module_scope.data().class_children.clone(),
            });
            scope_id
        };

        let scope = self.scope(scope_id);
        self.register_function_scope_override(func_def, scope.clone());
        scope
    }

    pub(crate) fn register_function_scope_override(
        &mut self,
        func_def: &StmtFunctionDef,
        scope: SemanticScope,
    ) {
        let mut provenance = self
            .provenance
            .lock()
            .expect("semantic provenance mutex poisoned");
        let node_index = provenance.ensure_node_index(func_def);
        provenance
            .function_scope_overrides
            .insert(node_index, scope.scope_id);
    }

    pub(crate) fn function_scope(&self, func_def: &StmtFunctionDef) -> Option<SemanticScope> {
        self.function_scope_id(func_def)
            .map(|scope_id| self.scope(scope_id))
    }

    pub(crate) fn has_function_scope_override(&self, func_def: &StmtFunctionDef) -> bool {
        self.provenance
            .lock()
            .expect("semantic provenance mutex poisoned")
            .function_scope_override(func_def)
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SemanticAstState, SemanticBindingKind, SemanticBindingUse, SemanticScope, SemanticScopeKind,
    };
    use crate::passes::ast_to_ast::body::suite_ref;
    use crate::passes::ast_to_ast::context::Context;
    use crate::passes::ast_to_ast::rewrite_class_def::class_body::rewrite_class_body_scopes;
    use crate::passes::ast_to_ast::Options;
    use crate::transform_str_to_ruff_with_options;
    use ruff_python_ast::{self as ast, Stmt};
    use ruff_python_parser::parse_module;

    fn parse_module_body(source: &str) -> Vec<Stmt> {
        parse_module(source).unwrap().into_syntax().body
    }

    fn find_function<'a>(body: &'a [Stmt], name: &str) -> &'a ast::StmtFunctionDef {
        for stmt in body {
            if let Stmt::FunctionDef(func_def) = stmt {
                if func_def.name.id.as_str() == name {
                    return func_def;
                }
            }
        }
        panic!("function {name} not found");
    }

    fn find_class<'a>(body: &'a [Stmt], name: &str) -> &'a ast::StmtClassDef {
        for stmt in body {
            if let Stmt::ClassDef(class_def) = stmt {
                if class_def.name.id.as_str() == name {
                    return class_def;
                }
            }
        }
        panic!("class {name} not found");
    }

    fn find_class_recursive<'a>(body: &'a [Stmt], name: &str) -> Option<&'a ast::StmtClassDef> {
        for stmt in body {
            match stmt {
                Stmt::ClassDef(class_def) if class_def.name.id.as_str() == name => {
                    return Some(class_def);
                }
                Stmt::If(if_stmt) => {
                    if let Some(found) = find_class_recursive(suite_ref(&if_stmt.body), name) {
                        return Some(found);
                    }
                    for clause in &if_stmt.elif_else_clauses {
                        if let Some(found) = find_class_recursive(suite_ref(&clause.body), name) {
                            return Some(found);
                        }
                    }
                }
                Stmt::For(for_stmt) => {
                    if let Some(found) = find_class_recursive(suite_ref(&for_stmt.body), name) {
                        return Some(found);
                    }
                    if let Some(found) = find_class_recursive(suite_ref(&for_stmt.orelse), name) {
                        return Some(found);
                    }
                }
                Stmt::While(while_stmt) => {
                    if let Some(found) = find_class_recursive(suite_ref(&while_stmt.body), name) {
                        return Some(found);
                    }
                    if let Some(found) = find_class_recursive(suite_ref(&while_stmt.orelse), name) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn function_scope<'a>(
        state: &'a SemanticAstState,
        func_def: &ast::StmtFunctionDef,
    ) -> SemanticScope {
        state
            .function_scope(func_def)
            .expect("missing function scope")
    }

    #[test]
    fn semantic_state_keeps_class_helper_scope_overrides_transformable() {
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
        let mut semantic_state = SemanticAstState::from_ruff(&mut module);
        rewrite_class_body_scopes(&context, &mut semantic_state, &mut module);
    }

    #[test]
    fn semantic_state_module_bindings_include_assignments() {
        let mut body = parse_module_body("x = 1\ny = 2\n");
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let scope = semantic_state.module_scope();
        assert_eq!(
            scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Local
        );
        assert_eq!(
            scope.binding_in_scope("y", SemanticBindingUse::Load),
            SemanticBindingKind::Local
        );
    }

    #[test]
    fn synthesized_module_init_scope_reuses_module_children_and_translates_bindings() {
        let mut body = parse_module_body(concat!(
            "x = 1\n",
            "def f():\n",
            "    return x\n",
            "class C:\n",
            "    y = x\n",
        ));
        let mut semantic_state = SemanticAstState::from_ruff(&mut body);
        let module_init: ast::StmtFunctionDef = crate::py_stmt_typed!(
            r#"
def _dp_module_init():
    pass
"#
        );
        let module_init_scope = semantic_state.synthesize_module_init_scope(&module_init);

        assert_eq!(
            module_init_scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Global
        );
        assert_eq!(
            module_init_scope.binding_in_scope("f", SemanticBindingUse::Load),
            SemanticBindingKind::Global
        );
        assert!(module_init_scope
            .child_scope_for_function(find_function(&body, "f"))
            .is_some());
        assert!(module_init_scope
            .child_scope_for_class(find_class(&body, "C"))
            .is_some());
    }

    #[test]
    fn semantic_state_function_scope_tracks_parameters_and_globals() {
        let mut body = parse_module_body(concat!(
            "x = 0\n",
            "def f(a, b, *args, c=1, **kwargs):\n",
            "    global x\n",
            "    x = a\n",
            "    y = b\n",
        ));
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let func_scope = function_scope(&semantic_state, find_function(&body, "f"));

        for name in ["a", "b", "args", "c", "kwargs", "y"] {
            assert_eq!(
                func_scope.binding_in_scope(name, SemanticBindingUse::Load),
                SemanticBindingKind::Local,
                "{name}"
            );
        }
        assert_eq!(
            func_scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Global
        );
    }

    #[test]
    fn semantic_state_named_expr_in_while_test_binds_local() {
        let mut body = parse_module_body(concat!(
            "def f(values):\n",
            "    while not (value := values[0]):\n",
            "        break\n",
            "    return value\n",
        ));
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let func_scope = function_scope(&semantic_state, find_function(&body, "f"));

        assert_eq!(
            func_scope.binding_in_scope("value", SemanticBindingUse::Load),
            SemanticBindingKind::Local
        );
        assert!(func_scope.local_binding_names().contains("value"));
    }

    #[test]
    fn semantic_state_nested_global_function_def_qualifies_globally() {
        let mut body = parse_module_body(concat!(
            "def build_qualnames():\n",
            "    def global_function():\n",
            "        def inner_function():\n",
            "            global inner_global_function\n",
            "            def inner_global_function():\n",
            "                pass\n",
            "            return inner_global_function\n",
            "        return inner_function()\n",
            "    return global_function()\n",
        ));
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let build_qualnames = find_function(&body, "build_qualnames");
        let global_function = find_function(suite_ref(&build_qualnames.body), "global_function");
        let inner_function = find_function(suite_ref(&global_function.body), "inner_function");
        let inner_scope = function_scope(&semantic_state, inner_function);
        let inner_global_function =
            find_function(suite_ref(&inner_function.body), "inner_global_function");
        let inner_global_scope = function_scope(&semantic_state, inner_global_function);

        assert_eq!(
            inner_scope.binding_in_scope("inner_global_function", SemanticBindingUse::Load),
            SemanticBindingKind::Global
        );
        assert_eq!(inner_global_scope.qualname(), "inner_global_function");
    }

    #[test]
    fn semantic_state_nonlocal_in_child_scopes_is_detected() {
        let mut body = parse_module_body(concat!(
            "def outer():\n",
            "    x = 1\n",
            "    def inner():\n",
            "        nonlocal x\n",
            "        return x\n",
            "    return inner\n",
        ));
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let outer_scope = function_scope(&semantic_state, find_function(&body, "outer"));
        let inner_def = find_function(&find_function(&body, "outer").body, "inner");
        let inner_scope = function_scope(&semantic_state, inner_def);

        assert_eq!(
            inner_scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Nonlocal
        );
        assert_eq!(
            outer_scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Nonlocal
        );
        assert_eq!(
            outer_scope.binding_in_scope("y", SemanticBindingUse::Load),
            SemanticBindingKind::Local
        );
    }

    #[test]
    fn semantic_state_implicit_nonlocal_reads_mark_root_binding() {
        let mut body = parse_module_body(concat!(
            "def outer():\n",
            "    x = 1\n",
            "    def inner():\n",
            "        return x\n",
            "    return inner\n",
        ));
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let outer_scope = function_scope(&semantic_state, find_function(&body, "outer"));
        let inner_def = find_function(&find_function(&body, "outer").body, "inner");
        let inner_scope = function_scope(&semantic_state, inner_def);

        assert_eq!(
            inner_scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Nonlocal
        );
        assert_eq!(
            outer_scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Nonlocal
        );
    }

    #[test]
    fn semantic_state_recursive_local_function_is_tracked_as_cell_binding() {
        let mut body = parse_module_body(concat!(
            "def outer():\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    return recurse\n",
        ));
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let outer_scope = function_scope(&semantic_state, find_function(&body, "outer"));

        assert!(outer_scope.local_cell_bindings().contains("recurse"));
    }

    #[test]
    fn semantic_state_class_scope_has_local_bindings() {
        let mut body = parse_module_body(concat!(
            "class C:\n",
            "    y = 1\n",
            "    def m(self):\n",
            "        z = y\n",
        ));
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let class_scope = semantic_state
            .module_scope()
            .child_scope_for_class(find_class(&body, "C"))
            .expect("missing class scope");

        assert_eq!(class_scope.kind(), SemanticScopeKind::Class);
        assert_eq!(
            class_scope.binding_in_scope("y", SemanticBindingUse::Load),
            SemanticBindingKind::Local
        );
    }

    #[test]
    fn semantic_state_class_scope_marks_enclosing_function_loads_nonlocal() {
        let mut body = parse_module_body(concat!(
            "def outer():\n",
            "    x = 1\n",
            "    class C:\n",
            "        y = x\n",
            "    return C\n",
        ));
        let semantic_state = SemanticAstState::from_ruff(&mut body);
        let outer_scope = function_scope(&semantic_state, find_function(&body, "outer"));
        let class_scope = outer_scope
            .child_scope_for_class(
                find_class_recursive(&find_function(&body, "outer").body, "C")
                    .expect("missing class"),
            )
            .expect("missing class scope");

        assert_eq!(
            class_scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Nonlocal
        );
        assert_eq!(
            outer_scope.binding_in_scope("x", SemanticBindingUse::Load),
            SemanticBindingKind::Nonlocal
        );
    }

    #[test]
    fn semantic_state_keeps_nested_class_binding_shape_transformable() {
        let source = concat!(
            "class Container:\n",
            "    class Member:\n",
            "        pass\n",
            "\n",
            "def get_member():\n",
            "    return getattr(Container, \"Member\", None)\n",
        );
        let _ = transform_str_to_ruff_with_options(source, Options::for_test())
            .expect("transform should succeed");
    }

    #[test]
    fn semantic_state_keeps_genexpr_iter_once_shape_transformable() {
        let source = concat!(
            "class Iterator:\n",
            "    def __next__(self):\n",
            "        raise StopIteration\n",
            "\n",
            "class Iterable:\n",
            "    def __iter__(self):\n",
            "        return Iterator()\n",
            "\n",
            "def run():\n",
            "    return list(x for x in Iterable())\n",
        );
        let _ = transform_str_to_ruff_with_options(source, Options::for_test())
            .expect("transform should succeed");
    }
}

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use ruff_python_ast::{self as ast, Expr, ExprContext, HasNodeIndex, NodeIndex, Stmt};
use ruff_text_size::{Ranged, TextRange};

use crate::body_transform::{Transformer, walk_expr, walk_stmt};
use crate::transform::node_index;


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingKind {
    Local,
    Nonlocal,
    Global,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingUse {
    Load,
    Modify,
}

type ScopeBindings = HashMap<String, BindingKind>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScopeKind {
    Function { name: String },
    Class { name: String },
    Module,
}

#[derive(Debug)]
pub struct ScopeTree {
    inner: Mutex<ScopeTreeInner>,
}

#[derive(Debug)]
struct ScopeTreeInner {
    scopes: Vec<Arc<Scope>>,
    children: Vec<Vec<usize>>,
    node_index_map: HashMap<NodeIndex, usize>,
    range_map: HashMap<TextRange, usize>,
}

impl ScopeTree {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(ScopeTreeInner {
                scopes: Vec::new(),
                children: Vec::new(),
                node_index_map: HashMap::new(),
                range_map: HashMap::new(),
            }),
        })
    }

    fn add_scope(
        self: &Arc<Self>,
        kind: ScopeKind,
        bindings: ScopeBindings,
        parent_id: Option<usize>,
        node_index: Option<NodeIndex>,
        range: Option<TextRange>,
    ) -> Arc<Scope> {
        let mut inner = self.inner.lock().expect("ScopeTree mutex poisoned");
        let id = inner.scopes.len();
        let scope = Arc::new(Scope {
            id,
            parent_id,
            kind,
            bindings,
            tree: Arc::clone(self),
            child_nonlocals: Mutex::new(HashSet::new()),
        });
        inner.scopes.push(Arc::clone(&scope));
        inner.children.push(Vec::new());
        if let Some(parent_id) = parent_id {
            if let Some(children) = inner.children.get_mut(parent_id) {
                children.push(id);
            }
        }
        if let Some(node_index) = node_index {
            if node_index != NodeIndex::NONE {
                inner.node_index_map.insert(node_index, id);
            }
        }
        if let Some(range) = range {
            if !range.is_empty() {
                inner.range_map.insert(range, id);
            }
        }
        scope
    }

    fn get(&self, id: usize) -> Option<Arc<Scope>> {
        self.inner
            .lock()
            .expect("ScopeTree mutex poisoned")
            .scopes
            .get(id)
            .cloned()
    }

    fn child_ids(&self, id: usize) -> Vec<usize> {
        self.inner
            .lock()
            .expect("ScopeTree mutex poisoned")
            .children
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    fn scope_for_def(&self, node_index: NodeIndex, parent_id: Option<usize>) -> Option<Arc<Scope>> {
        let inner = self.inner.lock().expect("ScopeTree mutex poisoned");

        if let Some(id) = inner.node_index_map.get(&node_index).copied() {
            let scope = inner.scopes.get(id).cloned();
            if let Some(expected_parent_id) = parent_id {
                assert_eq!(scope.as_ref().map(|scope| scope.parent_id), Some(Some(expected_parent_id)));
            }
            return scope;
        }

        None
    }
}

#[derive(Debug)]
pub struct Scope {
    id: usize,
    parent_id: Option<usize>,
    kind: ScopeKind,
    bindings: ScopeBindings,
    tree: Arc<ScopeTree>,
    child_nonlocals: Mutex<HashSet<String>>,
}

impl Scope {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn kind(&self) -> &ScopeKind {
        &self.kind
    }

    pub fn make_qualname(&self, func_name: &str) -> String {
        if self.is_global(func_name) {
            return func_name.to_string();
        }
        let mut components = vec![func_name.to_string()];
        let mut current = self.parent_scope();

        while let Some(scope) = current {
            match &scope.kind {
                ScopeKind::Function { name } => {
                    components.push("<locals>".to_string());
                    components.push(name.clone());

                    current = scope.parent_scope();
                }
                ScopeKind::Class { name } => {
                    components.push(name.clone());
                    if scope
                        .parent_scope()
                        .as_ref()
                        .is_some_and(|parent| parent.is_global(name))
                    {
                        break;
                    }
                    current = scope.parent_scope();
                }
                ScopeKind::Module => {
                    break;
                }
            }
        }
        components.reverse();
        components.join(".")
    }


    pub fn scope_bindings(&self) -> &ScopeBindings {
        &self.bindings
    }
    
    pub fn parent_scope(&self) -> Option<Arc<Scope>> {
        self.parent_id.and_then(|id| self.tree.get(id))
    }

    pub fn child_ids(&self) -> Vec<usize> {
        self.tree.child_ids(self.id)
    }

    pub fn child_scope_for_function(&self, func_def: &ast::StmtFunctionDef) -> Arc<Scope> {
        self.lookup_child_scope(func_def).unwrap_or_else(|| {
           panic!("no child scope for function {} {:?}", func_def.name.id.as_str(), func_def.node_index.load());
        })
    }

    pub fn child_scope_for_class(&self, class_def: &ast::StmtClassDef) -> Arc<Scope> {
        self.lookup_child_scope(class_def).unwrap_or_else(|| {
           panic!("no child scope for class {} {:?}", class_def.name.id.as_str(), class_def.node_index.load());
        })
    }

    pub fn child_nonlocal_names(&self) -> HashSet<String> {
        self.child_nonlocals
            .lock()
            .expect("Scope child_nonlocals mutex poisoned")
            .clone()
    }

    pub fn is_nonlocal_in_children(&self, name: &str) -> bool {
        self.child_nonlocals
            .lock()
            .expect("Scope child_nonlocals mutex poisoned")
            .contains(name)
    }

    pub fn binding_in_scope(&self, name: &str, use_kind: BindingUse) -> BindingKind {
        match self.bindings.get(name).copied() {
            Some(binding) => binding,
            None => match use_kind {
                BindingUse::Load => BindingKind::Local,
                BindingUse::Modify => {
                    panic!("Name not found in scope: {} {:?}", name, self)
                }
            },
        }
    }

    pub fn ensure_child_scope_for_function(
        &self,
        func_def: &ast::StmtFunctionDef,
    ) -> Arc<Scope> {
        self.child_scope_for_function(func_def)
    }

    pub fn ensure_child_scope_for_class(&self, class_def: &ast::StmtClassDef) -> Arc<Scope> {
        self.child_scope_for_class(class_def)
    }

    pub(crate) fn local_names(&self) -> HashSet<String> {
        self.collect_by_binding(BindingKind::Local)
    }

    pub(crate) fn global_names(&self) -> HashSet<String> {
        self.collect_by_binding(BindingKind::Global)
    }

    pub(crate) fn nonlocal_names(&self) -> HashSet<String> {
        self.collect_by_binding(BindingKind::Nonlocal)
    }

    fn collect_by_binding(&self, kind: BindingKind) -> HashSet<String> {
        let mut ret = HashSet::new();
        self.binding_like(|bindings| {
            for (name, binding) in bindings {
                if *binding == kind {
                    ret.insert(name.clone());
                }
            }
            None
        }).unwrap_or_default()
    }

    fn binding_like<T>(&self, mut find: impl FnMut(&ScopeBindings) -> Option<T>) -> Option<T> {

        let mut current = self.tree.get(self.id);
        while let Some(current_scope) = current {
            let bindings = current_scope.scope_bindings();
            if let Some(ret) = find(bindings) {
                return Some(ret);
            }
            current = current_scope.parent_scope();
        };
        None
    }

    pub fn is_local(&self, name: &str) -> bool {
        self.binding_is(name, BindingKind::Local)
    }

    pub fn is_global(&self, name: &str) -> bool {
        self.binding_is(name, BindingKind::Global)
    }

    pub fn is_nonlocal(&self, name: &str) -> bool {
        self.binding_is(name, BindingKind::Nonlocal)
    }

    fn binding_is(&self, name: &str, scope: BindingKind) -> bool {
        self.binding_like(|bindings| {
            if let Some(found) = bindings.get(name) {
                Some(*found == scope)
            } else {
                None
            }
        }).unwrap_or(false)
    }

    pub fn lookup_child_scope(
        &self,
        has_node_index: & impl HasNodeIndex,
    ) -> Option<Arc<Scope>> {
        self.tree
            .scope_for_def(has_node_index.node_index().load(), Some(self.id))
    }

}

#[derive(Default)]
struct ScopeCollector {
    bindings: ScopeBindings,
}

fn merge_binding(existing: BindingKind, incoming: BindingKind) -> BindingKind {
    match (existing, incoming) {
        (BindingKind::Global | BindingKind::Nonlocal, BindingKind::Local) => existing,
        (BindingKind::Local, BindingKind::Global | BindingKind::Nonlocal) => incoming,
        _ => existing,
    }
}

fn set_binding(bindings: &mut HashMap<String, BindingKind>, name: &str, binding: BindingKind) {
    if let Some(existing) = bindings.get(name).copied() {
        let merged = merge_binding(existing, binding);
        if merged != existing {
            bindings.insert(name.to_string(), merged);
        }
    } else {
        bindings.insert(name.to_string(), binding);
    }
}

impl ScopeCollector {
    fn add_import_binding(&mut self, alias: &ast::Alias) {
        let name = if let Some(asname) = &alias.asname {
            asname.id.as_str()
        } else {
            alias
                .name
                .id
                .as_str()
                .split('.')
                .next()
                .unwrap_or_else(|| alias.name.id.as_str())
        };
        set_binding(&mut self.bindings, name, BindingKind::Local);
    }
}

impl Transformer for ScopeCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) => {
                set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Local);
                return;
            }
            Stmt::ClassDef(ast::StmtClassDef { name, .. }) => {
                set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Local);
                return;
            }
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                for name in names {
                    set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Global);
                }
                return;
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Nonlocal);
                }
                return;
            }
            Stmt::Import(ast::StmtImport { names, .. }) => {
                for alias in names {
                    self.add_import_binding(alias);
                }
                return;
            }
            Stmt::ImportFrom(ast::StmtImportFrom { names, .. }) => {
                for alias in names {
                    self.add_import_binding(alias);
                }
                return;
            }
            _ => {}
        }

        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, ctx, .. }) if matches!(ctx, ExprContext::Store) => {
                set_binding(&mut self.bindings, id.as_str(), BindingKind::Local);
                return;
            }
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => {
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}


fn collect_scope_info(body: &[Stmt]) -> ScopeBindings {
    let mut collector = ScopeCollector::default();
    let mut cloned_body = body.to_vec();
    collector.visit_body(&mut cloned_body);

    collector.bindings
}

pub fn analyze_module_scope(module: &mut Vec<Stmt>) -> Arc<Scope> {
    node_index::ensure_node_indices(module);
    let tree = ScopeTree::new();
    let bindings = collect_scope_info(module);
    let root = tree.add_scope(ScopeKind::Module, bindings, None, None, None);
    let analyzer = ScopeAnalyzer::new(Arc::clone(&tree));
    analyzer.visit_body(module, root.id);
    annotate_child_nonlocals(&tree);
    root
}

struct ScopeAnalyzer {
    tree: Arc<ScopeTree>,
}

impl ScopeAnalyzer {
    fn new(tree: Arc<ScopeTree>) -> Self {
        Self { tree }
    }

    fn add_function_scope(&self, func_def: &ast::StmtFunctionDef, parent_id: usize) -> Arc<Scope> {
        let mut bindings = function_bindings(func_def);
        let load_names = collect_load_names(&func_def.body);
        for name in load_names {
            if bindings.contains_key(name.as_str()) {
                continue;
            }
            if self.resolves_to_enclosing_function(parent_id, name.as_str()) {
                set_binding(&mut bindings, name.as_str(), BindingKind::Nonlocal);
            }
        }
        let node_index = func_def.node_index.load();
        let range = func_def.range;
        let scope = self.tree.add_scope(
            ScopeKind::Function {
                name: func_def.name.id.to_string(),
            },
            bindings,
            Some(parent_id),
            Some(node_index),
            Some(range),
        );
        self.visit_body(&func_def.body, scope.id);
        scope
    }

    fn add_class_scope(&self, class_def: &ast::StmtClassDef, parent_id: usize) -> Arc<Scope> {
        let bindings = class_bindings(class_def);
        let node_index = class_def.node_index.load();
        let range = class_def.range;
        let scope = self.tree.add_scope(
            ScopeKind::Class {
                name: class_def.name.id.to_string(),
            },
            bindings,
            Some(parent_id),
            Some(node_index),
            Some(range),
        );
        self.visit_body(&class_def.body, scope.id);
        scope
    }

    fn visit_body(&self, body: &[Stmt], parent_id: usize) {
        for stmt in body {
            self.visit_stmt(stmt, parent_id);
        }
    }

    fn resolves_to_enclosing_function(&self, parent_id: usize, name: &str) -> bool {
        let mut current = self.tree.get(parent_id);
        while let Some(scope) = current {
            match scope.kind() {
                ScopeKind::Function { .. } => {
                    if let Some(binding) = scope.scope_bindings().get(name).copied() {
                        match binding {
                            BindingKind::Local => return true,
                            BindingKind::Nonlocal => {
                                current = scope.parent_scope();
                                continue;
                            }
                            BindingKind::Global => return false,
                        }
                    }
                }
                ScopeKind::Module => return false,
                ScopeKind::Class { .. } => {}
            }
            current = scope.parent_scope();
        }
        false
    }

    fn visit_stmt(&self, stmt: &Stmt, parent_id: usize) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                self.add_function_scope(func_def, parent_id);
            }
            Stmt::ClassDef(class_def) => {
                self.add_class_scope(class_def, parent_id);
            }
            Stmt::If(ast::StmtIf { body, elif_else_clauses, .. }) => {
                self.visit_body(body, parent_id);
                for clause in elif_else_clauses {
                    self.visit_body(&clause.body, parent_id);
                }
            }
            Stmt::For(ast::StmtFor { body, orelse, .. }) => {
                self.visit_body(body, parent_id);
                self.visit_body(orelse, parent_id);
            }
            Stmt::While(ast::StmtWhile { body, orelse, .. }) => {
                self.visit_body(body, parent_id);
                self.visit_body(orelse, parent_id);
            }
            Stmt::With(ast::StmtWith { body, .. }) => {
                self.visit_body(body, parent_id);
            }
            Stmt::Match(ast::StmtMatch { cases, .. }) => {
                for case in cases {
                    self.visit_body(&case.body, parent_id);
                }
            }
            Stmt::Try(ast::StmtTry {
                body,
                handlers,
                orelse,
                finalbody,
                ..
            }) => {
                self.visit_body(body, parent_id);
                for handler in handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    self.visit_body(&handler.body, parent_id);
                }
                self.visit_body(orelse, parent_id);
                self.visit_body(finalbody, parent_id);
            }
            _ => {}
        }
    }
}

fn collect_load_names(body: &[Stmt]) -> HashSet<String> {
    #[derive(Default)]
    struct LoadCollector {
        names: HashSet<String>,
    }

    impl Transformer for LoadCollector {
        fn visit_stmt(&mut self, stmt: &mut Stmt) {
            match stmt {
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => return,
                _ => {}
            }
            walk_stmt(self, stmt);
        }

        fn visit_expr(&mut self, expr: &mut Expr) {
            match expr {
                Expr::Name(ast::ExprName { id, ctx, .. })
                    if matches!(ctx, ExprContext::Load) =>
                {
                    self.names.insert(id.as_str().to_string());
                    return;
                }
                Expr::Lambda(_)
                | Expr::Generator(_)
                | Expr::ListComp(_)
                | Expr::SetComp(_)
                | Expr::DictComp(_) => {
                    return;
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }

    let mut collector = LoadCollector::default();
    let mut cloned = body.to_vec();
    collector.visit_body(&mut cloned);
    collector.names
}

fn annotate_child_nonlocals(tree: &Arc<ScopeTree>) {
    let scopes = {
        let inner = tree.inner.lock().expect("ScopeTree mutex poisoned");
        inner.scopes.clone()
    };

    for scope in scopes {
        let nonlocals = scope
            .bindings
            .iter()
            .filter_map(|(name, kind)| {
                if matches!(kind, BindingKind::Nonlocal) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for name in nonlocals {
            let mut current = scope.parent_scope();
            while let Some(parent) = current {
                if matches!(parent.scope_bindings().get(&name), Some(BindingKind::Local))
                    && matches!(parent.kind, ScopeKind::Function { .. })
                {
                    let mut set = parent
                        .child_nonlocals
                        .lock()
                        .expect("Scope child_nonlocals mutex poisoned");
                    set.insert(name.clone());
                    break;
                }
                current = parent.parent_scope();
            }
        }
    }
}

fn function_bindings(func_def: &ast::StmtFunctionDef) -> ScopeBindings {
    let mut bindings = collect_scope_info(&func_def.body);

    let ast::Parameters {
        posonlyargs,
        args,
        vararg,
        kwonlyargs,
        kwarg,
        ..
    } = func_def.parameters.as_ref();

    for param in posonlyargs {
        let name = param.parameter.name.to_string();
        set_binding(&mut bindings, name.as_str(), BindingKind::Local);
    }
    for param in args {
        let name = param.parameter.name.to_string();
        set_binding(&mut bindings, name.as_str(), BindingKind::Local);
    }
    for param in kwonlyargs {
        let name = param.parameter.name.to_string();
        set_binding(&mut bindings, name.as_str(), BindingKind::Local);
    }
    if let Some(param) = vararg {
        let name = param.name.to_string();
        set_binding(&mut bindings, name.as_str(), BindingKind::Local);
    }
    if let Some(param) = kwarg {
        let name = param.name.to_string();
        set_binding(&mut bindings, name.as_str(), BindingKind::Local);
    }

    bindings
}

fn class_bindings(class_def: &ast::StmtClassDef) -> ScopeBindings {
    collect_scope_info(&class_def.body)
}

pub(crate) mod explicit_scope {
    use std::collections::{HashMap, HashSet};

    use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

    use crate::body_transform::{walk_expr, walk_pattern, walk_stmt, Transformer};
    use super::{BindingKind, set_binding, ScopeBindings};


    struct ScopeCollector {
        bindings: ScopeBindings,
        named_expr_nonlocal: bool,
    }

    impl ScopeCollector {
        fn new(named_expr_nonlocal: bool) -> Self {
            Self {
                bindings: HashMap::new(),
                named_expr_nonlocal,
            }
        }
    }


    impl ScopeCollector {
        fn add_import_binding(&mut self, alias: &ast::Alias) {
            let name = alias_binding_name(alias);
            set_binding(&mut self.bindings, name, BindingKind::Local);
        }
    }

    fn is_comprehension_function(name: &str) -> bool {
        name.starts_with("_dp_comp_") || name.starts_with("_dp_gen_")
    }

    fn comprehension_nonlocals(body: &[Stmt]) -> HashSet<String> {
        let mut names = HashSet::new();
        for stmt in body {
            if let Stmt::Nonlocal(ast::StmtNonlocal { names: decls, .. }) = stmt {
                for name in decls {
                    names.insert(name.id.as_str().to_string());
                }
            }
        }
        names
    }

    impl Transformer for ScopeCollector {
        fn visit_stmt(&mut self, stmt: &mut Stmt) {
            match stmt {
                Stmt::FunctionDef(ast::StmtFunctionDef { name, body, .. }) => {
                    set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Local);
                    if is_comprehension_function(name.id.as_str()) {
                        for name in comprehension_nonlocals(body) {
                            set_binding(&mut self.bindings, name.as_str(), BindingKind::Local);
                        }
                    }
                    return;
                }
                Stmt::ClassDef(ast::StmtClassDef { name, .. }) => {
                    set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Local);
                    return;
                }
                Stmt::Global(ast::StmtGlobal { names, .. }) => {
                    for name in names {
                        set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Global);
                    }
                    return;
                }
                Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                    for name in names {
                        set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Nonlocal);
                    }
                    return;
                }
                Stmt::Import(ast::StmtImport { names, .. }) => {
                    for alias in names {
                        self.add_import_binding(alias);
                    }
                    return;
                }
                Stmt::ImportFrom(ast::StmtImportFrom { names, .. }) => {
                    for alias in names {
                        self.add_import_binding(alias);
                    }
                    return;
                }
                Stmt::Try(ast::StmtTry { handlers, .. }) => {
                    for ast::ExceptHandler::ExceptHandler(handler) in handlers {
                        if let Some(name) = &handler.name {
                            set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Local);
                        }
                    }
                }
                _ => {}
            }

            walk_stmt(self, stmt);
        }

        fn visit_expr(&mut self, expr: &mut Expr) {
            match expr {
                Expr::Named(ast::ExprNamed { target, value, .. }) => {
                    if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                        let binding = if self.named_expr_nonlocal {
                            BindingKind::Nonlocal
                        } else {
                            BindingKind::Local
                        };
                        set_binding(&mut self.bindings, id.as_str(), binding);
                    } else {
                        self.visit_expr(target);
                    }
                    self.visit_expr(value);
                    return;
                }
                Expr::Name(ast::ExprName { id, ctx, .. }) if matches!(ctx, ExprContext::Store) => {
                    set_binding(&mut self.bindings, id.as_str(), BindingKind::Local);
                    return;
                }
                Expr::Lambda(_)
                | Expr::Generator(_)
                | Expr::ListComp(_)
                | Expr::SetComp(_)
                | Expr::DictComp(_) => {
                    return;
                }
                _ => {}
            }

            walk_expr(self, expr);
        }

        fn visit_pattern(&mut self, pattern: &mut ast::Pattern) {
            match pattern {
                ast::Pattern::MatchStar(ast::PatternMatchStar { name, .. }) => {
                    if let Some(name) = name.as_ref() {
                        let name_str = name.id.as_str();
                        if name_str != "_" {
                            set_binding(&mut self.bindings, name_str, BindingKind::Local);
                        }
                    }
                }
                ast::Pattern::MatchAs(ast::PatternMatchAs { name, pattern, .. }) => {
                    if let Some(name) = name.as_ref() {
                        let name_str = name.id.as_str();
                        if name_str != "_" {
                            set_binding(&mut self.bindings, name_str, BindingKind::Local);
                        }
                    }
                    if let Some(pattern) = pattern {
                        self.visit_pattern(pattern);
                    }
                    return;
                }
                ast::Pattern::MatchMapping(ast::PatternMatchMapping { rest, patterns, .. }) => {
                    if let Some(rest) = rest.as_ref() {
                        let name_str = rest.id.as_str();
                        if name_str != "_" {
                            set_binding(&mut self.bindings, name_str, BindingKind::Local);
                        }
                    }
                    for pattern in patterns {
                        self.visit_pattern(pattern);
                    }
                    return;
                }
                _ => {}
            }

            walk_pattern(self, pattern);
        }
    }

    pub(crate) fn collect_scope_info(body: &[Stmt]) -> ScopeBindings {
        collect_scope_info_with_named_expr(body, false)
    }

    pub(crate) fn collect_scope_info_with_named_expr(
        body: &[Stmt],
        named_expr_nonlocal: bool,
    ) -> ScopeBindings {
        let mut collector = ScopeCollector::new(named_expr_nonlocal);
        let mut cloned_body = body.to_vec();
        collector.visit_body(&mut cloned_body);
        collector.bindings
    }

    pub(crate) fn alias_binding_name(alias: &ast::Alias) -> &str {
        if let Some(asname) = &alias.asname {
            asname.id.as_str()
        } else {
            alias
                .name
                .id
                .as_str()
                .split('.')
                .next()
                .unwrap_or_else(|| alias.name.id.as_str())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ruff_python_parser::parse_module;

    fn parse_module_body(source: &str) -> Vec<Stmt> {
        parse_module(source)
            .expect("parse failure")
            .into_syntax()
            .body
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

    #[test]
    fn module_bindings_include_assignments() {
        let mut body = parse_module_body("x = 1\ny = 2\n");
        let scope = analyze_module_scope(&mut body);
        assert!(scope.is_local("x"));
        assert!(scope.is_local("y"));
        assert!(!scope.is_global("x"));
    }

    #[test]
    fn function_scope_tracks_parameters_and_globals() {
        let mut body = parse_module_body(concat!(
            "x = 0\n",
            "def f(a, b, *args, c=1, **kwargs):\n",
            "    global x\n",
            "    x = a\n",
            "    y = b\n",
        ));
        let module_scope = analyze_module_scope(&mut body);
        let func_def = find_function(&body, "f");
        let func_scope = module_scope
            .lookup_child_scope(func_def)
            .expect("missing function scope");

        assert!(func_scope.is_local("a"));
        assert!(func_scope.is_local("b"));
        assert!(func_scope.is_local("args"));
        assert!(func_scope.is_local("c"));
        assert!(func_scope.is_local("kwargs"));
        assert!(func_scope.is_global("x"));
        assert!(func_scope.is_local("y"));
    }

    #[test]
    fn nonlocal_in_child_scopes_is_detected() {
        let mut body = parse_module_body(concat!(
            "def outer():\n",
            "    x = 1\n",
            "    def inner():\n",
            "        nonlocal x\n",
            "        return x\n",
            "    return inner\n",
        ));
        let module_scope = analyze_module_scope(&mut body);
        let outer_def = find_function(&body, "outer");
        let outer_scope = module_scope
            .lookup_child_scope(outer_def)
            .expect("missing outer scope");
        let inner_def = find_function(&outer_def.body, "inner");
        let inner_scope = outer_scope
            .lookup_child_scope(inner_def)
            .expect("missing inner scope");

        assert!(inner_scope.is_nonlocal("x"));
        assert!(outer_scope.is_nonlocal_in_children("x"));
        assert!(!outer_scope.is_nonlocal_in_children("y"));
    }

    #[test]
    fn class_scope_has_local_bindings() {
        let mut body = parse_module_body(concat!(
            "class C:\n",
            "    y = 1\n",
            "    def m(self):\n",
            "        z = y\n",
        ));
        let module_scope = analyze_module_scope(&mut body);
        let class_def = find_class(&body, "C");
        let class_scope = module_scope
            .lookup_child_scope(class_def)
            .expect("missing class scope");
        assert!(class_scope.is_local("y"));
        assert!(!class_scope.is_global("y"));
    }

    #[test]
    fn module_children_track_top_level_defs() {
        let mut body = parse_module_body(concat!(
            "def f():\n",
            "    return 1\n",
            "class C:\n",
            "    pass\n",
        ));
        let module_scope = analyze_module_scope(&mut body);
        let mut names = module_scope
            .child_ids()
            .into_iter()
            .filter_map(|id| module_scope.tree.get(id))
            .map(|scope| match &scope.kind {
                ScopeKind::Function { name } => format!("func:{name}"),
                ScopeKind::Class { name } => format!("class:{name}"),
                ScopeKind::Module => "module".to_string(),
            })
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, vec!["class:C".to_string(), "func:f".to_string()]);
    }
}

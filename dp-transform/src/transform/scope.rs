use std::cell::{Ref, RefCell};
use anyhow::{Result, anyhow, Context};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use ruff_python_ast::{self as ast, Expr, ExprContext, HasNodeIndex, NodeIndex, Stmt, StmtBody};


use crate::transformer::{Transformer, walk_expr, walk_stmt};
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScopeKind {
    Function,
    Class,
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
    next_node_index: u32,
}

impl ScopeTree {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(ScopeTreeInner {
                scopes: Vec::new(),
                children: Vec::new(),
                node_index_map: HashMap::new(),
                next_node_index: 1,
            }),
        })
    }

    pub fn child_scope_for_function(&self, func_def: &ast::StmtFunctionDef) -> Result<Arc<Scope>> {
        Ok(self.scope_for_def(func_def)
            .with_context(|| format!("no child scope for function {}", func_def.name.id.as_str()))?)
    }

    pub fn child_scope_for_class(&self, class_def: &ast::StmtClassDef) -> Result<Arc<Scope>> {
        Ok(self.scope_for_def(class_def)
            .with_context(|| format!("no child scope for class {}", class_def.name.id.as_str()))?)
    }


    fn add_scope(
        self: &Arc<Self>,
        kind: ScopeKind,
        name: &str,
        bindings: ScopeBindings,
        explicit_nonlocals: HashSet<String>,
        local_defs: HashSet<String>,
        parent: Option<Arc<Scope>>,
        node_index: Option<NodeIndex>,
    ) -> Arc<Scope> {
        let mut inner = self.inner.lock().expect("ScopeTree mutex poisoned");
        let id = inner.scopes.len();
        let parent_id = parent.as_ref().map(|id| id.id);
        let qualnamer = match &parent {
            Some(ps) => {
                let is_global = matches!(
                    ps.scope_bindings().get(name),
                    Some(BindingKind::Global)
                );
                if is_global {
                    QualNamer::new().enter_scope(kind, name.to_string())
                } else {
                    ps.qualnamer.enter_scope(kind, name.to_string())
                }
            }
            None => QualNamer::new(),
        };

        if let Some(ps) = parent {
            ps.inner.borrow_mut().child_ids.push(id);
        }

        let scope = Arc::new(Scope {
            id,
            parent_id,
            kind,
            tree: Arc::clone(self),
            qualnamer,
            inner: RefCell::new(ScopeInner {
                bindings,
                explicit_nonlocals,
                local_defs,
                child_ids: Vec::new(),
            }),
        });
        inner.scopes.push(Arc::clone(&scope));
        inner.children.push(Vec::new());

        if let Some(node_index) = node_index {
            if node_index != NodeIndex::NONE {
                inner.node_index_map.insert(node_index, id);
                if let Some(value) = node_index.as_u32() {
                    inner.next_node_index = inner.next_node_index.max(value + 1);
                }
            }
        }
        scope
    }

    fn ensure_node_index<T: HasNodeIndex>(&self, node: &T) -> NodeIndex {
        let mut inner = self.inner.lock().expect("ScopeTree mutex poisoned");
        let node_index = node.node_index().load();
        if node_index != NodeIndex::NONE {
            if inner.node_index_map.contains_key(&node_index) {
                let index = NodeIndex::from(inner.next_node_index);
                inner.next_node_index += 1;
                node.node_index().set(index);
                return index;
            }
            if let Some(value) = node_index.as_u32() {
                inner.next_node_index = inner.next_node_index.max(value + 1);
            }
            return node_index;
        }

        let index = NodeIndex::from(inner.next_node_index);
        inner.next_node_index += 1;
        node.node_index().set(index);
        index
    }

    fn get(&self, id: usize) -> Option<Arc<Scope>> {
        self.inner
            .lock()
            .expect("ScopeTree mutex poisoned")
            .scopes
            .get(id)
            .cloned()
    }

    pub fn scope_for_def(&self, has_node_index: &impl HasNodeIndex) -> Result<Arc<Scope>> {
        let inner = self.inner.lock().expect("ScopeTree mutex poisoned");

        if let Some(id) = inner.node_index_map.get(&has_node_index.node_index().load()).copied() {
            if let Some(scope) = inner.scopes.get(id).cloned() {
                return Ok(scope);
            }
        } 
        Err(anyhow!("no scope for {:?}", has_node_index.node_index().load()))
    }
}


#[derive(Debug)]
struct ScopeInner {
    bindings: ScopeBindings,
    explicit_nonlocals: HashSet<String>,
    local_defs: HashSet<String>,
    child_ids: Vec<usize>,
}

#[derive(Debug)]
pub struct Scope {
    id: usize,
    parent_id: Option<usize>,
    kind: ScopeKind,
    pub tree: Arc<ScopeTree>,
    pub qualnamer: QualNamer,
    inner: RefCell<ScopeInner>,
}

impl Scope {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn kind(&self) -> &ScopeKind {
        &self.kind
    }

    pub fn child_ids<'a>(&'a self) -> Ref<'a, Vec<usize>> {
        Ref::map(self.inner.borrow(), |inner| &inner.child_ids)
    }


    pub fn scope_bindings<'a>(&'a self) -> Ref<'a, ScopeBindings> {
        Ref::map(self.inner.borrow(), |inner| &inner.bindings)
    }

    pub fn is_explicit_nonlocal(&self, name: &str) -> bool {
        self.inner.borrow().explicit_nonlocals.contains(name)
    }

    pub fn is_local_definition(&self, name: &str) -> bool {
        self.inner.borrow().local_defs.contains(name)
    }
    
    pub fn parent_scope(&self) -> Option<Arc<Scope>> {
        self.parent_id.and_then(|id| self.tree.get(id))
    }

    pub fn any_parent_scope<T>(&self, mut func: impl FnMut(&Scope) -> Option<T>) -> Option<T> {
        if let Some(ret) = func(self) {
            return Some(ret);
        }
        self.parent_scope().and_then(|parent| parent.any_parent_scope(func))
    }

    pub fn child_scope_for_function(&self, func_def: &ast::StmtFunctionDef) -> Result<Arc<Scope>> {
        self.tree.child_scope_for_function(func_def)
    }

    pub fn child_scope_for_class(&self, class_def: &ast::StmtClassDef) -> Result<Arc<Scope>> {
        Ok(self.lookup_child_scope(class_def)
            .with_context(|| format!("no child scope for class {}", class_def.name.id.as_str()))?)
    }

    pub fn binding_in_scope(&self, name: &str, use_kind: BindingUse) -> BindingKind {
        match self.scope_bindings().get(name).copied() {
            Some(binding) => binding,
            None => match use_kind {
                BindingUse::Load => BindingKind::Local,
                BindingUse::Modify => {
                    panic!("Name not found in scope: {} {:?}", name, self)
                }
            },
        }
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

    fn binding_is(&self, name: &str, kind: BindingKind) -> bool {
        self.any_parent_scope(|scope| {
            if let Some(found) = scope.scope_bindings().get(name) {
                Some(*found == kind)
            } else {
                None
            }
        }).unwrap_or(false)
    }

    pub fn lookup_child_scope(
        &self,
        has_node_index: & impl HasNodeIndex,
    ) -> Result<Arc<Scope>> {
        let child_scope = self.tree
            .scope_for_def(has_node_index)?;
        assert_eq!(Some(self.id), child_scope.parent_id);
        Ok(child_scope)        
    }

}

#[derive(Default)]
struct ScopeCollector {
    bindings: ScopeBindings,
    explicit_nonlocals: HashSet<String>,
    explicit_globals: HashSet<String>,
    local_defs: HashSet<String>,
}

struct ScopeInfo {
    bindings: ScopeBindings,
    explicit_nonlocals: HashSet<String>,
    local_defs: HashSet<String>,
}

fn bind_target_names(
    bindings: &mut ScopeBindings,
    local_defs: &mut HashSet<String>,
    explicit_globals: &HashSet<String>,
    explicit_nonlocals: &HashSet<String>,
    target: &Expr,
) {
    match target {
        Expr::Name(ast::ExprName { id, .. }) => {
            set_binding(bindings, id.as_str(), BindingKind::Local);
            if !explicit_globals.contains(id.as_str())
                && !explicit_nonlocals.contains(id.as_str())
            {
                local_defs.insert(id.to_string());
            }
        }
        Expr::Tuple(ast::ExprTuple { elts, .. })
        | Expr::List(ast::ExprList { elts, .. }) => {
            for elt in elts {
                bind_target_names(
                    bindings,
                    local_defs,
                    explicit_globals,
                    explicit_nonlocals,
                    elt,
                );
            }
        }
        Expr::Starred(ast::ExprStarred { value, .. }) => {
            bind_target_names(
                bindings,
                local_defs,
                explicit_globals,
                explicit_nonlocals,
                value,
            );
        }
        _ => {}
    }
}

fn merge_binding(existing: BindingKind, incoming: BindingKind) -> BindingKind {
    match (existing, incoming) {
        (BindingKind::Global | BindingKind::Nonlocal, BindingKind::Local) => existing,
        (BindingKind::Local, BindingKind::Global | BindingKind::Nonlocal) => incoming,
        _ => existing,
    }
}

pub fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name == "__dp__"
}

pub fn cell_name(name: &str) -> String {
    format!("_dp_cell_{name}")
}

fn set_binding(bindings: &mut HashMap<String, BindingKind>, name: &str, binding: BindingKind) {
    let binding = if is_internal_symbol(name) {
        BindingKind::Local
    } else {
        binding
    };
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
    fn add_local_def(&mut self, name: &str) {
        if self.explicit_globals.contains(name) || self.explicit_nonlocals.contains(name) {
            return;
        }
        self.local_defs.insert(name.to_string());
    }

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
        self.add_local_def(name);
    }
}

impl Transformer for ScopeCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) => {
                set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Local);
                self.add_local_def(name.id.as_str());
                return;
            }
            Stmt::ClassDef(ast::StmtClassDef { name, .. }) => {
                set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Local);
                self.add_local_def(name.id.as_str());
                return;
            }
            Stmt::Assign(ast::StmtAssign { targets, .. }) => {
                for target in targets {
                    bind_target_names(
                        &mut self.bindings,
                        &mut self.local_defs,
                        &self.explicit_globals,
                        &self.explicit_nonlocals,
                        target,
                    );
                }
            }
            Stmt::AugAssign(ast::StmtAugAssign { target, .. }) => {
                bind_target_names(
                    &mut self.bindings,
                    &mut self.local_defs,
                    &self.explicit_globals,
                    &self.explicit_nonlocals,
                    target,
                );
            }
            Stmt::For(ast::StmtFor { target, .. }) => {
                bind_target_names(
                    &mut self.bindings,
                    &mut self.local_defs,
                    &self.explicit_globals,
                    &self.explicit_nonlocals,
                    target,
                );
            }
            Stmt::With(ast::StmtWith { items, .. }) => {
                for item in items {
                    if let Some(optional_vars) = item.optional_vars.as_deref() {
                        bind_target_names(
                            &mut self.bindings,
                            &mut self.local_defs,
                            &self.explicit_globals,
                            &self.explicit_nonlocals,
                            optional_vars,
                        );
                    }
                }
            }
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                for target in targets {
                    bind_target_names(
                        &mut self.bindings,
                        &mut self.local_defs,
                        &self.explicit_globals,
                        &self.explicit_nonlocals,
                        target,
                    );
                }
            }
            Stmt::Try(ast::StmtTry { handlers, .. }) => {
                for handler in handlers {
                    let ast::ExceptHandler::ExceptHandler(except) = handler;
                    if let Some(name) = except.name.as_ref() {
                        set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Local);
                        self.add_local_def(name.id.as_str());
                    }
                }
            }
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                for name in names {
                    set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Global);
                    self.explicit_globals.insert(name.id.to_string());
                    self.local_defs.remove(name.id.as_str());
                }
                return;
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    set_binding(&mut self.bindings, name.id.as_str(), BindingKind::Nonlocal);
                    self.explicit_nonlocals.insert(name.id.to_string());
                    self.local_defs.remove(name.id.as_str());
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
                self.add_local_def(id.as_str());
                return;
            }
            Expr::Lambda(_) | Expr::Generator(_) => {
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}


fn collect_scope_info(body: &StmtBody) -> ScopeInfo {
    let mut collector = ScopeCollector::default();
    let mut cloned_body = body.clone();
    collector.visit_body(&mut cloned_body);

    ScopeInfo {
        bindings: collector.bindings,
        explicit_nonlocals: collector.explicit_nonlocals,
        local_defs: collector.local_defs,
    }
}

pub fn analyze_module_scope(module: &mut StmtBody) -> Arc<Scope> {
    let tree = ScopeTree::new();
    let info = collect_scope_info(module);
    let root = tree.add_scope(
        ScopeKind::Module,
        "",
        info.bindings,
        info.explicit_nonlocals,
        info.local_defs,
        None,
        None,
    );
    let mut analyzer = ScopeAnalyzer::new(root.clone());
    analyzer.visit_body(module);
    propagate_nonlocal_roots(&tree);
    root
}

struct ScopeAnalyzer {
    scope: Arc<Scope>,
}

impl ScopeAnalyzer {
    fn new(scope: Arc<Scope>) -> Self {
        Self { scope }
    }

    fn resolves_to_enclosing_function(&self, name: &str) -> bool {
        self.scope.any_parent_scope(|scope| {
            match scope.kind() {
                ScopeKind::Function => {
                    if let Some(binding) = scope.scope_bindings().get(name).copied() {
                        match binding {
                            BindingKind::Local | BindingKind::Nonlocal => Some(true),
                            BindingKind::Global => Some(false),
                        }
                    } else {
                        None
                    }
                }
                ScopeKind::Module => Some(false),
                ScopeKind::Class => None,
            }
        }).unwrap_or(false)
    }
}

impl Transformer for ScopeAnalyzer {

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
        let mut info = function_param_bindings(func_def);
        let bindings = &mut info.bindings;
        let load_names = collect_load_names(&func_def.body);
        for name in load_names {
            if is_internal_symbol(name.as_str()) {
                continue;
            }
            if bindings.contains_key(name.as_str()) {
                continue;
            }
            if self.resolves_to_enclosing_function(name.as_str()) {
                set_binding(bindings, name.as_str(), BindingKind::Nonlocal);
            }
        }
        let node_index = self.scope.tree.ensure_node_index(func_def);

        let scope = self.scope.tree.add_scope(
            ScopeKind::Function,
            func_def.name.id.as_str(),
            info.bindings,
            info.explicit_nonlocals,
            info.local_defs,
            Some(self.scope.clone()),
            Some(node_index),
        );
                ScopeAnalyzer::new(scope.clone()).visit_body(&mut func_def.body);                
            }
            Stmt::ClassDef(class_def) => {
                let mut info = class_bindings(class_def);
                let bindings = &mut info.bindings;
                let load_names = collect_load_names(&class_def.body);
                for name in load_names {
                    if is_internal_symbol(name.as_str()) {
                        continue;
                    }
                    if bindings.contains_key(name.as_str()) {
                        continue;
                    }
                    if self.resolves_to_enclosing_function(name.as_str()) {
                        set_binding(bindings, name.as_str(), BindingKind::Nonlocal);
                    }
                }
                let node_index = self.scope.tree.ensure_node_index(class_def);
        
                let scope = self.scope.tree.add_scope(
                    ScopeKind::Class,
                    class_def.name.id.as_str(),
                    info.bindings,
                    info.explicit_nonlocals,
                    info.local_defs,
                    Some(self.scope.clone()),
                    Some(node_index),
        
                );
                ScopeAnalyzer::new(scope.clone()).visit_body(&mut class_def.body);
            }
            _ => {
                walk_stmt(self, stmt);
            }
        }
    }
    
}
fn collect_load_names(body: &StmtBody) -> HashSet<String> {
    #[derive(Default)]
    struct LoadCollector {
        names: HashSet<String>,
    }

    impl Transformer for &mut LoadCollector {
        fn visit_stmt(&mut self, stmt: &mut Stmt) {
            match stmt {
                Stmt::FunctionDef(ast::StmtFunctionDef {
                    parameters,
                    decorator_list,
                    returns,
                    type_params,
                    ..
                }) => {
                    for decorator in decorator_list {
                        self.visit_decorator(decorator);
                    }
                    if let Some(type_params) = type_params {
                        self.visit_type_params(type_params);
                    }
                    self.visit_parameters(parameters);
                    if let Some(expr) = returns {
                        self.visit_annotation(expr);
                    }
                    return;
                }
                Stmt::ClassDef(ast::StmtClassDef {
                    arguments,
                    decorator_list,
                    type_params,
                    ..
                }) => {
                    for decorator in decorator_list {
                        self.visit_decorator(decorator);
                    }
                    if let Some(type_params) = type_params {
                        self.visit_type_params(type_params);
                    }
                    if let Some(arguments) = arguments {
                        self.visit_arguments(arguments);
                    }
                    return;
                }
                _ => walk_stmt(self, stmt)
            }
        }

        fn visit_expr(&mut self, expr: &mut Expr) {
            match expr {
                Expr::Name(ast::ExprName { id, ctx, .. })
                    if matches!(ctx, ExprContext::Load) =>
                {
                    if is_internal_symbol(id.as_str()) {
                        return;
                    }
                    self.names.insert(id.as_str().to_string());
                    return;
                }
                Expr::Lambda(_) | Expr::Generator(_) => {
                    return;
                }
                _ => {}
            }
            walk_expr(self, expr);
        }
    }

    let mut collector = LoadCollector::default();
    let mut cloned = body.clone();
    (&mut collector).visit_body(&mut cloned);
    collector.names
}

fn propagate_nonlocal_roots(tree: &Arc<ScopeTree>) {
    let scopes = {
        let inner = tree.inner.lock().expect("ScopeTree mutex poisoned");
        inner.scopes.clone()
    };

    for scope in scopes {
        let nonlocals = scope
            .scope_bindings()
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
                    && matches!(parent.kind, ScopeKind::Function)
                {
                    set_binding(&mut parent.inner.borrow_mut().bindings, &name, BindingKind::Nonlocal);
                    break;
                }
                current = parent.parent_scope();
            }
        }
    }
}

fn function_param_bindings(func_def: &ast::StmtFunctionDef) -> ScopeInfo {
    let mut info = collect_scope_info(&func_def.body);
    let bindings = &mut info.bindings;

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
        set_binding(bindings, name.as_str(), BindingKind::Local);
        info.local_defs.insert(name);
    }
    for param in args {
        let name = param.parameter.name.to_string();
        set_binding(bindings, name.as_str(), BindingKind::Local);
        info.local_defs.insert(name);
    }
    for param in kwonlyargs {
        let name = param.parameter.name.to_string();
        set_binding(bindings, name.as_str(), BindingKind::Local);
        info.local_defs.insert(name);
    }
    if let Some(param) = vararg {
        let name = param.name.to_string();
        set_binding(bindings, name.as_str(), BindingKind::Local);
        info.local_defs.insert(name);
    }
    if let Some(param) = kwarg {
        let name = param.name.to_string();
        set_binding(bindings, name.as_str(), BindingKind::Local);
        info.local_defs.insert(name);
    }

    info
}

fn class_bindings(class_def: &ast::StmtClassDef) -> ScopeInfo {
    collect_scope_info(&class_def.body)
}


#[derive(Debug, Clone)]
pub struct QualNamer {
    pub kind: ScopeKind,
    pub qualname: String,
}

impl QualNamer {
    pub fn new() -> Self {
        Self { kind: ScopeKind::Module, qualname: "".to_string() }
    }

    pub fn enter_scope(&self, kind: ScopeKind, name: String) -> QualNamer {
        let raw_name = name.strip_prefix("_dp_fn_").unwrap_or(name.as_str());
        let display_name = if raw_name.starts_with("_dp_lambda_") {
            "<lambda>"
        } else if raw_name.starts_with("_dp_genexpr_") {
            "<genexpr>"
        } else {
            raw_name
        };
        let qualname = match self.kind {
            ScopeKind::Function => {
                format!("{}.<locals>.{}", self.qualname, display_name)
            }
            ScopeKind::Class => {
                format!("{}.{}", self.qualname, display_name)
            }
            ScopeKind::Module => {
                display_name.to_string()
            }
        };
        QualNamer {
            kind,
            qualname,
        }
    }    
}



#[cfg(test)]
mod test {
    use super::*;
    use crate::transform::ast_rewrite::rewrite_with_pass;
    use crate::transform::context::Context;
    use crate::transform::driver::{SimplifyExprPass, SimplifyStmtPass};
    use crate::transform::Options;
    use ruff_python_parser::parse_module;

    fn parse_module_body(source: &str) -> StmtBody {
        parse_module(source)
            .expect("parse failure")
            .into_syntax()
            .body
    }

    fn find_function<'a>(body: &'a [Box<Stmt>], name: &str) -> &'a ast::StmtFunctionDef {
        for stmt in body {
            if let Stmt::FunctionDef(func_def) = stmt.as_ref() {
                if func_def.name.id.as_str() == name {
                    return func_def;
                }
            }
        }
        panic!("function {name} not found");
    }

    fn find_class<'a>(body: &'a [Box<Stmt>], name: &str) -> &'a ast::StmtClassDef {
        for stmt in body {
            if let Stmt::ClassDef(class_def) = stmt.as_ref() {
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
        let func_def = find_function(&body.body, "f");
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
        let outer_def = find_function(&body.body, "outer");
        let outer_scope = module_scope
            .lookup_child_scope(outer_def)
            .expect("missing outer scope");
        let inner_def = find_function(&outer_def.body.body, "inner");
        let inner_scope = outer_scope
            .lookup_child_scope(inner_def)
            .expect("missing inner scope");

        assert!(inner_scope.is_nonlocal("x"));
        assert_eq!(
            outer_scope.binding_in_scope("x", BindingUse::Load),
            BindingKind::Nonlocal
        );
        assert_eq!(
            outer_scope.binding_in_scope("y", BindingUse::Load),
            BindingKind::Local
        );
    }

    #[test]
    fn implicit_nonlocal_reads_mark_root_binding() {
        let mut body = parse_module_body(concat!(
            "def outer():\n",
            "    x = 1\n",
            "    def inner():\n",
            "        return x\n",
            "    return inner\n",
        ));
        let module_scope = analyze_module_scope(&mut body);
        let outer_def = find_function(&body.body, "outer");
        let outer_scope = module_scope
            .lookup_child_scope(outer_def)
            .expect("missing outer scope");
        let inner_def = find_function(&outer_def.body.body, "inner");
        let inner_scope = outer_scope
            .lookup_child_scope(inner_def)
            .expect("missing inner scope");

        assert!(inner_scope.is_nonlocal("x"));
        assert_eq!(
            outer_scope.binding_in_scope("x", BindingUse::Load),
            BindingKind::Nonlocal
        );
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
        let class_def = find_class(&body.body, "C");
        let class_scope = module_scope
            .lookup_child_scope(class_def)
            .expect("missing class scope");
        assert!(class_scope.is_local("y"));
        assert!(!class_scope.is_global("y"));
    }

    #[test]
    fn class_scope_marks_enclosing_function_loads_nonlocal() {
        let mut body = parse_module_body(concat!(
            "def outer():\n",
            "    x = 1\n",
            "    class C:\n",
            "        y = x\n",
            "    return C\n",
        ));
        let module_scope = analyze_module_scope(&mut body);
        let outer_def = find_function(&body.body, "outer");
        let outer_scope = module_scope
            .lookup_child_scope(outer_def)
            .expect("missing outer scope");
        let class_def = find_class_recursive(&outer_def.body.body, "C")
            .expect("missing class");
        let class_scope = outer_scope
            .lookup_child_scope(class_def)
            .expect("missing class scope");

        assert!(class_scope.is_nonlocal("x"));
        assert_eq!(
            outer_scope.binding_in_scope("x", BindingUse::Load),
            BindingKind::Nonlocal
        );
    }

    fn find_class_recursive<'a>(body: &'a [Box<Stmt>], name: &str) -> Option<&'a ast::StmtClassDef> {
        for stmt in body {
            match stmt.as_ref() {
                Stmt::ClassDef(class_def) if class_def.name.id.as_str() == name => {
                    return Some(class_def);
                }
                Stmt::If(if_stmt) => {
                    if let Some(found) = find_class_recursive(&if_stmt.body.body, name) {
                        return Some(found);
                    }
                    for clause in &if_stmt.elif_else_clauses {
                        if let Some(found) = find_class_recursive(&clause.body.body, name) {
                            return Some(found);
                        }
                    }
                }
                Stmt::For(for_stmt) => {
                    if let Some(found) = find_class_recursive(&for_stmt.body.body, name) {
                        return Some(found);
                    }
                    if let Some(found) = find_class_recursive(&for_stmt.orelse.body, name) {
                        return Some(found);
                    }
                }
                Stmt::While(while_stmt) => {
                    if let Some(found) = find_class_recursive(&while_stmt.body.body, name) {
                        return Some(found);
                    }
                    if let Some(found) = find_class_recursive(&while_stmt.orelse.body, name) {
                        return Some(found);
                    }
                }
                Stmt::BodyStmt(body) => {
                    if let Some(found) = find_class_recursive(&body.body, name) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    #[test]
    fn loop_unpack_targets_bind_outer_locals_for_class_bodies() {
        let source = concat!(
            "def outer():\n",
            "    funcs = [1]\n",
            "    for i, func in enumerate(funcs):\n",
            "        class S:\n",
            "            value = func\n",
            "        return S\n",
        );
        let mut body = parse_module_body(source);
        let context = Context::new(Options::for_test(), source);
        rewrite_with_pass(&context, Some(&SimplifyStmtPass), Some(&SimplifyExprPass), &mut body);
        let module_scope = analyze_module_scope(&mut body);
        let outer_def = find_function(&body.body, "outer");
        let outer_scope = module_scope
            .lookup_child_scope(outer_def)
            .expect("missing outer scope");
        assert_eq!(
            outer_scope.scope_bindings().get("func").copied(),
            Some(BindingKind::Nonlocal)
        );

        let class_def = find_class_recursive(&outer_def.body.body, "S")
            .expect("missing nested class");
        let class_scope = outer_scope
            .lookup_child_scope(class_def)
            .expect("missing class scope");
        let parent = class_scope.parent_scope().expect("missing class parent scope");
        assert!(matches!(parent.kind(), ScopeKind::Function));
    }

}

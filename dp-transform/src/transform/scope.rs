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
                child_nonlocals: HashSet::new(),
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
    child_nonlocals: HashSet<String>,
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

    pub fn nonlocal_in_children<'a>(&'a self) -> Ref<'a, HashSet<String>> {
        Ref::map(self.inner.borrow(), |inner| &inner.child_nonlocals)
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

    pub fn child_nonlocal_names(&self) -> HashSet<String> {
        self.nonlocal_in_children().clone()
    }

    pub fn is_nonlocal_in_children(&self, name: &str) -> bool {
        self.nonlocal_in_children().contains(name)
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
            Expr::Lambda(_) | Expr::Generator(_) => {
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}


fn collect_scope_info(body: &StmtBody) -> ScopeBindings {
    let mut collector = ScopeCollector::default();
    let mut cloned_body = body.clone();
    collector.visit_body(&mut cloned_body);

    collector.bindings
}

pub fn analyze_module_scope(module: &mut StmtBody) -> Arc<Scope> {
    let tree = ScopeTree::new();
    let bindings = collect_scope_info(module);
    let root = tree.add_scope(ScopeKind::Module, "", bindings, None, None);
    let mut analyzer = ScopeAnalyzer::new(root.clone());
    analyzer.visit_body(module);
    annotate_child_nonlocals(&tree);
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
                            BindingKind::Local => Some(true),
                            BindingKind::Nonlocal => None,
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
                let mut bindings = function_param_bindings(func_def);
                let load_names = collect_load_names(&func_def.body);
                for name in load_names {
                    if is_internal_symbol(name.as_str()) {
                        continue;
                    }
                    if bindings.contains_key(name.as_str()) {
                        continue;
                    }
                    if self.resolves_to_enclosing_function(name.as_str()) {
                        set_binding(&mut bindings, name.as_str(), BindingKind::Nonlocal);
                    }
                }
                let node_index = self.scope.tree.ensure_node_index(func_def);
        
                let scope = self.scope.tree.add_scope(
                    ScopeKind::Function,
                    func_def.name.id.as_str(),
                    bindings,
                    Some(self.scope.clone()),
                    Some(node_index),
                );
                ScopeAnalyzer::new(scope.clone()).visit_body(&mut func_def.body);                
            }
            Stmt::ClassDef(class_def) => {
                let bindings = class_bindings(class_def);
                let node_index = self.scope.tree.ensure_node_index(class_def);
        
                let scope = self.scope.tree.add_scope(
                    ScopeKind::Class,
                    class_def.name.id.as_str(),
                    bindings,
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
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => return,
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

fn annotate_child_nonlocals(tree: &Arc<ScopeTree>) {
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
                    parent.inner.borrow_mut().child_nonlocals.insert(name.clone());
                    break;
                }
                current = parent.parent_scope();
            }
        }
    }
}

fn function_param_bindings(func_def: &ast::StmtFunctionDef) -> ScopeBindings {
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
        let display_name = name.strip_prefix("_dp_fn_").unwrap_or(name.as_str());
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
        let class_def = find_class(&body.body, "C");
        let class_scope = module_scope
            .lookup_child_scope(class_def)
            .expect("missing class scope");
        assert!(class_scope.is_local("y"));
        assert!(!class_scope.is_global("y"));
    }

}

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use super::Options;
use crate::body_transform::{walk_expr, walk_stmt, Transformer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeKind {
    Function,
    Class,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scope {
    Local,
    Nonlocal,
    Global,
}

#[derive(Clone, Debug)]
pub struct ScopeInfo {
    pub kind: ScopeKind,
    pub qualname: String,
    bindings: HashMap<String, Scope>,
}

impl ScopeInfo {
    pub fn is_local(&self, name: &str) -> bool {
        self.binding_is(name, Scope::Local)
    }

    pub fn is_global(&self, name: &str) -> bool {
        self.binding_is(name, Scope::Global)
    }

    pub fn is_nonlocal(&self, name: &str) -> bool {
        self.binding_is(name, Scope::Nonlocal)
    }

    pub(crate) fn local_names(&self) -> HashSet<String> {
        self.bindings
            .keys()
            .filter(|name| self.is_local(name))
            .cloned()
            .collect()
    }

    pub(crate) fn global_names(&self) -> HashSet<String> {
        self.bindings
            .keys()
            .filter(|name| self.is_global(name))
            .cloned()
            .collect()
    }

    pub(crate) fn nonlocal_names(&self) -> HashSet<String> {
        self.bindings
            .keys()
            .filter(|name| self.is_nonlocal(name))
            .cloned()
            .collect()
    }

    pub(crate) fn remap_bindings<F>(&mut self, f: F)
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut next = HashMap::with_capacity(self.bindings.len());
        for (name, binding) in std::mem::take(&mut self.bindings) {
            let mapped = f(&name);
            let key = mapped.unwrap_or(name);
            if let Some(existing) = next.get(&key).copied() {
                let merged = merge_binding(existing, binding);
                if merged != existing {
                    next.insert(key, merged);
                }
            } else {
                next.insert(key, binding);
            }
        }
        self.bindings = next;
    }

    fn binding_is(&self, name: &str, scope: Scope) -> bool {
        matches!(self.bindings.get(name), Some(found) if *found == scope)
    }

}

#[derive(Default)]
struct ScopeCollector {
    bindings: HashMap<String, Scope>,
}

fn merge_binding(existing: Scope, incoming: Scope) -> Scope {
    match (existing, incoming) {
        (Scope::Global | Scope::Nonlocal, Scope::Local) => existing,
        (Scope::Local, Scope::Global | Scope::Nonlocal) => incoming,
        _ => existing,
    }
}

fn set_binding(bindings: &mut HashMap<String, Scope>, name: &str, binding: Scope) {
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
        set_binding(&mut self.bindings, name, Scope::Local);
    }
}

impl Transformer for ScopeCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) => {
                set_binding(&mut self.bindings, name.id.as_str(), Scope::Local);
                return;
            }
            Stmt::ClassDef(ast::StmtClassDef { name, .. }) => {
                set_binding(&mut self.bindings, name.id.as_str(), Scope::Local);
                return;
            }
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                for name in names {
                    set_binding(&mut self.bindings, name.id.as_str(), Scope::Global);
                }
                return;
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    set_binding(&mut self.bindings, name.id.as_str(), Scope::Nonlocal);
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
                set_binding(&mut self.bindings, id.as_str(), Scope::Local);
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

struct ScopeBasics {
    bindings: HashMap<String, Scope>,
}

fn collect_scope_info(body: &[Stmt]) -> ScopeBasics {
    let mut collector = ScopeCollector::default();
    let mut cloned_body = body.to_vec();
    collector.visit_body(&mut cloned_body);

    ScopeBasics {
        bindings: collector.bindings,
    }
}

pub struct Namer {
    counter: Cell<usize>,
}

impl Namer {
    pub fn new() -> Self {
        Self {
            counter: Cell::new(0),
        }
    }

    pub fn fresh(&self, name: &str) -> String {
        let id = self.counter.get() + 1;
        self.counter.set(id);
        format!("_dp_{name}_{id}")
    }
}

pub struct Context {
    pub namer: Namer,
    pub options: Options,
    pub source: String,
    function_scopes: RefCell<Vec<ScopeInfo>>,
}

impl Context {
    pub fn new(options: Options, source: &str) -> Self {
        Self {
            namer: Namer::new(),
            options,
            source: source.to_string(),
            function_scopes: RefCell::new(Vec::new()),
        }
    }

    pub fn source_slice(&self, range: ruff_text_size::TextRange) -> Option<&str> {
        let start = range.start().to_usize();
        let end = range.end().to_usize();
        self.source.get(start..end)
    }

    pub fn fresh(&self, name: &str) -> String {
        self.namer.fresh(name)
    }

    pub fn current_qualname(&self) -> Option<(String, ScopeKind)> {
        self.function_scopes
            .borrow()
            .iter()
            .rev()
            .find(|scope| {
                !(scope.kind == ScopeKind::Function && is_internal_function(&scope.qualname))
            })
            .map(|scope| (scope.qualname.clone(), scope.kind))
    }

    pub fn make_qualname(&self, func_name: &str) -> String {
        if let Some((current_qualname, kind)) = self.current_qualname() {
            if self.function_globals_contains(func_name) {
                return func_name.to_string();
            }
            if kind == ScopeKind::Function {
                return format!("{current_qualname}.<locals>.{func_name}");
            } else {
                return format!("{current_qualname}.{func_name}");
            }
        } else {
            return func_name.to_string();
        }
    }

    pub fn push_scope(&self, info: ScopeInfo) {
        self.function_scopes.borrow_mut().push(info);
    }

    pub fn pop_scope(&self) {
        self.function_scopes.borrow_mut().pop();
    }

    fn function_globals_contains(&self, name: &str) -> bool {
        self.function_scopes
            .borrow()
            .iter()
            .rev()
            .find(|scope| scope.kind == ScopeKind::Function)
            .map_or(false, |info| info.is_global(name))
    }

    pub fn analyze_function_scope(&self, func_def: &ast::StmtFunctionDef) -> ScopeInfo {
        let info = collect_scope_info(&func_def.body);
        let mut bindings = info.bindings;

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
            set_binding(&mut bindings, name.as_str(), Scope::Local);
        }
        for param in args {
            let name = param.parameter.name.to_string();
            set_binding(&mut bindings, name.as_str(), Scope::Local);
        }
        for param in kwonlyargs {
            let name = param.parameter.name.to_string();
            set_binding(&mut bindings, name.as_str(), Scope::Local);
        }
        if let Some(param) = vararg {
            let name = param.name.to_string();
            set_binding(&mut bindings, name.as_str(), Scope::Local);
        }
        if let Some(param) = kwarg {
            let name = param.name.to_string();
            set_binding(&mut bindings, name.as_str(), Scope::Local);
        }

        let qualname = self.make_qualname(func_def.name.id.as_str());

        ScopeInfo {
            kind: ScopeKind::Function,
            qualname: qualname,
            bindings,
        }
    }

    pub fn analyze_class_scope(&self, class_qualname: &str, body: &[Stmt]) -> ScopeInfo {
        let info = collect_scope_info(body);
        let bindings = info.bindings;
        ScopeInfo {
            kind: ScopeKind::Class,
            qualname: class_qualname.to_string(),
            bindings,
        }
    }

}

fn is_internal_function(qualname: &str) -> bool {
    qualname
        .rsplit('.')
        .next()
        .map_or(false, |segment| segment.starts_with("_dp_"))
}

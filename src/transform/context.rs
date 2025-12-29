use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use super::Options;
use crate::body_transform::{walk_expr, walk_stmt, Transformer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeKind {
    Function,
    Class,
}

#[derive(Clone, Debug)]
pub struct ScopeInfo {
    pub kind: ScopeKind,
    pub qualname: String,
    pub locals: HashSet<String>,
    pub globals: HashSet<String>,
    pub nonlocals: HashSet<String>,
    pub params: Vec<String>,
    pub pending: HashSet<String>,
}

#[derive(Default)]
struct ScopeCollector {
    locals: HashSet<String>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
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
        self.locals.insert(name.to_string());
    }
}

impl Transformer for ScopeCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) => {
                self.locals.insert(name.id.to_string());
                return;
            }
            Stmt::ClassDef(ast::StmtClassDef { name, .. }) => {
                self.locals.insert(name.id.to_string());
                return;
            }
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                for name in names {
                    self.globals.insert(name.id.to_string());
                }
                return;
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    self.nonlocals.insert(name.id.to_string());
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
                self.locals.insert(id.to_string());
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
    locals: HashSet<String>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
    params: Vec<String>,
}

fn collect_scope_info(body: &[Stmt]) -> ScopeBasics {
    let mut collector = ScopeCollector::default();
    let mut cloned_body = body.to_vec();
    collector.visit_body(&mut cloned_body);

    let mut locals = collector.locals;
    for name in collector.globals.iter().chain(collector.nonlocals.iter()) {
        locals.remove(name);
    }

    ScopeBasics {
        locals,
        globals: collector.globals,
        nonlocals: collector.nonlocals,
        params: Vec::new(),
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
    function_scopes: RefCell<Vec<ScopeInfo>>,
}

impl Context {
    pub fn new(options: Options) -> Self {
        Self {
            namer: Namer::new(),
            options,
            function_scopes: RefCell::new(Vec::new()),
        }
    }

    pub fn fresh(&self, name: &str) -> String {
        self.namer.fresh(name)
    }

    pub fn current_qualname(&self) -> Option<(String, ScopeKind)> {
        self.function_scopes
            .borrow()
            .last()
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
            .map_or(false, |info| info.globals.contains(name))
    }

    pub fn analyze_function_scope(&self, func_def: &ast::StmtFunctionDef) -> ScopeInfo {
        let mut info = collect_scope_info(&func_def.body);

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
            info.locals.insert(name.clone());
            info.params.push(name);
        }
        for param in args {
            let name = param.parameter.name.to_string();
            info.locals.insert(name.clone());
            info.params.push(name);
        }
        for param in kwonlyargs {
            let name = param.parameter.name.to_string();
            info.locals.insert(name.clone());
            info.params.push(name);
        }
        if let Some(param) = vararg {
            let name = param.name.to_string();
            info.locals.insert(name.clone());
            info.params.push(name);
        }
        if let Some(param) = kwarg {
            let name = param.name.to_string();
            info.locals.insert(name.clone());
            info.params.push(name);
        }

        for name in info.globals.iter().chain(info.nonlocals.iter()) {
            info.locals.remove(name);
        }

        let qualname = self.make_qualname(func_def.name.id.as_str());

        ScopeInfo {
            kind: ScopeKind::Function,
            qualname: qualname,
            locals: info.locals,
            globals: info.globals,
            nonlocals: info.nonlocals,
            params: info.params,
            pending: HashSet::new(),
        }
    }

    pub fn analyze_class_scope(&self, class_qualname: &str, body: &[Stmt]) -> ScopeInfo {
        let mut info = collect_scope_info(body);
        let mut pending = HashSet::new();
        for stmt in body {
            if let Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) = stmt {
                pending.insert(name.id.to_string());
            }
        }

        for name in info.globals.iter().chain(info.nonlocals.iter()) {
            info.locals.remove(name);
        }

        ScopeInfo {
            kind: ScopeKind::Class,
            qualname: class_qualname.to_string(),
            locals: info.locals,
            globals: info.globals,
            nonlocals: info.nonlocals,
            params: info.params,
            pending,
        }
    }
}

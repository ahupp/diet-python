use std::{collections::{HashMap, HashSet}};

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use crate::body_transform::{Transformer, walk_expr, walk_stmt};


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingKind {
    Local,
    Nonlocal,
    Global,
}

type ScopeBindings = HashMap<String, BindingKind>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scope<'a> {
    Function { name: String, bindings: ScopeBindings , parent: Option<&'a Scope<'a>>},
    Class { name: String, bindings: ScopeBindings , parent: Option<&'a Scope<'a>>},
    Module { bindings: ScopeBindings },
}

impl<'a> Scope<'a> {

    pub fn make_qualname(&self, func_name: &str) -> String {
        let mut components = vec![func_name.to_string()];
        let mut current = match self {
            Scope::Function { parent, .. } => *parent,
            Scope::Class { parent, .. } => *parent,
            Scope::Module { .. } => None,
        };

        while let Some(scope) = current {
            match scope {
                Scope::Function { name, parent, .. } => {
                    components.push("<locals>".to_string());
                    components.push(name.clone());
                    current = *parent;
                }
                Scope::Class { name, parent, .. } => {
                    components.push(name.clone());
                    current = *parent;
                }
                Scope::Module { .. } => {
                    break;
                }
            }
        }
        components.reverse();
        components.join(".")
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

        let mut current = Some( self);
        while let Some(current_scope) = current {
            let (bindings, parent) = match current_scope {
                Scope::Function { bindings, parent, .. } => {
                    (bindings, parent.to_owned())
                }
                Scope::Class { bindings, parent,.. } => {
                    (bindings, parent.to_owned())
                }
                Scope::Module { bindings } => {
                    (bindings, None::<&'a Scope<'a>>)
                }
            };
            if let Some(ret) = find(bindings) {
                return Some(ret);
            }
            current = parent;
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


pub fn collect_scope_info(body: &[Stmt]) -> HashMap<String, BindingKind> {
    let mut collector = ScopeCollector::default();
    let mut cloned_body = body.to_vec();
    collector.visit_body(&mut cloned_body);

    collector.bindings
}


pub fn analyze_function_scope<'a>(func_def: &ast::StmtFunctionDef, parent: Option<&'a Scope<'a>>) -> Scope<'a> {
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

    Scope::Function {
        name: func_def.name.id.to_string(),
        bindings,
        parent,
    }
}

pub fn analyze_class_scope<'a>(class_def: &ast::StmtClassDef, parent: Option<&'a Scope<'a>>) -> Scope<'a> {
    let bindings = collect_scope_info(&class_def.body);
    Scope::Class {
        name: class_def.name.id.to_string(),
        bindings,
        parent,
    }
}

pub(crate) mod explicit_scope {
    use std::collections::{HashMap, HashSet};

    use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

    use crate::body_transform::{walk_expr, walk_pattern, walk_stmt, Transformer};
    use super::{BindingKind, merge_binding, set_binding, ScopeBindings};


    #[derive(Clone, Debug)]
    pub(crate) struct ScopeInfo {
        pub(crate) bindings: HashMap<String, BindingKind>,
    }

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

    pub(crate) fn collect_scope_info(body: &[Stmt]) -> ScopeInfo {
        collect_scope_info_with_named_expr(body, false)
    }

    pub(crate) fn collect_scope_info_with_named_expr(
        body: &[Stmt],
        named_expr_nonlocal: bool,
    ) -> ScopeInfo {
        let mut collector = ScopeCollector::new(named_expr_nonlocal);
        let mut cloned_body = body.to_vec();
        collector.visit_body(&mut cloned_body);
        ScopeInfo {
            bindings: collector.bindings,
        }
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

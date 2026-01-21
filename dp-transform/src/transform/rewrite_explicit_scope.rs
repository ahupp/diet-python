use std::collections::{HashMap, HashSet};

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use crate::body_transform::{walk_expr, walk_stmt, Transformer};

pub fn rewrite(body: &mut Vec<Stmt>) {
    let module_scope = collect_scope_info(body);
    let mut rewriter = ExplicitScopeRewriter::new(Frame::module(module_scope));
    rewriter.visit_body(body);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Binding {
    Local,
    Global,
    Nonlocal,
}

#[derive(Clone, Debug)]
struct ScopeInfo {
    bindings: HashMap<String, Binding>,
}

impl ScopeInfo {
    fn local_names(&self) -> HashSet<String> {
        self.bindings
            .iter()
            .filter_map(|(name, binding)| {
                if *binding == Binding::Local {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn global_names(&self) -> HashSet<String> {
        self.bindings
            .iter()
            .filter_map(|(name, binding)| {
                if *binding == Binding::Global {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn nonlocal_names(&self) -> HashSet<String> {
        self.bindings
            .iter()
            .filter_map(|(name, binding)| {
                if *binding == Binding::Nonlocal {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Default)]
struct ScopeCollector {
    bindings: HashMap<String, Binding>,
}

fn merge_binding(existing: Binding, incoming: Binding) -> Binding {
    match (existing, incoming) {
        (Binding::Global | Binding::Nonlocal, Binding::Local) => existing,
        (Binding::Local, Binding::Global | Binding::Nonlocal) => incoming,
        _ => existing,
    }
}

fn set_binding(bindings: &mut HashMap<String, Binding>, name: &str, binding: Binding) {
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
        set_binding(&mut self.bindings, name, Binding::Local);
    }
}

impl Transformer for ScopeCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) => {
                set_binding(&mut self.bindings, name.id.as_str(), Binding::Local);
                return;
            }
            Stmt::ClassDef(ast::StmtClassDef { name, .. }) => {
                set_binding(&mut self.bindings, name.id.as_str(), Binding::Local);
                return;
            }
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                for name in names {
                    set_binding(&mut self.bindings, name.id.as_str(), Binding::Global);
                }
                return;
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    set_binding(&mut self.bindings, name.id.as_str(), Binding::Nonlocal);
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
                set_binding(&mut self.bindings, id.as_str(), Binding::Local);
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

fn collect_scope_info(body: &[Stmt]) -> ScopeInfo {
    let mut collector = ScopeCollector::default();
    let mut cloned_body = body.to_vec();
    collector.visit_body(&mut cloned_body);
    ScopeInfo {
        bindings: collector.bindings,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameKind {
    Module,
    Function,
    Class,
}

#[derive(Clone, Debug)]
struct Frame {
    kind: FrameKind,
    depth: usize,
    locals: HashSet<String>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
}

impl Frame {
    fn module(scope: ScopeInfo) -> Self {
        Self {
            kind: FrameKind::Module,
            depth: 0,
            locals: scope.local_names(),
            globals: scope.global_names(),
            nonlocals: scope.nonlocal_names(),
        }
    }

    fn function(scope: ScopeInfo, parent_depth: usize) -> Self {
        Self {
            kind: FrameKind::Function,
            depth: parent_depth + 1,
            locals: scope.local_names(),
            globals: scope.global_names(),
            nonlocals: scope.nonlocal_names(),
        }
    }

    fn class(scope: ScopeInfo, parent_depth: usize) -> Self {
        Self {
            kind: FrameKind::Class,
            depth: parent_depth,
            locals: scope.local_names(),
            globals: scope.global_names(),
            nonlocals: scope.nonlocal_names(),
        }
    }
}

struct ExplicitScopeRewriter {
    stack: Vec<Frame>,
    in_binding: usize,
}

impl ExplicitScopeRewriter {
    fn new(module: Frame) -> Self {
        Self {
            stack: vec![module],
            in_binding: 0,
        }
    }

    fn current(&self) -> &Frame {
        self.stack
            .last()
            .expect("explicit scope rewriter stack should not be empty")
    }

    fn find_binding_depth(&self, name: &str) -> Option<usize> {
        for frame in self.stack.iter().rev() {
            if matches!(frame.kind, FrameKind::Module | FrameKind::Function)
                && frame.locals.contains(name)
            {
                return Some(frame.depth);
            }
        }
        None
    }

    fn in_class_scope(&self) -> bool {
        self.stack
            .iter()
            .any(|frame| matches!(frame.kind, FrameKind::Class))
    }

    fn should_skip_name(&self, name: &str) -> bool {
        name.starts_with("_dp_")
            || name == "__dp__"
            || name.contains('$')
            || matches!(name, "__classcell__")
    }

    fn rewrite_name(&self, id: &mut ast::name::Name, ctx: ExprContext) {
        let name = id.as_str();
        if self.should_skip_name(name) {
            return;
        }
        if name == "__class__" && self.in_class_scope() {
            return;
        }

        let current = self.current();
        if current.kind == FrameKind::Module {
            return;
        }
        if self.in_binding > 0
            && !current.globals.contains(name)
            && !current.nonlocals.contains(name)
        {
            return;
        }
        if current.locals.contains(name) {
            return;
        }

        let target_depth = if current.globals.contains(name) {
            Some(0)
        } else if current.nonlocals.contains(name) {
            self.stack
                .iter()
                .rev()
                .skip(1)
                .find_map(|frame| {
                    if matches!(frame.kind, FrameKind::Module | FrameKind::Function)
                        && frame.locals.contains(name)
                    {
                        Some(frame.depth)
                    } else {
                        None
                    }
                })
        } else if matches!(ctx, ExprContext::Load | ExprContext::Del) {
            self.find_binding_depth(name).or(Some(0))
        } else {
            None
        };

        let Some(depth) = target_depth else {
            return;
        };

        let rewritten = format!("{name}${depth}");
        *id = ast::name::Name::new(rewritten.as_str());
    }

    fn push_function_frame(&mut self, func_def: &ast::StmtFunctionDef) {
        let mut scope = collect_scope_info(&func_def.body);
        let ast::Parameters {
            posonlyargs,
            args,
            vararg,
            kwonlyargs,
            kwarg,
            ..
        } = func_def.parameters.as_ref();

        for param in posonlyargs {
            set_binding(&mut scope.bindings, param.parameter.name.as_str(), Binding::Local);
        }
        for param in args {
            set_binding(&mut scope.bindings, param.parameter.name.as_str(), Binding::Local);
        }
        for param in kwonlyargs {
            set_binding(&mut scope.bindings, param.parameter.name.as_str(), Binding::Local);
        }
        if let Some(param) = vararg {
            set_binding(&mut scope.bindings, param.name.as_str(), Binding::Local);
        }
        if let Some(param) = kwarg {
            set_binding(&mut scope.bindings, param.name.as_str(), Binding::Local);
        }

        let parent_depth = self.current().depth;
        self.stack.push(Frame::function(scope, parent_depth));
    }

    fn push_class_builder_frame(&mut self, func_def: &ast::StmtFunctionDef) {
        let mut scope = collect_scope_info(&func_def.body);
        let ast::Parameters {
            posonlyargs,
            args,
            vararg,
            kwonlyargs,
            kwarg,
            ..
        } = func_def.parameters.as_ref();

        for param in posonlyargs {
            set_binding(&mut scope.bindings, param.parameter.name.as_str(), Binding::Local);
        }
        for param in args {
            set_binding(&mut scope.bindings, param.parameter.name.as_str(), Binding::Local);
        }
        for param in kwonlyargs {
            set_binding(&mut scope.bindings, param.parameter.name.as_str(), Binding::Local);
        }
        if let Some(param) = vararg {
            set_binding(&mut scope.bindings, param.name.as_str(), Binding::Local);
        }
        if let Some(param) = kwarg {
            set_binding(&mut scope.bindings, param.name.as_str(), Binding::Local);
        }

        let parent_depth = self.current().depth;
        self.stack.push(Frame::class(scope, parent_depth));
    }

    fn push_class_frame(&mut self, class_def: &ast::StmtClassDef) {
        let scope = collect_scope_info(&class_def.body);
        let parent_depth = self.current().depth;
        self.stack.push(Frame::class(scope, parent_depth));
    }

    fn pop_frame(&mut self) {
        self.stack.pop();
    }
}

fn is_class_ns_builder(func_def: &ast::StmtFunctionDef) -> bool {
    let name = func_def.name.id.as_str();
    if !name.starts_with("_dp_ns_") {
        return false;
    }
    let params = func_def.parameters.as_ref();
    let first = params
        .posonlyargs
        .first()
        .map(|param| param.parameter.name.as_str())
        .or_else(|| params.args.first().map(|param| param.parameter.name.as_str()));
    matches!(first, Some("_dp_class_ns"))
}

impl Transformer for ExplicitScopeRewriter {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        for mut stmt in std::mem::take(body) {
            match stmt {
                Stmt::Global(_) | Stmt::Nonlocal(_) => {
                    continue;
                }
                _ => {
                    self.visit_stmt(&mut stmt);
                    new_body.push(stmt);
                }
            }
        }
        *body = new_body;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                self.visit_expr(value);
                for target in targets {
                    self.in_binding += 1;
                    self.visit_expr(target);
                    self.in_binding -= 1;
                }
                return;
            }
            Stmt::AnnAssign(ast::StmtAnnAssign { target, value, annotation, .. }) => {
                self.in_binding += 1;
                self.visit_expr(target);
                self.in_binding -= 1;
                self.visit_annotation(annotation);
                if let Some(value) = value {
                    self.visit_expr(value);
                }
                return;
            }
            Stmt::AugAssign(ast::StmtAugAssign { target, value, .. }) => {
                self.in_binding += 1;
                self.visit_expr(target);
                self.in_binding -= 1;
                self.visit_expr(value);
                return;
            }
            Stmt::For(ast::StmtFor { target, iter, body, orelse, .. }) => {
                self.in_binding += 1;
                self.visit_expr(target);
                self.in_binding -= 1;
                self.visit_expr(iter);
                self.visit_body(body);
                self.visit_body(orelse);
                return;
            }
            Stmt::With(ast::StmtWith { items, body, .. }) => {
                for item in items {
                    self.visit_expr(&mut item.context_expr);
                    if let Some(optional_vars) = &mut item.optional_vars {
                        self.in_binding += 1;
                        self.visit_expr(optional_vars);
                        self.in_binding -= 1;
                    }
                }
                self.visit_body(body);
                return;
            }
            Stmt::FunctionDef(func_def) => {
                if is_class_ns_builder(func_def) {
                    self.push_class_builder_frame(func_def);
                } else {
                    self.push_function_frame(func_def);
                }
                walk_stmt(self, stmt);
                self.pop_frame();
                return;
            }
            Stmt::ClassDef(class_def) => {
                self.push_class_frame(class_def);
                walk_stmt(self, stmt);
                self.pop_frame();
                return;
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            if self.in_binding > 0 {
                if let Some(current) = self.stack.last_mut() {
                    let name = id.as_str();
                    if !current.globals.contains(name) && !current.nonlocals.contains(name) {
                        current.locals.insert(name.to_string());
                    }
                }
            }
            self.rewrite_name(id, *ctx);
            return;
        }
        walk_expr(self, expr);
    }
}

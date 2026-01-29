use std::mem::take;
use std::sync::Arc;

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt, StmtBody};

use crate::template::{empty_body, into_body};
use crate::transform::context::Context;
use crate::transform::rewrite_class_def::{class_def_to_create_class_fn, method};
use crate::transform::rewrite_stmt;
use crate::transform::scope::{BindingKind, BindingUse, Scope, ScopeKind, is_internal_symbol};
use crate::transform::util::is_noarg_call;
use crate::transformer::{Transformer, walk_expr, walk_stmt};
use crate::{py_expr, py_stmt};

pub fn class_body_load_cell(name: &str) -> Expr {
    py_expr!(
        "__dp__.class_lookup_cell(_dp_class_ns, {name:literal}, {name:id})",
        name = name,
    )
}

pub fn class_body_load_global(name: &str) -> Expr {
    py_expr!(
        "__dp__.class_lookup_global(_dp_class_ns, {name:literal}, globals())",
        name = name,
    )
}

fn class_body_store_target(name: &str, ctx: ExprContext) -> Expr {
    let mut expr = py_expr!("_dp_class_ns[{name:literal}]", name = name);
    if let Expr::Subscript(sub) = &mut expr {
        sub.ctx = ctx;
    }
    expr
}

fn class_body_store_global(name: &str, ctx: ExprContext) -> Expr {
    let mut expr = py_expr!("globals()[{name:literal}]", name = name);
    if let Expr::Subscript(sub) = &mut expr {
        sub.ctx = ctx;
    }
    expr
}


pub fn rewrite_class_body_scopes(context: &Context, scope: Arc<Scope>, body: &mut StmtBody) {
    ClassBodyScopeRewriter::new(context, scope).visit_body(body);
}

struct ClassBodyScopeRewriter<'a> {
    context: &'a Context,
    scope: Arc<Scope>,
    hoisted_class_defs: Vec<Stmt>,
    body_name_rewriter: ClassBodyNameRewriter,
}

impl<'a> ClassBodyScopeRewriter<'a> {
    fn new(context: &'a Context, scope: Arc<Scope>) -> Self {
        Self { context, scope: scope.clone(), hoisted_class_defs: Vec::new(), body_name_rewriter: ClassBodyNameRewriter::new(scope.clone()) }
    }

    fn take_hoisted(&mut self) -> Vec<Stmt> {
        take(&mut self.hoisted_class_defs)
    }
}

impl<'a> Transformer for ClassBodyScopeRewriter<'a> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::ClassDef(class_def) => {
                if matches!(self.scope.kind(), ScopeKind::Class) {
                    
                    if let Some(arguments) = class_def.arguments.as_mut() {
                        self.body_name_rewriter.visit_arguments(arguments);
                    }
                    for dec in &mut class_def.decorator_list {
                        self.body_name_rewriter.visit_decorator(dec);
                    }
                    if let Some(type_params) = class_def.type_params.as_mut() {
                        self.body_name_rewriter.visit_type_params(type_params);
                    }
                }

                let decorator_list = take(&mut class_def.decorator_list);
                let needs_class_cell = method::rewrite_explicit_super_classcell(class_def);

                let class_scope = self
                    .scope
                    .child_scope_for_class(class_def)
                    .expect("no child scope for class");

                let mut class_rewriter = ClassBodyScopeRewriter::new(self.context, class_scope.clone());
                class_rewriter.visit_body(&mut class_def.body);
                let mut hoisted = class_rewriter.take_hoisted();
                class_rewriter.body_name_rewriter.visit_body(&mut class_def.body);

                let (class_ns_def, define_class_fn) = class_def_to_create_class_fn(
                    self.context,
                    class_def,
                    class_scope.qualnamer.qualname.clone(),
                    needs_class_cell,
                );

                hoisted.push(class_ns_def.clone().into());

                let mut children = Vec::new();
                if matches!(self.scope.kind(), ScopeKind::Class) {
                    self.hoisted_class_defs.append(&mut hoisted);
                } else {
                    children.append(&mut hoisted);
                }
                children.push(define_class_fn.clone().into());

                let decorated_class = rewrite_stmt::decorator::rewrite(
                    decorator_list, 
                    py_expr!(r"{define_class_fn:id}()", define_class_fn = define_class_fn.name.id.as_str()));

                children.push(py_stmt!(r"{class_name:id} = {decorated_class:expr}", 
                                        class_name = class_def.name.id.as_str(), 
                                        decorated_class = decorated_class));

                *stmt = into_body(children);
            }
            Stmt::FunctionDef(func_def) => {
                let func_scope = self
                    .scope
                    .child_scope_for_function(func_def)
                    .expect("no child scope for function");
                ClassBodyScopeRewriter::new(self.context, func_scope).visit_body(&mut func_def.body);
  
                return;
            }
            _ => walk_stmt(self, stmt),
        }
    }
}

struct ClassBodyNameRewriter {
    scope: Arc<Scope>,
}

impl ClassBodyNameRewriter {
    fn new(scope: Arc<Scope>) -> Self {
        Self { scope }
    }

    fn rewrite_load(&self, name: &ast::ExprName) -> Option<Expr> {
        if !matches!(self.scope.kind(), ScopeKind::Class) {
            return None;
        }

        let id = name.id.as_str();
        if id == "__classcell__" || is_internal_symbol(id) {
            return None;
        }
        let binding = self.scope.scope_bindings().get(id).copied();
        match binding {
            Some(BindingKind::Global) => Some(class_body_load_global(id)),
            Some(BindingKind::Nonlocal) => Some(class_body_load_cell(id)),
            Some(BindingKind::Local) => {
                if self.enclosing_function_binds_name(id) {
                    Some(class_body_load_global(id))
                } else {
                    Some(class_body_load_global(id))
                }
            }
            None => {
                if self.enclosing_function_binds_name(id) {
                    Some(class_body_load_cell(id))
                } else {
                    Some(class_body_load_global(id))
                }
            }
        }
    }

    fn rewrite_store(&self, name: &ast::ExprName) -> Option<Expr> {
        if !matches!(self.scope.kind(), ScopeKind::Class) {
            return None;
        }

        let id = name.id.as_str();
        if id == "__classcell__" || is_internal_symbol(id) {
            return None;
        }

        match self.scope.binding_in_scope(id, BindingUse::Load) {
            BindingKind::Global => Some(class_body_store_global(id, name.ctx)),
            BindingKind::Nonlocal => None,
            BindingKind::Local => Some(class_body_store_target(id, name.ctx)),
        }
    }

    fn enclosing_function_binds_name(&self, name: &str) -> bool {
        self.scope
            .any_parent_scope(|scope| match scope.kind() {
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
            })
            .unwrap_or(false)
    }
}

impl Transformer for ClassBodyNameRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if !matches!(self.scope.kind(), ScopeKind::Class) {
            return;
        }
        match stmt {
            Stmt::Assign(ast::StmtAssign {
                targets,
                value,
                range,
                node_index,
            }) => {
                assert!(targets.len() == 1);
                if let Expr::Name(name) = &targets[0] {
                    if is_internal_symbol(name.id.as_str()) {
                        self.visit_expr(value.as_mut());
                        return;
                    }
                    let binding = self.scope.binding_in_scope(name.id.as_str(), BindingUse::Load);
                    let mut value = value.clone();
                    self.visit_expr(value.as_mut());
                    match binding {
                        BindingKind::Global => {
                            *stmt = py_stmt!(
                                "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                                name = name.id.as_str(),
                                value = value
                            );
                            return;
                        }
                        BindingKind::Local => {
                            let target =
                                class_body_store_target(name.id.as_str(), ExprContext::Store);
                            *stmt = Stmt::Assign(ast::StmtAssign {
                                targets: vec![target],
                                value,
                                range: *range,
                                node_index: node_index.clone(),
                            });
                            return;
                        }
                        BindingKind::Nonlocal => {}
                    }
                }
            }
            Stmt::FunctionDef(func_def) => {
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
                return;
            }
            Stmt::ClassDef(class_def) => {
                for decorator in &mut class_def.decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = class_def.type_params.as_mut() {
                    self.visit_type_params(type_params);
                }
                if let Some(arguments) = class_def.arguments.as_mut() {
                    self.visit_arguments(arguments);
                }
                return;
            }
            Stmt::Global(_) => {
                *stmt = empty_body().into();
                return;
            }
            Stmt::AnnAssign(_) => {
                panic!("AnnAssign should be gone now");
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if !matches!(self.scope.kind(), ScopeKind::Class) {
            return;
        }
        match expr {
            Expr::Call(..) => {
                if is_noarg_call("locals", expr) || is_noarg_call("vars", expr) {
                    *expr = py_expr!("_dp_class_ns");
                    return;
                }
            }
            Expr::Name(name) if matches!(name.ctx, ExprContext::Load) => {
                if let Some(rewritten) = self.rewrite_load(name) {
                    *expr = rewritten;
                }
                return;
            }
            Expr::Name(name) if matches!(name.ctx, ExprContext::Store | ExprContext::Del) => {
                if let Some(rewritten) = self.rewrite_store(name) {
                    *expr = rewritten;
                }
                return;
            }
            Expr::Lambda(ast::ExprLambda { parameters, .. }) => {
                if let Some(parameters) = parameters {
                    self.visit_parameters(parameters);
                }
                return;
            }
            Expr::Generator(ast::ExprGenerator { generators, .. }) => {
                if let Some(first) = generators.first_mut() {
                    self.visit_expr(&mut first.iter);
                }
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

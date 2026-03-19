use std::mem::take;
use std::sync::Arc;

use ruff_python_ast::{Expr, ExprContext, Stmt};

use crate::basic_block::ast_to_ast::body::{suite_mut, Suite};
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_class_def::{class_def_to_create_class_fn, method};
use crate::basic_block::ast_to_ast::rewrite_stmt;
use crate::basic_block::ast_to_ast::scope::{cell_name, BindingKind, BindingUse, Scope, ScopeKind};
use crate::transformer::{walk_stmt, Transformer};
use crate::{py_expr, py_stmt};

pub fn class_body_load_cell(name: &str, cell: &str) -> Expr {
    py_expr!(
        "__dp_class_lookup_cell(_dp_class_ns, {name:literal}, {cell:id})",
        name = name,
        cell = cell,
    )
}

pub fn class_body_load_global(name: &str) -> Expr {
    py_expr!(
        "__dp_class_lookup_global(_dp_class_ns, {name:literal}, globals())",
        name = name,
    )
}

pub(crate) fn class_body_store_target(name: &str, ctx: ExprContext) -> Expr {
    let mut expr = py_expr!("_dp_class_ns[{name:literal}]", name = name);
    if let Expr::Subscript(sub) = &mut expr {
        sub.ctx = ctx;
    }
    expr
}

pub(crate) fn class_body_store_global(name: &str, ctx: ExprContext) -> Expr {
    let mut expr = py_expr!("globals()[{name:literal}]", name = name);
    if let Expr::Subscript(sub) = &mut expr {
        sub.ctx = ctx;
    }
    expr
}

pub fn rewrite_class_body_scopes(context: &Context, scope: Arc<Scope>, body: &mut Suite) {
    ClassBodyScopeRewriter::new(context, scope).visit_body(body);
}

fn class_binding_stmt(scope: &Scope, name: &str, value: Expr) -> Stmt {
    match scope.kind() {
        ScopeKind::Class => match scope.binding_in_scope(name, BindingUse::Load) {
            BindingKind::Global => py_stmt!(
                "__dp_store_global(globals(), {name:literal}, {value:expr})",
                name = name,
                value = value
            ),
            BindingKind::Local | BindingKind::Nonlocal => {
                let target = class_body_store_target(name, ExprContext::Store);
                py_stmt!(
                    "{target:expr} = {value:expr}",
                    target = target,
                    value = value
                )
            }
        },
        ScopeKind::Function => match scope.binding_in_scope(name, BindingUse::Load) {
            BindingKind::Global => py_stmt!(
                "__dp_store_global(globals(), {name:literal}, {value:expr})",
                name = name,
                value = value
            ),
            BindingKind::Nonlocal => {
                let cell = cell_name(name);
                py_stmt!(
                    "__dp_store_cell({cell:id}, {value:expr})",
                    cell = cell.as_str(),
                    value = value
                )
            }
            BindingKind::Local => {
                py_stmt!("{name:id} = {value:expr}", name = name, value = value)
            }
        },
        ScopeKind::Module => py_stmt!("{name:id} = {value:expr}", name = name, value = value),
    }
}

struct ClassBodyScopeRewriter<'a> {
    context: &'a Context,
    scope: Arc<Scope>,
    hoisted_class_defs: Vec<Stmt>,
}

impl<'a> ClassBodyScopeRewriter<'a> {
    fn new(context: &'a Context, scope: Arc<Scope>) -> Self {
        Self {
            context,
            scope: scope.clone(),
            hoisted_class_defs: Vec::new(),
        }
    }

    fn take_hoisted(&mut self) -> Vec<Stmt> {
        take(&mut self.hoisted_class_defs)
    }
}

impl<'a> Transformer for ClassBodyScopeRewriter<'a> {
    fn visit_body(&mut self, body: &mut Suite) {
        let mut rewritten = Vec::with_capacity(body.len());
        for stmt in std::mem::take(body) {
            rewritten.extend(self.rewrite_stmt_list(stmt));
        }
        *body = rewritten;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                let func_scope = self
                    .scope
                    .child_scope_for_function(func_def)
                    .expect("no child scope for function");
                ClassBodyScopeRewriter::new(self.context, func_scope)
                    .visit_body(suite_mut(&mut func_def.body));
            }
            _ => walk_stmt(self, stmt),
        }
    }
}

impl<'a> ClassBodyScopeRewriter<'a> {
    fn rewrite_stmt_list(&mut self, stmt: Stmt) -> Vec<Stmt> {
        let Stmt::ClassDef(mut class_def) = stmt else {
            let mut stmt = stmt;
            self.visit_stmt(&mut stmt);
            return vec![stmt];
        };

        let decorator_list = take(&mut class_def.decorator_list);
        let needs_class_cell = method::rewrite_explicit_super_classcell(&mut class_def);

        let class_scope = self
            .scope
            .child_scope_for_class(&class_def)
            .expect("no child scope for class");

        let mut class_rewriter = ClassBodyScopeRewriter::new(self.context, class_scope.clone());
        class_rewriter.visit_body(suite_mut(&mut class_def.body));
        let mut hoisted = class_rewriter.take_hoisted();

        let (class_ns_def, define_class_fn) = class_def_to_create_class_fn(
            self.context,
            &mut class_def,
            class_scope.qualnamer.qualname.clone(),
            needs_class_cell,
        );

        hoisted.push(class_ns_def.clone().into());

        let mut children = Vec::new();
        // Keep nested class namespace helpers in lexical scope with the
        // matching `_dp_define_class_*` call site. Hoisting these out
        // of class bodies makes helper resolution depend on module
        // globals, which breaks once top-level code is wrapped in
        // `_dp_module_init`.
        children.append(&mut hoisted);
        children.push(define_class_fn.clone().into());

        let class_ns_outer = if matches!(self.scope.kind(), ScopeKind::Class) {
            py_expr!("_dp_class_ns")
        } else {
            py_expr!("globals()")
        };

        let decorated_class = rewrite_stmt::decorator::rewrite(
            decorator_list,
            py_expr!(
                r"{define_class_fn:id}({class_ns_fn:id}, {class_ns_outer:expr})",
                define_class_fn = define_class_fn.name.id.as_str(),
                class_ns_fn = class_ns_def.name.id.as_str(),
                class_ns_outer = class_ns_outer,
            ),
        );

        children.push(class_binding_stmt(
            self.scope.as_ref(),
            class_def.name.id.as_str(),
            decorated_class,
        ));
        children
    }
}

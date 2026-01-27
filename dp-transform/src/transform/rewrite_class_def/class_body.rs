use std::sync::Arc;

use ruff_python_ast::{self as ast, name::Name, Expr, ExprContext, Stmt};

use crate::body_transform::{Transformer, walk_expr, walk_stmt};
use crate::template::py_stmt_single;
use crate::transform::ast_rewrite::Rewrite;
use crate::{py_expr, py_stmt};
use crate::transform::context::Context;
use crate::transform::rewrite_class_def::{method};
use crate::transform::rewrite_stmt;
use crate::transform::scope::{BindingKind, BindingUse, Scope, ScopeKind};



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

fn is_dp_temp_name(name: &str) -> bool {
    name.starts_with("_dp_")
}



pub fn rewrite_class_body_scopes(context: &Context, scope: Arc<Scope>, body: &mut Vec<Stmt>) {
    let mut rewriter = ClassBodyScopeRewriter::new(context, scope);
    rewriter.visit_body(body);
}

struct ClassBodyScopeRewriter<'a> {
    context: &'a Context,
    scope: Arc<Scope>,
}

impl<'a> ClassBodyScopeRewriter<'a> {
    fn new(context: &'a Context, scope: Arc<Scope>) -> Self {
        Self { context, scope }
    }

    fn child_scope(&self, scope: Arc<Scope>) -> ClassBodyScopeRewriter<'a> {
        ClassBodyScopeRewriter::new(self.context, scope)
    }

    fn rewrite_class_def(&mut self, class_def: &mut ast::StmtClassDef) -> Stmt {
        let needs_class_cell = method::rewrite_explicit_super_classcell(class_def);

        let class_scope = self.scope.child_scope_for_class(class_def);

        let mut child = self.child_scope(class_scope.clone());
        child.visit_body(&mut class_def.body);

        let mut name_rewriter = ClassBodyNameRewriter::new(class_scope.clone());
        name_rewriter.visit_body(&mut class_def.body);
        rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(&mut class_def.body);
        bind_function_defs_to_class_ns(class_scope.clone(), &mut class_def.body);

        let rewrite = crate::transform::rewrite_class_def::rewrite(self.context, &class_scope, class_def, needs_class_cell);

        let stmts = match rewrite {
            Rewrite::Unmodified(stmt) => {
                vec![stmt]
            }
            Rewrite::Walk(stmts) => {
                stmts
            }
        };

        // TODO: make this a rewrite pass

        py_stmt_single(py_stmt!(r#"
if True:
    {stmts:stmt}
"#, stmts = stmts))
    }

    fn rewrite_function_def(&mut self, func_def: &mut ast::StmtFunctionDef) {
        let func_scope = self.scope.child_scope_for_function(func_def);
        let mut child = self.child_scope(func_scope.clone());
        child.visit_body(&mut func_def.body);
    }
}

impl<'a> Transformer for ClassBodyScopeRewriter<'a> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::ClassDef(class_def) => {
                *stmt = self.rewrite_class_def(class_def);
                return;
            }
            Stmt::FunctionDef(func_def) => {
                if func_def.name.id.as_str().starts_with("_dp_class_create_") {
                    return;
                }
                self.rewrite_function_def(func_def);
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
        let id = name.id.as_str();
        if id == "__dp__" || id == "_dp_class_ns" || id == "__classcell__" || is_dp_temp_name(id) {
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
        let id = name.id.as_str();
        if id == "__dp__" || id == "_dp_class_ns" || id == "__classcell__" || is_dp_temp_name(id) {
            return None;
        }

        match self.scope.binding_in_scope(id, BindingUse::Load) {
            BindingKind::Global => Some(class_body_store_global(id, name.ctx)),
            BindingKind::Nonlocal => None,
            BindingKind::Local => Some(class_body_store_target(id, name.ctx)),
        }
    }

    fn enclosing_function_binds_name(&self, name: &str) -> bool {
        let mut current = self.scope.parent_scope();
        while let Some(scope) = current {
            match scope.kind() {
                ScopeKind::Function { .. } => {
                    if let Some(binding) = scope.scope_bindings().get(name).copied() {
                        match binding {
                            BindingKind::Local | BindingKind::Nonlocal => return true,
                            BindingKind::Global => return false,
                        }
                    }
                }
                ScopeKind::Module => break,
                ScopeKind::Class { .. } => {}
            }
            current = scope.parent_scope();
        }
        false
    }
}

impl Transformer for ClassBodyNameRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Assign(ast::StmtAssign {
                targets,
                value,
                range,
                node_index,
            }) if targets.len() == 1 => {
                if let Expr::Name(name) = &targets[0] {
                    if is_dp_temp_name(name.id.as_str()) {
                        self.visit_expr(value.as_mut());
                        return;
                    }
                    let binding = self.scope.binding_in_scope(name.id.as_str(), BindingUse::Load);
                    let mut value = value.clone();
                    self.visit_expr(value.as_mut());
                    match binding {
                        BindingKind::Global => {
                            *stmt = py_stmt_single(py_stmt!(
                                "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                                name = name.id.as_str(),
                                value = value
                            ));
                            return;
                        }
                        BindingKind::Local => {
                            let target = class_body_store_target(name.id.as_str(), ExprContext::Store);
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
                if func_def.name.id.as_str().starts_with("_dp_class_create_") {
                    return;
                }
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
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target: _,
                annotation,
                value,
                ..
            }) => {
                if let Some(value) = value.as_mut() {
                    self.visit_expr(value);
                }
                self.visit_annotation(annotation);
                return;
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
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
            Expr::Generator(ast::ExprGenerator { generators, .. })
            | Expr::ListComp(ast::ExprListComp { generators, .. })
            | Expr::SetComp(ast::ExprSetComp { generators, .. })
            | Expr::DictComp(ast::ExprDictComp { generators, .. }) => {
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

fn bind_function_defs_to_class_ns(scope: Arc<Scope>, body: &mut Vec<Stmt>) {
    let mut binder = ClassBodyFunctionBinder { scope };
    binder.visit_body(body);
}

struct ClassBodyFunctionBinder {
    scope: Arc<Scope>,
}

impl ClassBodyFunctionBinder {
    fn method_qualname(
        &self,
        func_def: &ast::StmtFunctionDef,
        original_name: &str,
    ) -> String {
        if let Some(child_scope) = self.scope.lookup_child_scope(func_def) {
            return child_scope.make_qualname(original_name);
        }
        match self.scope.kind() {
            ScopeKind::Class { name } => {
                let class_qualname = self.scope.make_qualname(name.as_str());
                format!("{class_qualname}.{original_name}")
            }
            _ => original_name.to_string(),
        }
    }

    fn store_function(&self, name: &str, value: &str) -> Vec<Stmt> {
        match self.scope.binding_in_scope(name, BindingUse::Load) {
            BindingKind::Global => py_stmt!(
                "__dp__.store_global(globals(), {name:literal}, {value:id})",
                name = name,
                value = value
            ),
            BindingKind::Nonlocal => py_stmt!(
                "__dp__.store_cell({cell:id}, {value:id})",
                cell = name,
                value = value
            ),
            BindingKind::Local => py_stmt!(
                "_dp_class_ns[{name:literal}] = {value:id}",
                name = name,
                value = value
            ),
        }
    }
}

impl Transformer for ClassBodyFunctionBinder {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        for mut stmt in body.drain(..) {
            match stmt {
                Stmt::FunctionDef(mut func_def) => {
                    let original_name = func_def.name.id.to_string();
                    if original_name.starts_with("_dp_class_create_") {
                        new_body.push(Stmt::FunctionDef(func_def));
                        continue;
                    }
                    let qualname = self.method_qualname(&func_def, original_name.as_str());
                    let temp_name = format!("_dp_method_{original_name}");
                    func_def.name.id = Name::new(temp_name.as_str());
                    new_body.push(Stmt::FunctionDef(func_def));
                    new_body.extend(py_stmt!(
                        "__dp__.setattr({value:id}, \"__name__\", {name:literal})",
                        value = temp_name.as_str(),
                        name = original_name.as_str()
                    ));
                    new_body.extend(py_stmt!(
                        "__dp__.setattr({value:id}, \"__qualname__\", {qualname:literal})",
                        value = temp_name.as_str(),
                        qualname = qualname.as_str()
                    ));
                    new_body.extend(self.store_function(original_name.as_str(), temp_name.as_str()));
                }
                Stmt::ClassDef(class_def) => {
                    new_body.push(Stmt::ClassDef(class_def));
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
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }
}

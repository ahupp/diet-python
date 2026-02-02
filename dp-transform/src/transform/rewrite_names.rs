use std::{collections::HashSet, sync::Arc};

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt, StmtBody};

use crate::{
    py_expr,
    py_stmt,
    scope_aware_transformer::ScopeAwareTransformer,
    template::empty_body,
    transform::{
        rewrite_class_def::class_body::{
            class_body_load_cell,
            class_body_load_global,
            class_body_store_global,
            class_body_store_target,
        },
        scope::{is_internal_symbol, BindingKind, BindingUse, Scope, ScopeKind},
        util::is_noarg_call,
    },
};
use crate::transformer::{Transformer, walk_expr, walk_stmt};

pub fn rewrite_explicit_bindings(scope: Arc<Scope>, body: &mut StmtBody) {
    let mut rewriter = NameScopeRewriter::new(scope);
    rewriter.visit_body(body);
}

struct NameScopeRewriter {
    scope: Arc<Scope>,
}

impl ScopeAwareTransformer for NameScopeRewriter {
    fn scope(&self) -> &Arc<Scope> {
        &self.scope
    }

    fn enter_scope(&self, scope: Arc<Scope>) -> Self {
        Self { scope }
    }
}

impl NameScopeRewriter {
    fn new(scope: Arc<Scope>) -> Self {
        Self { scope }
    }

    fn is_class_scope(&self) -> bool {
        matches!(self.scope.kind(), ScopeKind::Class)
    }

    fn cell_init_needed(&self) -> bool {
        !self.cell_binding_names().is_empty()
    }

    fn insert_preamble(&self, body: &mut StmtBody, param_names: &HashSet<String>) {
       
        let body = &mut body.body;
        let mut stmts = Vec::new();

        if self.cell_init_needed() {
            // TODO: do we need to mut the underlying Scope?
            let mut names = self.cell_binding_names().into_iter().collect::<Vec<_>>();
            names.sort();
            for name in names {
                if param_names.contains(&name) {
                    stmts.push(py_stmt!(
                        "{name:id} = __dp__.make_cell({name:id})",
                        name = name.as_str()
                    ));
                } else {
                    stmts.push(py_stmt!("{name:id} = __dp__.make_cell()", name = name.as_str()));
                }
            }
        }
        if stmts.is_empty() {
            return;
        }
        let insert_at = match body.first().map(|stmt| stmt.as_ref()) {
            Some(Stmt::Expr(ast::StmtExpr { value, .. }))
                if matches!(value.as_ref(), Expr::StringLiteral(_)) =>
            {
                1
            }
            _ => 0,
        };
        body.splice(
            insert_at..insert_at,
            stmts.into_iter().map(Box::new),
        );
    }

    fn cell_binding_names(&self) -> HashSet<String> {
        self.scope
            .scope_bindings()
            .iter()
            .filter_map(|(name, kind)| {
                if matches!(kind, BindingKind::Nonlocal)
                    && self.scope.is_local_definition(name)
                    && !self.scope.is_explicit_nonlocal(name)
                {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn module_binds_name(&self, name: &str) -> bool {
        self.scope.any_parent_scope(|scope| {
            if matches!(scope.kind(), ScopeKind::Module) {
                return Some(scope.scope_bindings().contains_key(name));
            } else {
                None
            }
        }).unwrap_or(false)
    }

    fn should_rewrite_locals_call(&self) -> bool {
        if let Some(binding) = self.scope.scope_bindings().get("locals").copied() {
            match binding {
                BindingKind::Local | BindingKind::Nonlocal => return false,
                BindingKind::Global => {
                    if self.module_binds_name("locals") {
                        return false;
                    }
                }
            }
        }
        true
    }


    fn rewrite_name_load(&self, name: &ast::ExprName) -> Option<Expr> {

        let id = name.id.as_str();
        if is_internal_symbol(id) {
            return None;
        }

        let binding = self.scope.scope_bindings().get(id).copied();
        match (self.scope.kind(), binding) {
            (ScopeKind::Class, Some(BindingKind::Global)) => Some(class_body_load_global(id)),
            (ScopeKind::Class, Some(BindingKind::Nonlocal)) => Some(class_body_load_cell(id)),
            (ScopeKind::Class, Some(BindingKind::Local)) => Some(class_body_load_global(id)),
            (ScopeKind::Class, None) => Some(class_body_load_global(id)),
            (_, Some(BindingKind::Global)) => Some(py_expr!(
                "__dp__.load_global(globals(), {name:literal})",
                name = id
            )),
            (_, Some(BindingKind::Nonlocal)) => Some(py_expr!("__dp__.load_cell({name:id})", name = id)),
            (_, Some(BindingKind::Local)) => None,
            (_, None) => None,
        }
    }

    fn rewrite_name_store(&self, name: &ast::ExprName) -> Option<Expr> {
        let id = name.id.as_str();
        if is_internal_symbol(id) {
            return None;
        }

        match (self.scope.kind(), self.scope.binding_in_scope(id, BindingUse::Load)) {
            (ScopeKind::Class, BindingKind::Global) => Some(class_body_store_global(id, name.ctx)),
            (ScopeKind::Class, BindingKind::Nonlocal) => None,
            (ScopeKind::Class, BindingKind::Local) => Some(class_body_store_target(id, name.ctx)),
            (_, _) => None,
        }
    }

    fn rewrite_named_expr_any(&mut self, named: &mut ast::ExprNamed) -> Option<Expr> {

        let ast::ExprNamed { target, value, .. } = named;
        let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() else {
            return None;
        };

        let name = id.as_str();
        if is_internal_symbol(name) {
            return None;
        }

        self.visit_expr(value.as_mut());

        match self.scope.binding_in_scope(id.as_str(), BindingUse::Modify) {
            BindingKind::Global => {
                Some(py_expr!(
                    "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                    name = id.as_str(),
                    value = value.as_ref().clone()
                ))
            }
            BindingKind::Nonlocal => {
                Some(py_expr!(
                    "__dp__.store_cell({cell:id}, {value:expr})",
                    cell = id.as_str(),
                    value = value.as_ref().clone()
                ))
            },
            _ => None,
        }
    }

    fn is_class_lookup_call(expr: &Expr) -> bool {
        let Expr::Call(ast::ExprCall { func, .. }) = expr else {
            return false;
        };
        let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func.as_ref() else {
            return false;
        };
        let Expr::Name(ast::ExprName { id, .. }) = value.as_ref() else {
            return false;
        };
        id.as_str() == "__dp__"
            && matches!(attr.id.as_str(), "class_lookup_cell" | "class_lookup_global")
    }

}

fn collect_parameter_names(parameters: &ast::Parameters) -> HashSet<String> {
    let mut names = HashSet::new();
    for param in parameters.posonlyargs.iter() {
        names.insert(param.parameter.name.to_string());
    }
    for param in parameters.args.iter() {
        names.insert(param.parameter.name.to_string());
    }
    for param in parameters.kwonlyargs.iter() {
        names.insert(param.parameter.name.to_string());
    }
    if let Some(param) = &parameters.vararg {
        names.insert(param.name.to_string());
    }
    if let Some(param) = &parameters.kwarg {
        names.insert(param.name.to_string());
    }
    names
}


impl Transformer for NameScopeRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Delete(delete) => {

                assert!(delete.targets.len() == 1);

                let target = &mut delete.targets[0];
                self.visit_expr(target);
                if let Expr::Name(ast::ExprName { id, .. }) = &target {
                    let name = id.as_str();
                    if name == "__class__" {
                        return;
                    }
        
                    match self.scope.binding_in_scope(name, BindingUse::Load) {
                        BindingKind::Global => {
                            *stmt = py_stmt!(
                                "__dp__.delitem(globals(), {name:literal})",
                                name = name
                            );
                        }
                        BindingKind::Nonlocal => {
                            *stmt = py_stmt!("del {cell:id}.cell_contents", cell = name);
                        }
                        _ => {},
                    }
                }
            }
            Stmt::Global(_) | Stmt::Nonlocal(_) => {
                *stmt = empty_body().into();
            }
            Stmt::Assign(ast::StmtAssign {
                targets,
                value,
                ..
            }) => {
                assert!(targets.len() == 1);

                let mut target = targets[0].clone();
                if let Expr::Name(ast::ExprName { ctx, .. }) = &mut target {
                    *ctx = ExprContext::Store;
                }
        
                self.visit_expr(value.as_mut());

                if let Expr::Name(ast::ExprName { id, .. }) = &target {
                    if is_internal_symbol(id.as_str()) {
                        return;
                    }
                    let binding =
                        self.scope.binding_in_scope(id.as_str(), BindingUse::Load);

                    match (self.scope.kind(), binding) {
                        (ScopeKind::Class, BindingKind::Local) => {
                            *stmt = py_stmt!("_dp_class_ns[{name:literal}] = {value:expr}", name = id.as_str(), value = value.clone());
                        }
                        (_, BindingKind::Global) => {
                            *stmt = py_stmt!(
                                "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                                name = id.as_str(),
                                value = value.clone()
                            );
                        }
                        (_, BindingKind::Nonlocal) => {
                            *stmt = py_stmt!(
                                "__dp__.store_cell({cell:id}, {value:expr})",
                                cell = id.as_str(),
                                value = value.clone()
                            );
                        }
                        (_, _) => {},
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
        
                let child_scope = self
                    .scope
                    .child_scope_for_function(func_def)
                    .expect("no child scope for function");
        
                let mut child_rewriter = NameScopeRewriter::new(child_scope);
                child_rewriter.visit_body(&mut func_def.body);
                let param_names = collect_parameter_names(&func_def.parameters);
                child_rewriter.insert_preamble(&mut func_def.body, &param_names);
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
        
                let class_scope = self
                    .scope
                    .child_scope_for_class(class_def)
                    .expect("no child scope for class");
        
        
                NameScopeRewriter::new(class_scope).visit_body(&mut class_def.body);        
            }
            Stmt::AnnAssign(_) => {
                panic!("AnnAssign should be gone now");
            }
            _ => walk_stmt(self, stmt),
        }
    }


    fn visit_expr(&mut self, expr: &mut Expr) {
        if self.is_class_scope() {
            match expr {
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
        }
        match expr {
            Expr::Call(ast::ExprCall { .. }) => {
                if self.is_class_scope() {
                    if Self::is_class_lookup_call(expr) {
                        return;
                    }
                    if is_noarg_call("locals", expr) || is_noarg_call("vars", expr) {
                        *expr = py_expr!("_dp_class_ns");
                        return;
                    }
                } else if is_noarg_call("locals", expr) && self.should_rewrite_locals_call()
                {
                    *expr = py_expr!("__dp__.locals()");
                    return;
                }
            }
            Expr::Named(named) => {
                if let Some(rewritten) = self.rewrite_named_expr_any(named) {
                    *expr = rewritten;
                    return;
                }
            }
            Expr::Name(name) if matches!(name.ctx, ExprContext::Load) => {
                if let Some(rewritten) = self.rewrite_name_load(name) {
                    *expr = rewritten;
                }
                return;
            }
            Expr::Name(name) if matches!(name.ctx, ExprContext::Store | ExprContext::Del) => {
                if let Some(rewritten) = self.rewrite_name_store(name) {
                    *expr = rewritten;
                }
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}

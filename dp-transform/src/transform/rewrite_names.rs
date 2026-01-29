use std::{collections::HashSet, sync::Arc};

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt, StmtBody};

use crate::{
    py_expr, py_stmt, scope_aware_transformer::ScopeAwareTransformer, template::empty_body, transform::{scope::{BindingKind, BindingUse, Scope, ScopeKind}, util::is_noarg_call}
};
use crate::transformer::{Transformer, walk_expr, walk_stmt};

pub fn rewrite_explicit_bindings(scope: Arc<Scope>, body: &mut StmtBody) {
    let mut rewriter = NameScopeRewriter::new(scope);
    rewriter.visit_body(body);
}

struct NameScopeRewriter {
    scope: Arc<Scope>,
    extra_cell_names: HashSet<String>,
}

impl ScopeAwareTransformer for NameScopeRewriter {
    fn scope(&self) -> &Arc<Scope> {
        &self.scope
    }

    fn enter_scope(&self, scope: Arc<Scope>) -> Self {
        Self {
            scope,
            extra_cell_names: HashSet::new(),
        }
    }
}

impl NameScopeRewriter {
    fn new(scope: Arc<Scope>) -> Self {
        Self {
            scope,
            extra_cell_names: HashSet::new(),
        }
    }

    fn explicit_bindings_enabled(&self) -> bool {
        !matches!(self.scope.kind(), ScopeKind::Function)
    }

    fn needs_cell(&self, name: &str) -> bool {
        if !self.explicit_bindings_enabled() {
            return false;
        }
        matches!(
            self.scope.binding_in_scope(name, BindingUse::Load),
            BindingKind::Nonlocal
        )
            || self.scope.is_nonlocal_in_children(name)
            || self.extra_cell_names.contains(name)
    }

    fn needs_global(&self, name: &str) -> bool {
        if !self.explicit_bindings_enabled() {
            return false;
        }
        matches!(
            self.scope.binding_in_scope(name, BindingUse::Load),
            BindingKind::Global
        )
    }

    fn cell_init_needed(&self) -> bool {
        !self.scope.child_nonlocal_names().is_empty() || !self.extra_cell_names.is_empty()
    }

    fn insert_preamble(&self, body: &mut StmtBody, param_names: &HashSet<String>) {
        if !self.explicit_bindings_enabled() {
            return;
        }
        let body = &mut body.body;
        let mut stmts = Vec::new();

        if self.cell_init_needed() {
            // TODO: do we need to mut the underlying Scope?
            let mut names = self.scope.child_nonlocal_names();
            names.extend(self.extra_cell_names.iter().cloned());
            let mut names = names.into_iter().collect::<Vec<_>>();
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

    fn set_extra_cell_names(&mut self, names: HashSet<String>) {
        self.extra_cell_names = names;
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

    fn enclosing_function_binds_name(&self, name: &str) -> bool {
        self.scope.parent_scope().and_then(|scope| scope.any_parent_scope(|scope| {
            match scope.kind() {
                ScopeKind::Function => {
                    if matches!(
                        scope.scope_bindings().get(name),
                        Some(BindingKind::Local) | Some(BindingKind::Nonlocal)
                    ) {
                        return Some(true);
                    } else {
                        None
                    }
                }
                ScopeKind::Module => {
                    Some(false)
                }
                ScopeKind::Class => None,
            }
        })).unwrap_or(false)
    }

    fn should_rewrite_locals_call(&self) -> bool {
        if self.enclosing_function_binds_name("locals") {
            return false;
        }
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


    fn rewrite_function_def(&mut self, func_def: &mut ast::StmtFunctionDef) {
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

        let mut extra_cells = collect_named_expr_cells(&func_def.body);
        let declared_globals = collect_declared_globals(&func_def.body);
        extra_cells.retain(|name| {
            if matches!(
                child_scope.binding_in_scope(name, BindingUse::Load),
                BindingKind::Global | BindingKind::Nonlocal
            ) {
                return false;
            }
            if declared_globals.contains(name) {
                return false;
            }
            true
        });
        let mut child_rewriter = NameScopeRewriter::new(child_scope);
        child_rewriter.set_extra_cell_names(extra_cells);
        child_rewriter.visit_body(&mut func_def.body);
        let param_names = collect_parameter_names(&func_def.parameters);
        child_rewriter.insert_preamble(&mut func_def.body, &param_names);
    }

    fn rewrite_delete(&mut self, delete: ast::StmtDelete) -> Stmt {
        
        let ast::StmtDelete {
            mut targets,
            range,
            node_index,
        } = delete;
        assert!(targets.len() == 1);

        let target = &mut targets[0];
        self.visit_expr(target);
        if let Expr::Name(ast::ExprName { id, .. }) = &target {
            let name = id.as_str();
            if name == "__class__" {

                return Stmt::Delete(ast::StmtDelete {
                    targets: vec![target.clone()],
                    range,
                    node_index: node_index.clone(),
                });
            }
            if self.needs_global(name) {
                return py_stmt!(
                    "__dp__.delitem(globals(), {name:literal})",
                    name = name
                );
            }
            if self.needs_cell(name) {
                return py_stmt!("__dp__.del_cell({cell:id})", cell = name);
            }
        }
        Stmt::Delete(ast::StmtDelete {
            targets: vec![target.clone()],
            range,
            node_index: node_index.clone(),
        })

    }

    fn rewrite_class_def(&mut self, class_def: &mut ast::StmtClassDef) {
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


        let mut class_rewriter = NameScopeRewriter::new(class_scope);
        class_rewriter.rewrite_class_children(&mut class_def.body);
    }

    fn rewrite_class_children(&mut self, body: &mut StmtBody) {
        let mut new_body = Vec::with_capacity(body.body.len());
        for stmt in body.body.drain(..) {
            let mut stmt = *stmt;
            match &mut stmt {
                Stmt::FunctionDef(func_def) => {
                    self.rewrite_function_def(func_def);
                }
                Stmt::ClassDef(class_def) => {
                    self.rewrite_class_def(class_def);
                }
                _ => {}
            }
            new_body.push(Box::new(stmt));
        }
        body.body = new_body;
    }

    fn rewrite_load(&self, name: &ast::ExprName) -> Option<Expr> {
        let id = name.id.as_str();
        if id == "__dp__" {
            return None;
        }

        if self.needs_global(id) {
            Some(py_expr!(
                "__dp__.load_global(globals(), {name:literal})",
                name = id
            ))
        } else if self.needs_cell(id) {
            Some(py_expr!("__dp__.load_cell({name:id})", name = id))
        } else {
            None
        }
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


fn collect_named_expr_cells(body: &StmtBody) -> HashSet<String> {
    let mut collector = NamedExprComprehensionCollector::default();
    let mut cloned = body.clone();
    (&mut collector).visit_body(&mut cloned);
    collector.names
}

fn collect_declared_globals(body: &StmtBody) -> HashSet<String> {
    #[derive(Default)]
    struct GlobalCollector {
        names: HashSet<String>,
    }

    impl Transformer for GlobalCollector {
        fn visit_stmt(&mut self, stmt: &mut Stmt) {
            match stmt {
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => return,
                Stmt::Global(ast::StmtGlobal { names, .. }) => {
                    for name in names {
                        self.names.insert(name.id.to_string());
                    }
                    return;
                }
                _ => {}
            }
            walk_stmt(self, stmt);
        }
    }

    let mut collector = GlobalCollector::default();
    let mut cloned = body.clone();
    (&mut collector).visit_body(&mut cloned);
    collector.names
}

#[derive(Default)]
struct NamedExprComprehensionCollector {
    names: HashSet<String>,
}

impl NamedExprComprehensionCollector {
    fn collect_from_comprehension(&mut self, elt: &Expr, generators: &[ast::Comprehension]) {
        let mut collector = NamedExprTargetCollector::default();
        let mut elt_clone = elt.clone();
        (&mut collector).visit_expr(&mut elt_clone);
        for comp in generators {
            for if_expr in &comp.ifs {
                let mut if_clone = if_expr.clone();
                (&mut collector).visit_expr(&mut if_clone);
            }
            let mut iter_clone = comp.iter.clone();
            (&mut collector).visit_expr(&mut iter_clone);
        }
        self.names.extend(collector.names);
    }
}

impl Transformer for NamedExprComprehensionCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {
                return;
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Generator(ast::ExprGenerator { elt, generators, .. }) => {
                self.collect_from_comprehension(elt, generators);
                return;
            }
            Expr::Lambda(_) => {
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

#[derive(Default)]
struct NamedExprTargetCollector {
    names: HashSet<String>,
}

impl Transformer for NamedExprTargetCollector {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Named(ast::ExprNamed { target, value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    self.names.insert(id.as_str().to_string());
                } else {
                    self.visit_expr(target);
                }
                self.visit_expr(value);
                return;
            }
            Expr::Lambda(_)
            | Expr::Generator(_)=> {
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

impl Transformer for NameScopeRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Delete(delete) => {
                *stmt = self.rewrite_delete(delete.clone());
            }
            Stmt::Global(_) | Stmt::Nonlocal(_) => {
                if self.explicit_bindings_enabled() {
                    *stmt = empty_body().into();
                }
                return;
            }
            Stmt::Assign(ast::StmtAssign {
                targets,
                value,
                range,
                node_index,
            }) => {
                assert!(targets.len() == 1);

                let mut target = targets[0].clone();
        
                self.visit_expr(value.as_mut());
        
                if let Expr::Name(ast::ExprName { id, .. }) = &target {
                    if self.needs_global(id.as_str()) {
                        *stmt = py_stmt!(
                            "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                            name = id.as_str(),
                            value = value.clone()
                        );
                        return;
                    }
                    if self.needs_cell(id.as_str()) {
                        *stmt = py_stmt!(
                            "__dp__.store_cell({cell:id}, {value:expr})",
                            cell = id.as_str(),
                            value = value.clone()
                        );
                        return;
                    }
                }
        
                self.visit_expr(&mut target);

                *stmt = Stmt::Assign(ast::StmtAssign {
                    targets: vec![target],
                    value: value.clone(),
                    range: range.clone(),
                    node_index: node_index.clone(),
                });

            }
            Stmt::FunctionDef(func_def) => {
                self.rewrite_function_def(func_def);
            }
            Stmt::ClassDef(class_def) => {
                self.rewrite_class_def(class_def);
            }
            _ => walk_stmt(self, stmt),
        }
    }


    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(name) if matches!(name.ctx, ExprContext::Load) => {
                if let Some(rewritten) = self.rewrite_load(name) {
                    *expr = rewritten;
                }
                return;
            }
            Expr::Named(ast::ExprNamed { target, value, .. }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    if self.needs_global(id.as_str()) {
                        self.visit_expr(value.as_mut());
                        *expr = py_expr!(
                            "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                            name = id.as_str(),
                            value = value.as_ref().clone()
                        );
                        return;
                    }
                    if self.needs_cell(id.as_str()) {
                        self.visit_expr(value.as_mut());
                        *expr = py_expr!(
                            "__dp__.store_cell({cell:id}, {value:expr})",
                            cell = id.as_str(),
                            value = value.as_ref().clone()
                        );
                        return;
                    }
                }
            }
            Expr::Call(ast::ExprCall { func, arguments, .. }) => {
                if is_noarg_call("locals", func.as_ref()) && self.should_rewrite_locals_call() {
                    *expr = py_expr!("__dp__.locals()");
                    return;
                }
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}

use std::{collections::HashSet, sync::Arc};

use ruff_python_ast::{self as ast, name::Name, Expr, ExprContext, Stmt};

use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr, py_stmt,
    transform::scope::{BindingKind, Scope, ScopeKind},
};
use crate::namegen::fresh_name;

pub fn rewrite_names(scope: Arc<Scope>, body: &mut Vec<Stmt>) {
    let mut rewriter = NameScopeRewriter::new(scope);
    rewriter.visit_body(body);
}

struct NameScopeRewriter {
    scope: Arc<Scope>,
    extra_cell_names: HashSet<String>,
}

impl NameScopeRewriter {
    fn new(scope: Arc<Scope>) -> Self {
        Self {
            scope,
            extra_cell_names: HashSet::new(),
        }
    }

    fn needs_cell(&self, name: &str) -> bool {
        matches!(self.scope.binding_in_scope(name), Some(BindingKind::Nonlocal))
            || self.scope.is_nonlocal_in_children(name)
            || self.extra_cell_names.contains(name)
    }

    fn needs_global(&self, name: &str) -> bool {
        matches!(self.scope.binding_in_scope(name), Some(BindingKind::Global))
    }

    fn cell_init_needed(&self) -> bool {
        !self.scope.child_nonlocal_names().is_empty() || !self.extra_cell_names.is_empty()
    }

    fn insert_preamble(&self, body: &mut Vec<Stmt>) {
        let mut stmts = Vec::new();

        if self.cell_init_needed() {
            let mut names = self.scope.child_nonlocal_names();
            names.extend(self.extra_cell_names.iter().cloned());
            let mut names = names.into_iter().collect::<Vec<_>>();
            names.sort();
            for name in names {
                stmts.extend(py_stmt!("{name:id} = __dp__.make_cell()", name = name.as_str()));
            }
        }
        if stmts.is_empty() {
            return;
        }
        let insert_at = match body.first() {
            Some(Stmt::Expr(ast::StmtExpr { value, .. }))
                if matches!(value.as_ref(), Expr::StringLiteral(_)) =>
            {
                1
            }
            _ => 0,
        };
        body.splice(insert_at..insert_at, stmts);
    }

    fn set_extra_cell_names(&mut self, names: HashSet<String>) {
        self.extra_cell_names = names;
    }

    fn module_binds_name(&self, name: &str) -> bool {
        let mut current = Some(Arc::clone(&self.scope));
        while let Some(scope) = current {
            if matches!(scope.kind(), ScopeKind::Module) {
                return scope.scope_bindings().contains_key(name);
            }
            current = scope.parent_scope();
        }
        false
    }

    fn enclosing_function_binds_name(&self, name: &str) -> bool {
        let mut current = self.scope.parent_scope();
        while let Some(scope) = current {
            match scope.kind() {
                ScopeKind::Function { .. } => {
                    if matches!(
                        scope.binding_in_scope(name),
                        Some(BindingKind::Local | BindingKind::Nonlocal)
                    ) {
                        return true;
                    }
                }
                ScopeKind::Module => {
                    break;
                }
                ScopeKind::Class { .. } => {}
            }
            current = scope.parent_scope();
        }
        false
    }

    fn should_rewrite_locals_call(&self) -> bool {
        if self.enclosing_function_binds_name("locals") {
            return false;
        }
        match self.scope.binding_in_scope("locals") {
            Some(BindingKind::Local | BindingKind::Nonlocal) => return false,
            Some(BindingKind::Global) => {
                if self.module_binds_name("locals") {
                    return false;
                }
            }
            None => {
                if self.module_binds_name("locals") {
                    return false;
                }
            }
        }
        true
    }

    fn rewrite_assign(&mut self, assign: ast::StmtAssign) -> Vec<Stmt> {
        let ast::StmtAssign {
            mut targets,
            mut value,
            range,
            node_index,
        } = assign;
        assert!(targets.len() == 1);

        self.visit_expr(value.as_mut());

        if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
            if self.needs_global(id.as_str()) {
                return py_stmt!(
                    "__dp__.store_global(globals(), {name:literal}, {value:expr})",
                    name = id.as_str(),
                    value = value
                );
            }
            if self.needs_cell(id.as_str()) {
                return py_stmt!(
                    "__dp__.store_cell({cell:id}, {value:expr})",
                    cell = id.as_str(),
                    value = value
                );
            }
        }

        for target in &mut targets {
            self.visit_expr(target);
        }
        vec![Stmt::Assign(ast::StmtAssign {
            targets,
            value,
            range,
            node_index,
        })]
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
            .unwrap_or_else(|| self.scope.ensure_child_scope_for_function(func_def));
        let mut extra_cells = collect_named_expr_cells(&func_def.body);
        let declared_globals = collect_declared_globals(&func_def.body);
        extra_cells.retain(|name| {
            if matches!(
                child_scope.binding_in_scope(name),
                Some(BindingKind::Global | BindingKind::Nonlocal)
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
        child_rewriter.insert_preamble(&mut func_def.body);
        child_rewriter.visit_body(&mut func_def.body);
    }

    fn rewrite_function_def_binding(
        &mut self,
        mut func_def: ast::StmtFunctionDef,
        binding: BindingKind,
    ) -> Vec<Stmt> {
        let original_name = func_def.name.id.to_string();
        let mut decorators = std::mem::take(&mut func_def.decorator_list);
        let mut prefix = Vec::with_capacity(decorators.len());
        let mut decorator_names = Vec::with_capacity(decorators.len());
        for decorator in decorators.drain(..) {
            let temp = fresh_name("decorator");
            let mut expr = decorator.expression;
            self.visit_expr(&mut expr);
            prefix.extend(py_stmt!(
                "{temp:id} = {decorator:expr}",
                temp = temp.as_str(),
                decorator = expr
            ));
            decorator_names.push(temp);
        }

        let temp_name = fresh_name("fn");
        func_def.name.id = Name::new(temp_name.as_str());
        self.rewrite_function_def(&mut func_def);

        let scope_expr = if matches!(binding, BindingKind::Global) {
            py_expr!("None")
        } else {
            py_expr!("__dp__.nested_scope()")
        };

        let mut out = Vec::new();
        out.extend(prefix);
        out.push(Stmt::FunctionDef(func_def));

        if !decorator_names.is_empty() {
            let mut decorated = py_expr!("{func:id}", func = temp_name.as_str());
            for decorator in decorator_names.iter().rev() {
                decorated = py_expr!(
                    "{decorator:id}({decorated:expr})",
                    decorator = decorator.as_str(),
                    decorated = decorated
                );
            }
            out.extend(py_stmt!(
                "{func:id} = {decorated:expr}",
                func = temp_name.as_str(),
                decorated = decorated
            ));
        }

        match binding {
            BindingKind::Global => {
                out.extend(py_stmt!(
                    "__dp__.store_global(__globals__, {name:literal}, {value:id})",
                    name = original_name.as_str(),
                    value = temp_name.as_str()
                ));
            }
            BindingKind::Nonlocal => {
                out.extend(py_stmt!(
                    "__dp__.store_cell({cell:id}, {value:id})",
                    cell = original_name.as_str(),
                    value = temp_name.as_str()
                ));
            }
            BindingKind::Local => {}
        }
        out
    }

    fn rewrite_delete(&mut self, delete: ast::StmtDelete) -> Vec<Stmt> {
        let ast::StmtDelete {
            targets,
            range,
            node_index,
        } = delete;
        let mut out = Vec::with_capacity(targets.len());
        for target in targets {
            if let Expr::Name(ast::ExprName { id, .. }) = &target {
                let name = id.as_str();
                if name == "__class__" {
                    let mut target = target;
                    self.visit_expr(&mut target);
                    out.push(Stmt::Delete(ast::StmtDelete {
                        targets: vec![target],
                        range,
                        node_index: node_index.clone(),
                    }));
                    continue;
                }
                if self.needs_global(name) {
                    out.extend(py_stmt!(
                        "__dp__.delitem(__globals__, {name:literal})",
                        name = name
                    ));
                    continue;
                }
                if self.needs_cell(name) {
                    out.extend(py_stmt!("__dp__.del_deref({cell:id})", cell = name));
                    continue;
                }
            }
            let mut target = target;
            self.visit_expr(&mut target);
            out.push(Stmt::Delete(ast::StmtDelete {
                targets: vec![target],
                range,
                node_index: node_index.clone(),
            }));
        }
        out
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
            .unwrap_or_else(|| self.scope.ensure_child_scope_for_class(class_def));
        let mut class_rewriter = NameScopeRewriter::new(class_scope);
        class_rewriter.rewrite_class_children(&mut class_def.body);
    }

    fn rewrite_class_children(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        for mut stmt in body.drain(..) {
            match &mut stmt {
                Stmt::FunctionDef(func_def) => {
                    self.rewrite_function_def(func_def);
                }
                Stmt::ClassDef(class_def) => {
                    self.rewrite_class_def(class_def);
                }
                _ => {}
            }
            new_body.push(stmt);
        }
        *body = new_body;
    }

    fn rewrite_load(&self, name: &ast::ExprName) -> Option<Expr> {
        let id = name.id.as_str();
        if id == "__dp__" || id == "__globals__" {
            return None;
        }

        if self.needs_global(id) {
            Some(py_expr!(
                "__dp__.load_global(__globals__, {name:literal})",
                name = id
            ))
        } else if self.needs_cell(id) {
            Some(py_expr!("__dp__.load_cell({name:id})", name = id))
        } else {
            None
        }
    }
}


fn collect_named_expr_cells(body: &[Stmt]) -> HashSet<String> {
    let mut collector = NamedExprComprehensionCollector::default();
    let mut cloned = body.to_vec();
    collector.visit_body(&mut cloned);
    collector.names
}

fn collect_declared_globals(body: &[Stmt]) -> HashSet<String> {
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
    let mut cloned = body.to_vec();
    collector.visit_body(&mut cloned);
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
        collector.visit_expr(&mut elt_clone);
        for comp in generators {
            for if_expr in &comp.ifs {
                let mut if_clone = if_expr.clone();
                collector.visit_expr(&mut if_clone);
            }
            let mut iter_clone = comp.iter.clone();
            collector.visit_expr(&mut iter_clone);
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
            Expr::ListComp(ast::ExprListComp { elt, generators, .. }) => {
                self.collect_from_comprehension(elt, generators);
                return;
            }
            Expr::SetComp(ast::ExprSetComp { elt, generators, .. }) => {
                self.collect_from_comprehension(elt, generators);
                return;
            }
            Expr::DictComp(ast::ExprDictComp {
                key,
                value,
                generators,
                ..
            }) => {
                let elt = py_expr!("({key:expr}, {value:expr})", key = key.clone(), value = value.clone());
                self.collect_from_comprehension(&elt, generators);
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

impl Transformer for NameScopeRewriter {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        for stmt in body.drain(..) {
            match stmt {
                Stmt::Delete(delete) => {
                    new_body.extend(self.rewrite_delete(delete));
                }
                Stmt::Global(_) | Stmt::Nonlocal(_) => {
                    continue;
                }
                Stmt::Assign(assign) => {
                    new_body.extend(self.rewrite_assign(assign));
                }
                Stmt::FunctionDef(mut func_def) => {
                    let name = func_def.name.id.to_string();
                    if let Some(binding) = self.scope.binding_in_scope(&name) {
                        if matches!(binding, BindingKind::Global | BindingKind::Nonlocal) {
                            new_body.extend(self.rewrite_function_def_binding(func_def, binding));
                            continue;
                        }
                    }

                    self.rewrite_function_def(&mut func_def);
                    new_body.push(Stmt::FunctionDef(func_def));
                }
                Stmt::ClassDef(mut class_def) => {
                    self.rewrite_class_def(&mut class_def);
                    new_body.push(Stmt::ClassDef(class_def));
                }
                mut stmt => {
                    self.visit_stmt(&mut stmt);
                    new_body.push(stmt);
                }
            }
        }
        *body = new_body;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {
                // handled in visit_body
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
            Expr::Call(ast::ExprCall { func, arguments, .. }) => {
                if matches!(
                    func.as_ref(),
                    Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "locals"
                ) && arguments.args.is_empty()
                    && arguments.keywords.is_empty()
                    && self.should_rewrite_locals_call()
                {
                    *expr = py_expr!("__dp__.locals()");
                    return;
                }
            }
            _ => {}
        }

        crate::body_transform::walk_expr(self, expr);
    }
}


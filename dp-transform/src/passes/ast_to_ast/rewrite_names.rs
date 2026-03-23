use std::{collections::HashSet, sync::Arc};

use ruff_python_ast::name::Name;
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use super::{
    body::{suite_mut, take_suite, Suite},
    context::Context,
    semantic::SemanticAstState,
};
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{
    passes::ast_to_ast::{
        ast_rewrite::Rewrite,
        rewrite_class_def::class_body::{
            class_body_load_cell, class_body_load_global, class_body_store_global,
            class_body_store_target,
        },
        rewrite_import,
        scope::{cell_name, is_internal_symbol, BindingKind, BindingUse, Scope, ScopeKind},
        util::is_noarg_call,
    },
    passes::ruff_to_blockpy,
    py_expr, py_stmt,
};

pub fn rewrite_explicit_bindings(
    context: &Context,
    semantic_state: &SemanticAstState,
    body: &mut Suite,
) {
    let mut rewriter = NameScopeRewriter::new(context, semantic_state.module_scope());
    rewriter.visit_body(body);
}

fn is_annotation_function_name(name: &str) -> bool {
    name == "__annotate__"
        || name == "__annotate_func__"
        || name.starts_with("_dp_fn___annotate___")
        || name.starts_with("_dp_fn___annotate_func___")
}

struct NameScopeRewriter<'a> {
    context: &'a Context,
    scope: Arc<Scope>,
}

impl<'a> NameScopeRewriter<'a> {
    fn new(context: &'a Context, scope: Arc<Scope>) -> Self {
        Self { context, scope }
    }

    fn is_class_scope(&self) -> bool {
        matches!(self.scope.kind(), ScopeKind::Class)
    }

    fn cell_init_needed(&self) -> bool {
        !self.cell_binding_names().is_empty()
    }

    fn insert_preamble(&self, body: &mut Suite, param_names: &HashSet<String>) {
        let mut stmts = Vec::new();

        if self.cell_init_needed() {
            // TODO: do we need to mut the underlying Scope?
            let mut names = self.cell_binding_names().into_iter().collect::<Vec<_>>();
            names.sort();
            for name in names {
                let cell = cell_name(&name);
                if param_names.contains(&name) {
                    stmts.push(py_stmt!(
                        "{cell:id} = __dp_make_cell({name:id})",
                        cell = cell.as_str(),
                        name = name.as_str(),
                    ));
                } else {
                    stmts.push(py_stmt!(
                        "{cell:id} = __dp_make_cell()",
                        cell = cell.as_str()
                    ));
                }
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

    fn cell_binding_names(&self) -> HashSet<String> {
        self.scope.local_cell_bindings()
    }

    fn stmt_cell_sync_stmts(&self, stmt: &Stmt) -> Vec<Stmt> {
        let bind_name = match stmt {
            Stmt::FunctionDef(func_def) => Some(func_def.name.id.as_str()),
            _ => None,
        };
        let Some(bind_name) = bind_name else {
            return Vec::new();
        };
        if !self.cell_binding_names().contains(bind_name) {
            return Vec::new();
        }
        let cell = cell_name(bind_name);
        vec![py_stmt!(
            "__dp_store_cell({cell:id}, {name:id})",
            cell = cell.as_str(),
            name = bind_name,
        )]
    }

    fn module_binds_name(&self, name: &str) -> bool {
        self.scope
            .any_parent_scope(|scope| {
                if matches!(scope.kind(), ScopeKind::Module) {
                    return Some(scope.scope_bindings().contains_key(name));
                } else {
                    None
                }
            })
            .unwrap_or(false)
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

    fn should_rewrite_vars_call(&self) -> bool {
        if let Some(binding) = self.scope.scope_bindings().get("vars").copied() {
            match binding {
                BindingKind::Local | BindingKind::Nonlocal => return false,
                BindingKind::Global => {
                    if self.module_binds_name("vars") {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn should_rewrite_globals_call(&self) -> bool {
        if let Some(binding) = self.scope.scope_bindings().get("globals").copied() {
            match binding {
                BindingKind::Local | BindingKind::Nonlocal => return false,
                BindingKind::Global => {
                    if self.module_binds_name("globals") {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn should_rewrite_exec_call(&self) -> bool {
        if let Some(binding) = self.scope.scope_bindings().get("exec").copied() {
            match binding {
                BindingKind::Local | BindingKind::Nonlocal => return false,
                BindingKind::Global => {
                    if self.module_binds_name("exec") {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn should_rewrite_eval_call(&self) -> bool {
        if let Some(binding) = self.scope.scope_bindings().get("eval").copied() {
            match binding {
                BindingKind::Local | BindingKind::Nonlocal => return false,
                BindingKind::Global => {
                    if self.module_binds_name("eval") {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn should_rewrite_dir_call(&self) -> bool {
        if let Some(binding) = self.scope.scope_bindings().get("dir").copied() {
            match binding {
                BindingKind::Local | BindingKind::Nonlocal => return false,
                BindingKind::Global => {
                    if self.module_binds_name("dir") {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn is_name_call(name: &str, expr: &Expr) -> bool {
        let Expr::Call(ast::ExprCall { func, .. }) = expr else {
            return false;
        };
        let Expr::Name(ast::ExprName { id, .. }) = func.as_ref() else {
            return false;
        };
        id.as_str() == name
    }

    fn rewrite_name_load(&self, name: &ast::ExprName) -> Option<Expr> {
        let id = name.id.as_str();
        if is_internal_symbol(id) {
            return None;
        }

        let binding = self.scope.scope_bindings().get(id).copied();
        match (self.scope.kind(), binding) {
            (ScopeKind::Class, Some(BindingKind::Global)) => Some(class_body_load_global(id)),
            (ScopeKind::Class, Some(BindingKind::Nonlocal)) => {
                let cell = cell_name(id);
                Some(class_body_load_cell(id, cell.as_str()))
            }
            (ScopeKind::Class, Some(BindingKind::Local)) => Some(class_body_load_global(id)),
            (ScopeKind::Class, None) => Some(class_body_load_global(id)),
            (_, Some(BindingKind::Global)) => Some(py_expr!(
                "__dp_load_global(globals(), {name:literal})",
                name = id
            )),
            (_, Some(BindingKind::Nonlocal)) => {
                let cell = cell_name(id);
                Some(py_expr!("__dp_load_cell({cell:id})", cell = cell.as_str()))
            }
            (_, Some(BindingKind::Local)) => None,
            (_, None) => None,
        }
    }

    fn rewrite_name_store(&self, name: &ast::ExprName) -> Option<Expr> {
        let id = name.id.as_str();
        if is_internal_symbol(id) {
            return None;
        }

        match (
            self.scope.kind(),
            self.scope.binding_in_scope(id, BindingUse::Load),
        ) {
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
            BindingKind::Global => Some(py_expr!(
                "__dp_store_global(globals(), {name:literal}, {value:expr})",
                name = id.as_str(),
                value = value.as_ref().clone()
            )),
            BindingKind::Nonlocal => {
                let cell = cell_name(id.as_str());
                Some(py_expr!(
                    "__dp_store_cell({cell:id}, {value:expr})",
                    cell = cell.as_str(),
                    value = value.as_ref().clone()
                ))
            }
            _ => None,
        }
    }

    fn is_class_lookup_call(expr: &Expr) -> bool {
        let Expr::Call(ast::ExprCall { func, .. }) = expr else {
            return false;
        };
        if matches!(
            func.as_ref(),
            Expr::Name(ast::ExprName { id, .. })
                if matches!(
                    id.as_str(),
                    "__dp_class_lookup_cell" | "__dp_class_lookup_global"
                )
        ) {
            return true;
        }
        let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func.as_ref() else {
            return false;
        };
        let Expr::Name(ast::ExprName { id, .. }) = value.as_ref() else {
            return false;
        };
        id.as_str() == "__dp__"
            && matches!(
                attr.id.as_str(),
                "class_lookup_cell" | "class_lookup_global"
            )
    }

    fn loop_target_sync_stmts(&self, target_names: &[String]) -> Vec<Stmt> {
        let mut names = target_names.to_vec();
        names.sort();
        names.dedup();
        names
            .into_iter()
            .filter_map(|name| {
                if name == "__class__" || is_internal_symbol(name.as_str()) {
                    return None;
                }
                let value = py_expr!("{name:id}", name = name.as_str());
                let binding = self.scope.binding_in_scope(name.as_str(), BindingUse::Load);
                match (self.scope.kind(), binding) {
                    (ScopeKind::Class, BindingKind::Nonlocal) | (_, BindingKind::Nonlocal) => {
                        let cell = cell_name(name.as_str());
                        Some(py_stmt!(
                            "__dp_store_cell({cell:id}, {value:expr})",
                            cell = cell.as_str(),
                            value = value
                        ))
                    }
                    (_, BindingKind::Global) => Some(py_stmt!(
                        "__dp_store_global(globals(), {name:literal}, {value:expr})",
                        name = name.as_str(),
                        value = value
                    )),
                    _ => None,
                }
            })
            .collect()
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

fn collect_assigned_names(target: &Expr, names: &mut HashSet<String>) {
    match target {
        Expr::Name(name) => {
            names.insert(name.id.to_string());
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_assigned_names(elt, names);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_assigned_names(elt, names);
            }
        }
        Expr::Starred(starred) => collect_assigned_names(starred.value.as_ref(), names),
        _ => {}
    }
}

impl Transformer for NameScopeRewriter<'_> {
    fn visit_body(&mut self, body: &mut Suite) {
        let mut rewritten = Vec::with_capacity(body.len());
        for stmt in std::mem::take(body) {
            for mut stmt in self.rewrite_stmt_list(stmt) {
                self.visit_stmt(&mut stmt);
                let sync_stmts = self.stmt_cell_sync_stmts(&stmt);
                rewritten.push(stmt);
                rewritten.extend(sync_stmts);
            }
        }
        *body = rewritten;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::For(for_stmt) => {
                let mut target_names = HashSet::new();
                collect_assigned_names(for_stmt.target.as_ref(), &mut target_names);
                let target_names = target_names.into_iter().collect::<Vec<_>>();

                self.visit_expr(for_stmt.iter.as_mut());
                self.visit_expr(for_stmt.target.as_mut());
                self.visit_body(suite_mut(&mut for_stmt.body));
                self.visit_body(suite_mut(&mut for_stmt.orelse));

                let sync_stmts = self.loop_target_sync_stmts(&target_names);
                if !sync_stmts.is_empty() {
                    suite_mut(&mut for_stmt.body).splice(0..0, sync_stmts);
                }
            }
            Stmt::Delete(delete) => {
                assert!(delete.targets.len() == 1);

                let target = &mut delete.targets[0];
                if let Expr::Name(ast::ExprName { id, .. }) = &target {
                    let name = id.as_str();
                    if name == "__class__" {
                        return;
                    }
                    if is_internal_symbol(name) {
                        return;
                    }

                    match (
                        self.scope.kind(),
                        self.scope.binding_in_scope(name, BindingUse::Load),
                    ) {
                        (&ScopeKind::Class, BindingKind::Local) => {
                            *stmt =
                                py_stmt!("__dp_delitem(_dp_class_ns, {name:literal})", name = name);
                        }
                        (&ScopeKind::Class, BindingKind::Global) => {
                            *stmt =
                                py_stmt!("__dp_delitem(globals(), {name:literal})", name = name);
                        }
                        (&ScopeKind::Class, BindingKind::Nonlocal) => {
                            let cell = cell_name(name);
                            *stmt = py_stmt!("del {cell:id}.cell_contents", cell = cell.as_str());
                        }
                        (_, BindingKind::Global) => {
                            *stmt =
                                py_stmt!("__dp_delitem(globals(), {name:literal})", name = name);
                        }
                        (_, BindingKind::Nonlocal) => {
                            let cell = cell_name(name);
                            *stmt = py_stmt!("del {cell:id}.cell_contents", cell = cell.as_str());
                        }
                        _ => {}
                    }
                }
            }
            Stmt::Global(_) => return,
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    if name.id.as_str() == "__class__" {
                        continue;
                    }
                    let cell = cell_name(name.id.as_str());
                    name.id = Name::new(cell);
                }
            }
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
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
                    let binding = self.scope.binding_in_scope(id.as_str(), BindingUse::Load);

                    match (self.scope.kind(), binding) {
                        (ScopeKind::Class, BindingKind::Local) => {
                            *stmt = py_stmt!(
                                "_dp_class_ns[{name:literal}] = {value:expr}",
                                name = id.as_str(),
                                value = value.clone()
                            );
                        }
                        (_, BindingKind::Global) => {
                            *stmt = py_stmt!(
                                "__dp_store_global(globals(), {name:literal}, {value:expr})",
                                name = id.as_str(),
                                value = value.clone()
                            );
                        }
                        (_, BindingKind::Nonlocal) => {
                            let cell = cell_name(id.as_str());
                            *stmt = py_stmt!(
                                "__dp_store_cell({cell:id}, {value:expr})",
                                cell = cell.as_str(),
                                value = value.clone()
                            );
                        }
                        (_, _) => {}
                    }
                }
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(suite_mut(&mut try_stmt.body));
                for handler in &mut try_stmt.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_);
                    }
                    if let Some(name) = handler.name.as_mut() {
                        let exc_name = name.id.as_str().to_string();
                        if exc_name != "__class__" && !is_internal_symbol(&exc_name) {
                            let binding = self
                                .scope
                                .binding_in_scope(exc_name.as_str(), BindingUse::Load);
                            let needs_rewrite = matches!(
                                (self.scope.kind(), binding),
                                (&ScopeKind::Class, BindingKind::Local)
                                    | (&ScopeKind::Class, BindingKind::Global)
                                    | (&ScopeKind::Class, BindingKind::Nonlocal)
                                    | (_, BindingKind::Global)
                                    | (_, BindingKind::Nonlocal)
                            );
                            if needs_rewrite {
                                let temp_name = format!("_dp_exc_{exc_name}");
                                name.id = Name::new(temp_name.as_str());
                                let store_stmt = match (self.scope.kind(), binding) {
                                    (&ScopeKind::Class, BindingKind::Local) => py_stmt!(
                                        "_dp_class_ns[{name:literal}] = {value:expr}",
                                        name = exc_name.as_str(),
                                        value = py_expr!("{temp:id}", temp = temp_name.as_str()),
                                    ),
                                    (&ScopeKind::Class, BindingKind::Global) => py_stmt!(
                                        "__dp_store_global(globals(), {name:literal}, {value:expr})",
                                        name = exc_name.as_str(),
                                        value = py_expr!("{temp:id}", temp = temp_name.as_str()),
                                    ),
                                    (&ScopeKind::Class, BindingKind::Nonlocal) => {
                                        let cell = cell_name(exc_name.as_str());
                                        py_stmt!(
                                            "__dp_store_cell({cell:id}, {value:expr})",
                                            cell = cell.as_str(),
                                            value = py_expr!("{temp:id}", temp = temp_name.as_str()),
                                        )
                                    }
                                    (_, BindingKind::Global) => py_stmt!(
                                        "__dp_store_global(globals(), {name:literal}, {value:expr})",
                                        name = exc_name.as_str(),
                                        value = py_expr!("{temp:id}", temp = temp_name.as_str()),
                                    ),
                                    (_, BindingKind::Nonlocal) => {
                                        let cell = cell_name(exc_name.as_str());
                                        py_stmt!(
                                            "__dp_store_cell({cell:id}, {value:expr})",
                                            cell = cell.as_str(),
                                            value = py_expr!("{temp:id}", temp = temp_name.as_str()),
                                        )
                                    }
                                    _ => py_stmt!("pass"),
                                };
                                let delete_stmt = match (self.scope.kind(), binding) {
                                    (&ScopeKind::Class, BindingKind::Local) => py_stmt!(
                                        "__dp_delitem(_dp_class_ns, {name:literal})",
                                        name = exc_name.as_str()
                                    ),
                                    (&ScopeKind::Class, BindingKind::Global)
                                    | (_, BindingKind::Global) => {
                                        py_stmt!(
                                            "__dp_delitem(globals(), {name:literal})",
                                            name = exc_name.as_str()
                                        )
                                    }
                                    (&ScopeKind::Class, BindingKind::Nonlocal)
                                    | (_, BindingKind::Nonlocal) => {
                                        let cell = cell_name(exc_name.as_str());
                                        py_stmt!(
                                            "del {cell:id}.cell_contents",
                                            cell = cell.as_str()
                                        )
                                    }
                                    _ => py_stmt!("pass"),
                                };
                                let original_body = take_suite(&mut handler.body);
                                let wrapped = py_stmt!(
                                    r#"
try:
    {body:stmt}
finally:
    {delete:stmt}
"#,
                                    body = original_body,
                                    delete = delete_stmt,
                                );
                                *suite_mut(&mut handler.body) = vec![store_stmt, wrapped];
                            }
                        }
                    }
                    self.visit_body(suite_mut(&mut handler.body));
                }
                self.visit_body(suite_mut(&mut try_stmt.orelse));
                self.visit_body(suite_mut(&mut try_stmt.finalbody));
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
                if is_annotation_function_name(func_def.name.id.as_str()) {
                    return;
                }

                let child_scope = self
                    .scope
                    .child_scope_for_function(func_def)
                    .expect("no child scope for function");

                let mut child_rewriter = NameScopeRewriter::new(self.context, child_scope);
                child_rewriter.visit_body(suite_mut(&mut func_def.body));
                let param_names = collect_parameter_names(&func_def.parameters);
                child_rewriter.insert_preamble(suite_mut(&mut func_def.body), &param_names);
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

                NameScopeRewriter::new(self.context, class_scope)
                    .visit_body(suite_mut(&mut class_def.body));
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
                if Self::is_name_call("exec", expr) && self.should_rewrite_exec_call() {
                    if let Expr::Call(ast::ExprCall { func, .. }) = expr {
                        *func = Box::new(py_expr!("__dp_exec_"));
                    }
                }
                if Self::is_name_call("eval", expr) && self.should_rewrite_eval_call() {
                    if let Expr::Call(ast::ExprCall { func, .. }) = expr {
                        *func = Box::new(py_expr!("__dp_eval_"));
                    }
                }
                if self.is_class_scope() {
                    if Self::is_class_lookup_call(expr) {
                        return;
                    }
                    if is_noarg_call("locals", expr) && self.should_rewrite_locals_call() {
                        *expr = py_expr!(
                            "__dp_unsupported_implicit_locals({feature:literal})",
                            feature = "locals()",
                        );
                        return;
                    }
                    if is_noarg_call("vars", expr) && self.should_rewrite_vars_call() {
                        *expr = py_expr!("_dp_class_ns");
                        return;
                    }
                    if is_noarg_call("globals", expr) {
                        *expr = py_expr!("__dp_globals()");
                        return;
                    }
                } else if is_noarg_call("locals", expr) && self.should_rewrite_locals_call() {
                    *expr = py_expr!(
                        "__dp_unsupported_implicit_locals({feature:literal})",
                        feature = "locals()",
                    );
                    return;
                } else if is_noarg_call("vars", expr) && self.should_rewrite_vars_call() {
                    *expr = py_expr!(
                        "__dp_unsupported_implicit_locals({feature:literal})",
                        feature = "vars()",
                    );
                    return;
                } else if is_noarg_call("dir", expr) && self.should_rewrite_dir_call() {
                    *expr = py_expr!(
                        "__dp_unsupported_implicit_locals({feature:literal})",
                        feature = "dir()",
                    );
                    return;
                } else if is_noarg_call("globals", expr) && self.should_rewrite_globals_call() {
                    *expr = py_expr!("__dp_globals()");
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

impl NameScopeRewriter<'_> {
    fn rewrite_stmt_list(&self, stmt: Stmt) -> Vec<Stmt> {
        match stmt {
            Stmt::Import(import) => self.rewrite_nested_stmt_list(rewrite_import::rewrite(import)),
            Stmt::ImportFrom(import_from) => self
                .rewrite_nested_stmt_list(rewrite_import::rewrite_from(self.context, import_from)),
            Stmt::TypeAlias(type_alias) => self.rewrite_nested_stmt_list(
                ruff_to_blockpy::rewrite_type_alias_stmt(self.context, type_alias),
            ),
            Stmt::AugAssign(augassign) => self.rewrite_nested_stmt_list(
                ruff_to_blockpy::rewrite_augassign_stmt(self.context, augassign),
            ),
            other => vec![other],
        }
    }

    fn rewrite_nested_stmt_list(&self, rewrite: Rewrite) -> Vec<Stmt> {
        match rewrite {
            Rewrite::Unmodified(stmt) => vec![stmt],
            Rewrite::Walk(stmts) => stmts
                .into_iter()
                .flat_map(|stmt| self.rewrite_stmt_list(stmt))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::rewrite_explicit_bindings;
    use crate::passes::ast_to_ast::context::Context;
    use crate::passes::ast_to_ast::scope::analyze_module_scope;
    use crate::passes::ast_to_ast::semantic::SemanticAstState;
    use crate::passes::ast_to_ast::Options;
    use ruff_python_parser::parse_module;

    #[test]
    fn recursive_local_function_syncs_function_binding_into_cell() {
        let source = concat!(
            "def outer():\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    return recurse()\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let semantic_state = SemanticAstState::new(analyze_module_scope(&mut module));
        rewrite_explicit_bindings(&context, &semantic_state, &mut module);
        let rendered = module
            .iter()
            .map(crate::ruff_ast_to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
            "{rendered}"
        );
    }

    #[test]
    fn nested_class_binding_does_not_emit_stale_local_cell_sync() {
        let source = concat!(
            "def outer():\n",
            "    class A:\n",
            "        pass\n",
            "    class B:\n",
            "        def probe(self):\n",
            "            return A\n",
            "    return B\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let semantic_state = SemanticAstState::new(analyze_module_scope(&mut module));
        rewrite_explicit_bindings(&context, &semantic_state, &mut module);
        let rendered = module
            .iter()
            .map(crate::ruff_ast_to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !rendered.contains("__dp_store_cell(_dp_cell_A, A)"),
            "{rendered}"
        );
    }
}

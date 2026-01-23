use std::{collections::HashSet, mem};

use ruff_python_ast::{
    self as ast, Expr, ExprContext, Stmt, TypeParam, TypeParamParamSpec,
    TypeParamTypeVar, TypeParamTypeVarTuple,
};

use crate::template::py_stmt_single;
use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr, py_stmt,
    transform::context::ScopeInfo,
};
use super::class_body_load;
use crate::transform::util::is_noarg_call;

pub fn rewrite_class_scope(
    qualname: String,
    body: &mut Vec<Stmt>,
    scope: ScopeInfo,
    type_params: HashSet<String>,
) {
    let mut rewriter = ClassScopeRewriter::new(
        qualname,
        scope,
        type_params,
    );
    rewriter.visit_body(body);
}

struct ClassScopeRewriter {
    qualname: String,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
    type_params: HashSet<String>,
    update_fn_target: Option<String>,
    decorated_names: HashSet<String>,
    function_defs: HashSet<String>,
}

impl ClassScopeRewriter {
    fn new(
        qualname: String,
        scope: ScopeInfo,
        type_params: HashSet<String>,
    ) -> Self {
        let globals = scope.global_names();
        let nonlocals = scope.nonlocal_names();
        Self {
            qualname,
            globals,
            nonlocals,
            type_params,
            update_fn_target: None,
            decorated_names: HashSet::new(),
            function_defs: HashSet::new(),
        }
    }

    fn should_rewrite(&self, name: &str) -> bool {
        !self.globals.contains(name)
            && !self.nonlocals.contains(name)
            && !self.type_params.contains(name)
            && !name.starts_with("_dp_")
            && !matches!(name, "__dp__"  | "globals")
    }

    fn is_update_fn_assign(&self, name: &str, value: &Expr) -> bool {
        let Expr::Call(ast::ExprCall { func, arguments, .. }) = value else {
            return false;
        };
        let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func.as_ref() else {
            return false;
        };
        if attr.as_str() != "update_fn" {
            return false;
        }
        let Expr::Name(ast::ExprName { id, .. }) = value.as_ref() else {
            return false;
        };
        if id.as_str() != "__dp__" {
            return false;
        }
        match arguments.args.first() {
            Some(Expr::Name(ast::ExprName { id, .. })) => id.as_str() == name,
            _ => false,
        }
    }

    fn is_decorator_assign(&self, name: &str, value: &Expr) -> bool {
        let mut expr = value;
        loop {
            let Expr::Call(ast::ExprCall { arguments, .. }) = expr else {
                return false;
            };
            if !arguments.keywords.is_empty() || arguments.args.len() != 1 {
                return false;
            }
            let arg = &arguments.args[0];
            match arg {
                Expr::Call(_) => {
                    expr = arg;
                    continue;
                }
                Expr::Name(ast::ExprName { id, .. }) => return id.as_str() == name,
                _ => return false,
            }
        }
    }

    fn rewrite_decorator_assign_value(&mut self, value: &mut Expr, target: &str) {
        let Expr::Call(ast::ExprCall { func, arguments, .. }) = value else {
            return;
        };
        self.visit_expr(func);
        if arguments.args.len() == 1 && arguments.keywords.is_empty() {
            if let Some(arg) = arguments.args.first_mut() {
                match arg {
                    Expr::Call(_) => self.rewrite_decorator_assign_value(arg, target),
                    Expr::Name(ast::ExprName { id, .. }) if id.as_str() == target => {
                        if !self.function_defs.contains(target) {
                            *arg = class_body_load(target);
                        }
                    }
                    _ => self.visit_expr(arg),
                }
            }
        } else {
            for arg in &mut arguments.args {
                self.visit_expr(arg);
            }
            for keyword in &mut arguments.keywords {
                self.visit_expr(&mut keyword.value);
            }
        }
    }

}

fn collect_type_param_names(type_params: &ast::TypeParams) -> HashSet<String> {
    let mut names = HashSet::new();
    for param in &type_params.type_params {
        match param {
            TypeParam::TypeVar(TypeParamTypeVar { name, .. })
            | TypeParam::TypeVarTuple(TypeParamTypeVarTuple { name, .. })
            | TypeParam::ParamSpec(TypeParamParamSpec { name, .. }) => {
                names.insert(name.id.to_string());
            }
        }
    }
    names
}

impl Transformer for ClassScopeRewriter {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        self.decorated_names.clear();
        self.function_defs.clear();
        for stmt in body.iter() {
            if let Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) = stmt {
                self.function_defs.insert(name.id.to_string());
            }
            if let Stmt::Assign(ast::StmtAssign { targets, value, .. }) = stmt {
                if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        if self.is_decorator_assign(id.as_str(), value.as_ref()) {
                            self.decorated_names.insert(id.as_str().to_string());
                        }
                    }
                }
            }
        }
        for stmt in mem::take(body) {
            match stmt {
                Stmt::TypeAlias(mut alias) => {
                    let name_str = if let Expr::Name(ast::ExprName { id, .. }) = &*alias.name {
                        id.as_str().to_string()
                    } else {
                        let mut stmt = Stmt::TypeAlias(alias);
                        self.visit_stmt(&mut stmt);
                        new_body.push(stmt);
                        continue;
                    };
                    if let Some(type_params) = alias.type_params.as_mut() {
                        self.visit_type_params(type_params);
                    }
                    self.visit_expr(alias.value.as_mut());
                    let tmp_name = format!("_dp_type_alias_{name_str}");
                    let alias_stmt = Stmt::TypeAlias(alias);
                    let mut helper = py_stmt!(
                        r#"
def {tmp:id}():
    {alias_stmt:stmt}
    return {name:id}
{name:id} = {tmp:id}()    
"#,
                        tmp = tmp_name.as_str(),
                        alias_stmt = vec![alias_stmt],
                        name = name_str.as_str(),
                        tmp = tmp_name.as_str(),
                    );
                    for stmt in &mut helper {
                        self.visit_stmt(stmt);
                    }
                    new_body.extend(helper);
                }
                Stmt::FunctionDef(func_def) => {
                    let func_name = func_def.name.id.clone();

                    let mut stmt = Stmt::FunctionDef(func_def.clone());
                    self.visit_stmt(&mut stmt);
                    new_body.push(stmt);
                    let mut with_assign = py_stmt!(r#"
{func_name:id} = __dp__.update_fn({func_name:id}, {qualname:literal}, {func_name:literal})
"#, func_name = func_name.as_str(), qualname = self.qualname.as_str());
                    for stmt in &mut with_assign {
                        self.visit_stmt(stmt);
                    }
                    new_body.extend(with_assign);
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
            Stmt::FunctionDef(ast::StmtFunctionDef {
                name,
                decorator_list,
                parameters,
                returns,
                type_params,
                body,
                ..
            }) => {
                // Only visit outer parts of function, not the body.

                assert!(decorator_list.is_empty(), "decorators should be rewritten to assign");
                let saved_type_params = self.type_params.clone();
                if let Some(type_params) = type_params {
                    self.type_params.extend(collect_type_param_names(type_params));
                    self.visit_type_params(type_params);
                }
                self.visit_parameters(parameters);
                if let Some(expr) = returns {
                    self.visit_annotation(expr);
                }
                if name.id.as_str() == "__annotate__" {
                    for stmt in body {
                        self.visit_stmt(stmt);
                    }
                }
                self.type_params = saved_type_params;
            }
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                assert!(targets.len() == 1);
                if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                    let name = id.as_str();
                    if self.should_rewrite(name) {
                        *stmt = py_stmt_single(py_stmt!("del _dp_class_ns[{name:literal}]", name = name));
                    }
                }
                walk_stmt(self, stmt);
            }
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                assert!(targets.len() == 1);
                if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                    let name = id.as_str();
                    if self.should_rewrite(name) {
                        if self.is_update_fn_assign(name, value.as_ref()) {
                            let saved = self.update_fn_target.take();
                            self.update_fn_target = Some(name.to_string());
                            self.visit_expr(value);
                            self.update_fn_target = saved;
                            if self.decorated_names.contains(name) {
                                return;
                            }
                        }
                        if self.is_decorator_assign(name, value.as_ref()) {
                            self.rewrite_decorator_assign_value(value, name);
                            *stmt = py_stmt_single(py_stmt!(
                                "_dp_class_ns[{name:literal}] = {value:expr}",
                                name = name,
                                value = value.clone()
                            ));
                            return;
                        }
                        let saved = self.update_fn_target.take();
                        self.update_fn_target = Some(name.to_string());
                        self.visit_expr(value);
                        self.update_fn_target = saved;
                        *stmt = py_stmt_single(py_stmt!("_dp_class_ns[{name:literal}] = {value:expr}", name = name, value = value.clone()));
                        return;
                    }
                }
                walk_stmt(self, stmt);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if is_noarg_call("vars", expr) || is_noarg_call("locals", expr) {
            *expr = py_expr!("_dp_class_ns");
            return;
        }
        if let Expr::Call(ast::ExprCall { func, arguments, .. }) = expr {
            if let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func.as_ref() {
                if attr.as_str() == "update_fn" {
                    if let Expr::Name(ast::ExprName { id, .. }) = value.as_ref() {
                        if id.as_str() == "__dp__" {
                            let target = self.update_fn_target.as_deref();
                            if let Some(target_name) = target {
                                if let Some(first_arg) = arguments.args.first() {
                                    if matches!(first_arg, Expr::Name(ast::ExprName { id, .. }) if id.as_str() == target_name) {
                                        if let Some(first) = arguments.args.first_mut() {
                                            // Skip rewriting the local function binding used to update metadata.
                                            if let Expr::Name(_) = first {
                                                for arg in arguments.args.iter_mut().skip(1) {
                                                    self.visit_expr(arg);
                                                }
                                                for keyword in &mut arguments.keywords {
                                                    self.visit_expr(&mut keyword.value);
                                                }
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            let name = id.as_str().to_string();
            let name_str = name.as_str();
            if self.type_params.contains(name_str) {
                return;
            }
            if !self.should_rewrite(name_str) {
                return;
            }
            if *ctx == ExprContext::Load {
                *expr = class_body_load(name_str);
                return;
            } 
        }
        walk_expr(self, expr);
    }
}

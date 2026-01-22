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
        }
    }

    fn should_rewrite(&self, name: &str) -> bool {
        !self.globals.contains(name)
            && !self.nonlocals.contains(name)
            && !self.type_params.contains(name)
            && !name.starts_with("_dp_")
            && !matches!(name, "__dp__"  | "globals")
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
                    let helper = py_stmt!(
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
                    self.visit_body(&mut with_assign);
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
                        *stmt = py_stmt_single(py_stmt!("_dp_class_ns[{name:literal}] = {value:expr}", name = name, value = value.clone()));
                    }
                }
                walk_stmt(self, stmt);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if is_noarg_call("vars", expr) || is_noarg_call("locals", expr) {
            *expr = py_expr!("lambda: _dp_class_ns");
            return;
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

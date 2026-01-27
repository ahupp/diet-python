use std::{collections::HashSet, mem};

use ruff_python_ast::{
    self as ast, Expr, ExprContext, Stmt, TypeParam, TypeParamParamSpec,
    TypeParamTypeVar, TypeParamTypeVarTuple,
};
use ruff_python_ast::name::Name;

use crate::template::py_stmt_single;
use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr, py_stmt,
};
use super::{class_body_load};
use crate::transform::util::is_noarg_call;
use crate::namegen::{fresh_name};

pub fn rewrite_class_scope(
    body: &mut Vec<Stmt>,
    type_params: HashSet<String>,
) {
    let mut rewriter = ClassScopeRewriter::new(
        type_params,
    );
    rewriter.visit_body(body);
}

struct ClassScopeRewriter {
    type_params: HashSet<String>,
    decorated_names: HashSet<String>,
    function_defs: HashSet<String>,
}

impl ClassScopeRewriter {
    fn new(
        type_params: HashSet<String>,
    ) -> Self {
        Self {
            type_params,
            decorated_names: HashSet::new(),
            function_defs: HashSet::new(),
        }
    }

    fn should_rewrite(&self, name: &str) -> bool {
            !self.type_params.contains(name)
            && !name.contains('$')
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
        self.decorated_names.clear();
        self.function_defs.clear();
        for stmt in body.iter() {
            if let Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) = stmt {
                self.function_defs.insert(name.id.to_string());
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
                let original_name = func_def.name.id.to_string();
                if original_name.starts_with("_dp_class_")
                    || original_name.starts_with("_dp_class_ns_")
                {
                    let mut stmt = Stmt::FunctionDef(func_def.clone());
                    self.visit_stmt(&mut stmt);
                    new_body.push(stmt);
                    continue;
                }

                let mut func_def = func_def.clone();
                let decorators = std::mem::take(&mut func_def.decorator_list);
                let local_name = fresh_name("fn");

                func_def.name.id = Name::new(local_name.as_str());

                let mut prefix = Vec::with_capacity(decorators.len());
                let mut decorator_names = Vec::with_capacity(decorators.len());
                for decorator in decorators {
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

                let mut stmt = Stmt::FunctionDef(func_def);
                self.visit_stmt(&mut stmt);
                new_body.extend(prefix);
                new_body.push(stmt);

                let mut decorated = py_expr!("{func:id}", func = local_name.as_str());
                for decorator in decorator_names.iter().rev() {
                    decorated = py_expr!(
                        "{decorator:id}({decorated:expr})",
                        decorator = decorator.as_str(),
                        decorated = decorated
                    );
                }
                new_body.extend(py_stmt!(
                    r#"_dp_class_ns[{name:literal}] = {decorated:expr}"#,
                    name = original_name.as_str(),
                    decorated = decorated
                ));
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
            Stmt::ClassDef(_) => {
                if let Stmt::ClassDef(ast::StmtClassDef {
                    decorator_list,
                    arguments,
                    type_params,
                    ..
                }) = stmt
                {
                    for decorator in decorator_list {
                        self.visit_decorator(decorator);
                    }
                    if let Some(type_params) = type_params {
                        self.visit_type_params(type_params);
                    }
                    if let Some(arguments) = arguments {
                        self.visit_arguments(arguments);
                    }
                }
                return;
            }
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                assert!(targets.len() == 1);
                if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                    let name = id.as_str();
                    if self.should_rewrite(name) {
                        *stmt = py_stmt_single(py_stmt!("del _dp_class_ns[{name:literal}]", name = name));
                        return;
                    }
                }
                walk_stmt(self, stmt);
            }
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                assert!(targets.len() == 1);
                if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                    let name = id.as_str();
                    if self.should_rewrite(name) {
                        self.visit_expr(value);
                        *stmt = py_stmt_single(py_stmt!(
                            "_dp_class_ns[{name:literal}] = {value:expr}",
                            name = name,
                            value = value.clone()
                        ));
                        return;
                    }
                }
                walk_stmt(self, stmt);
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::ListComp(ast::ExprListComp { generators, .. })
            | Expr::SetComp(ast::ExprSetComp { generators, .. })
            | Expr::Generator(ast::ExprGenerator { generators, .. }) => {
                if let Some(first) = generators.first_mut() {
                    self.visit_expr(&mut first.iter);
                }
                return;
            }
            Expr::DictComp(ast::ExprDictComp { generators, .. }) => {
                if let Some(first) = generators.first_mut() {
                    self.visit_expr(&mut first.iter);
                }
                return;
            }
            Expr::Lambda(_) => {
                return;
            }
            _ => {}
        }

        if is_noarg_call("vars", expr) || is_noarg_call("locals", expr) {
            *expr = py_expr!("_dp_class_ns");
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

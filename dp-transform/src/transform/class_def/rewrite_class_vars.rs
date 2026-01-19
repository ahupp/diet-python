use std::{collections::HashSet, mem};

use ruff_python_ast::{
    self as ast, name::Name, Expr, ExprContext, Stmt, TypeParam, TypeParamParamSpec,
    TypeParamTypeVar, TypeParamTypeVarTuple,
};

use crate::template::py_stmt_single;
use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr, py_stmt,
    transform::context::ScopeInfo,
};

pub fn rewrite_class_scope(
    qualname: String,
    body: &mut Vec<Stmt>,
    scope: ScopeInfo,
    type_params: HashSet<String>,
    has_enclosing_class_cell: bool,
) {
    let locals = scope.local_names();
    let mut rewriter = ClassScopeRewriter::new(
        qualname,
        scope,
        locals,
        type_params,
        has_enclosing_class_cell,
    );
    rewriter.visit_body(body);
}

struct ClassScopeRewriter {
    qualname: String,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
    locals: HashSet<String>,
    type_params: HashSet<String>,
    has_enclosing_class_cell: bool,
    in_annotate: bool,
}

impl ClassScopeRewriter {
    fn new(
        qualname: String,
        scope: ScopeInfo,
        locals: HashSet<String>,
        type_params: HashSet<String>,
        has_enclosing_class_cell: bool,
    ) -> Self {
        let globals = scope.global_names();
        let nonlocals = scope.nonlocal_names();
        Self {
            qualname,
            globals,
            nonlocals,
            locals,
            type_params,
            has_enclosing_class_cell,
            in_annotate: false,
        }
    }

    fn should_rewrite(&self, name: &str) -> bool {
        !self.globals.contains(name)
            && !self.nonlocals.contains(name)
            && !self.type_params.contains(name)
            && !name.starts_with("_dp_")
            && !matches!(name, "__dp__" | "__classcell__" | "globals" | "locals" | "vars")
            && (name != "__class__" || self.locals.contains("__class__"))
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
                    let name_expr = py_expr!("{name:id}", name = name_str.as_str());
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
    return {name:expr}
"#,
                        tmp = tmp_name.as_str(),
                        alias_stmt = vec![alias_stmt],
                        name = name_expr,
                    );
                    let assign = py_stmt!(
                        "_dp_class_ns.{name:id} = {tmp:id}()",
                        name = name_str.as_str(),
                        tmp = tmp_name.as_str(),
                    );
                    new_body.extend(helper);
                    new_body.extend(assign);
                }
                Stmt::FunctionDef(func_def) => {
                    let func_name = func_def.name.id.clone();

                    let mut stmt = Stmt::FunctionDef(func_def.clone());
                    self.visit_stmt(&mut stmt);
                    new_body.push(stmt);
                    let mut with_assign = py_stmt!(r#"
{func_name:id} = __dp__.update_fn({func_name:id}, {qualname:literal}, "<locals>")                    
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
                if name.id.as_str() == "_dp_annotate" {
                    let was_in_annotate = self.in_annotate;
                    self.in_annotate = true;
                    for stmt in body {
                        self.visit_stmt(stmt);
                    }
                    self.in_annotate = was_in_annotate;
                }
                self.type_params = saved_type_params;
            }
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                assert!(targets.len() == 1, "assign should have a single target");
                let target = targets.first_mut().unwrap();
                if let Expr::Name(ast::ExprName { id, .. }) = target {
                    if id.as_str() == "__classcell__" {
                        return;
                    }
                    self.visit_expr(value);
                    let name = id.as_str();
                    if name == "__class__" && self.nonlocals.contains(name) {
                        *stmt = py_stmt_single(py_stmt!(
                            "_dp_classcell = {value:expr}",
                            value = value.clone()
                        ));
                        return;
                    }
                    let is_class_name = name == "__class__";
                    if is_class_name
                        && (self.globals.contains(name) || self.nonlocals.contains(name))
                    {
                        return;
                    }
                    if is_class_name || self.should_rewrite(name) {
                        *target = py_expr!("_dp_class_ns.{storage_name:id}", storage_name = name,);
                    }
                } else {
                    walk_stmt(self, stmt);
                    return;
                }
            }
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                assert!(targets.len() == 1);
                if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                    let name = id.as_str();
                    if name == "__class__" && self.nonlocals.contains(name) {
                        *stmt = py_stmt_single(py_stmt!(
                            "_dp_classcell = __dp__.empty_classcell()",
                        ));
                        return;
                    }
                    if self.should_rewrite(name) {
                        *stmt = py_stmt_single(py_stmt!(
                            "del _dp_class_ns.{storage_name:id}",
                            storage_name = name,
                        ));
                        return;
                    }
                }
            }

            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                if names.iter().any(|name| name.id.as_str() == "__class__") {
                    for name in names.iter_mut() {
                        if name.id.as_str() == "__class__" {
                            name.id = Name::new("_dp_classcell");
                        }
                    }
                }
            }
            Stmt::AugAssign(_) => {
                panic!("augassign should be rewritten to assign");
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) = expr
        {
            if let Expr::Name(ast::ExprName { id, .. }) = func.as_ref() {
                if arguments.args.is_empty() && arguments.keywords.is_empty() {
                    if id.as_str() == "vars" || id.as_str() == "locals" {
                        *expr = py_expr!("_dp_class_ns._namespace");
                        return;
                    }
                }
            }
        }
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            if matches!(ctx, ExprContext::Load) {
                let name = id.as_str().to_string();
                let name_str = name.as_str();
                if name_str == "__class__" && self.nonlocals.contains(name_str) {
                    *expr = py_expr!("__dp__.class_cell_value(_dp_classcell)");
                    return;
                }
                if name_str == "__class__"
                    && self.has_enclosing_class_cell
                    && !self.locals.contains(name_str)
                    && !self.globals.contains(name_str)
                    && !self.nonlocals.contains(name_str)
                {
                    *expr = py_expr!("__dp__.class_cell_value(_dp_classcell)");
                    return;
                }
                if self.type_params.contains(name_str) {
                    return;
                }
                if !self.should_rewrite(name_str) {
                    return;
                }
                *expr = py_expr!(
                    "__dp__.class_lookup(_dp_class_ns, {name:literal}, lambda: {name:id})",
                    name = name_str,
                );
                return;
            }
        }
        walk_expr(self, expr);
    }
}

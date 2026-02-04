// Minimal AST definitions for desugared language

use ruff_python_ast::{self as ast, AtomicNodeIndex, Expr, ModModule, Stmt};
use ruff_text_size::TextRange;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::ruff_ast_to_string;

pub trait AstInfo: Clone + std::fmt::Debug + PartialEq {}
impl<T: Clone + std::fmt::Debug + PartialEq> AstInfo for T {}

pub trait StmtInfo: AstInfo {}
impl<T: AstInfo> StmtInfo for T {}

pub trait ExprInfo: AstInfo {}
impl<T: AstInfo> ExprInfo for T {}

#[derive(Debug, Clone, PartialEq)]
pub struct Module<S: StmtInfo = (), E: ExprInfo = ()> {
    pub body: Vec<StmtNode<S, E>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtNode<S: StmtInfo = (), E: ExprInfo = ()> {
    FunctionDef(FunctionDef<S, E>),
    While {
        info: S,
        test: ExprNode<E>,
        body: Vec<StmtNode<S, E>>,
        orelse: Vec<StmtNode<S, E>>,
    },
    If {
        info: S,
        test: ExprNode<E>,
        body: Vec<StmtNode<S, E>>,
        orelse: Vec<StmtNode<S, E>>,
    },
    Try {
        info: S,
        body: Vec<StmtNode<S, E>>,
        handler: Option<Vec<StmtNode<S, E>>>,
        orelse: Vec<StmtNode<S, E>>,
        finalbody: Vec<StmtNode<S, E>>,
    },
    Raise {
        info: S,
        exc: Option<ExprNode<E>>,
    },
    Break(S),
    Continue(S),
    Return {
        info: S,
        value: Option<ExprNode<E>>,
    },
    Expr {
        info: S,
        value: ExprNode<E>,
    },
    Assign {
        info: S,
        target: String,
        value: ExprNode<E>,
    },
    Delete {
        info: S,
        target: String,
    },
    Pass(S),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct OuterScopeVars {
    pub globals: Vec<String>,
    pub nonlocals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef<S: StmtInfo = (), E: ExprInfo = ()> {
    pub info: S,
    pub name: String,
    pub display_name: String,
    pub qualname: String,
    pub type_params: Vec<TypeParam<E>>,
    pub params: Vec<Parameter<E>>,
    pub returns: Option<ExprNode<E>>,
    pub body: Vec<StmtNode<S, E>>,
    pub is_async: bool,
    pub scope_vars: OuterScopeVars,
    pub freevars: Vec<String>,
    pub cellvars: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeParam<E: ExprInfo = ()> {
    TypeVar {
        name: String,
        bound: Option<ExprNode<E>>,
        default: Option<ExprNode<E>>,
    },
    TypeVarTuple {
        name: String,
        default: Option<ExprNode<E>>,
    },
    ParamSpec {
        name: String,
        default: Option<ExprNode<E>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parameter<E: ExprInfo = ()> {
    Positional {
        name: String,
        annotation: Option<ExprNode<E>>,
        default: Option<ExprNode<E>>,
    },
    VarArg {
        name: String,
        annotation: Option<ExprNode<E>>,
    },
    KwOnly {
        name: String,
        annotation: Option<ExprNode<E>>,
        default: Option<ExprNode<E>>,
    },
    KwArg {
        name: String,
        annotation: Option<ExprNode<E>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprNode<E: ExprInfo = ()> {
    Name {
        info: E,
        id: String,
    },
    Attribute {
        info: E,
        value: Box<ExprNode<E>>,
        attr: String,
    },
    Number {
        info: E,
        value: Number,
    },
    String {
        info: E,
        value: String,
    },
    Bytes {
        info: E,
        value: Vec<u8>,
    },
    Tuple {
        info: E,
        elts: Vec<ExprNode<E>>,
    },
    Await {
        info: E,
        value: Box<ExprNode<E>>,
    },
    Yield {
        info: E,
        from: bool,
        value: Option<Box<ExprNode<E>>>,
    },
    Call {
        info: E,
        func: Box<ExprNode<E>>,
        args: Vec<Arg<E>>,
    },
    Raw {
        info: E,
        expr: Expr,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arg<E: ExprInfo = ()> {
    Positional(ExprNode<E>),
    Starred(ExprNode<E>),
    Keyword { name: String, value: ExprNode<E> },
    KwStarred(ExprNode<E>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    Int(String),
    Float(String),
}

impl From<ModModule> for Module {
    fn from(module: ModModule) -> Self {
        Module::from_with_function_name_map(module, &HashMap::new())
    }
}

impl Module {
    pub fn from_with_function_name_map(
        module: ModModule,
        function_name_map: &HashMap<String, (String, String)>,
    ) -> Self {
        let mut lost = OuterScopeVars::default();
        let body = StmtNode::from_stmts(module.body, &mut lost, function_name_map);
        if !lost.nonlocals.is_empty() {
            panic!("nonlocal declarations at module scope");
        }
        Module { body }
    }
}

fn collect_bound_names<S: StmtInfo, E: ExprInfo>(
    stmts: &[StmtNode<S, E>],
    names: &mut HashSet<String>,
) {
    for stmt in stmts {
        match stmt {
            StmtNode::Assign { target, .. } | StmtNode::Delete { target, .. } => {
                names.insert(target.clone());
            }
            StmtNode::FunctionDef(func) => {
                names.insert(func.name.clone());
            }
            StmtNode::While { body, orelse, .. } | StmtNode::If { body, orelse, .. } => {
                collect_bound_names(body, names);
                collect_bound_names(orelse, names);
            }
            StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
                ..
            } => {
                collect_bound_names(body, names);
                if let Some(handler) = handler {
                    collect_bound_names(handler, names);
                }
                collect_bound_names(orelse, names);
                collect_bound_names(finalbody, names);
            }
            _ => {}
        }
    }
}

fn collect_local_names<S: StmtInfo, E: ExprInfo>(def: &FunctionDef<S, E>) -> HashSet<String> {
    let mut locals = HashSet::new();
    for param in &def.params {
        match param {
            Parameter::Positional { name, .. }
            | Parameter::VarArg { name, .. }
            | Parameter::KwOnly { name, .. }
            | Parameter::KwArg { name, .. } => {
                locals.insert(name.clone());
            }
        }
    }
    collect_bound_names(&def.body, &mut locals);
    locals
}

fn collect_used_names<S: StmtInfo, E: ExprInfo>(def: &FunctionDef<S, E>) -> HashSet<String> {
    fn visit_expr<E: ExprInfo>(expr: &ExprNode<E>, names: &mut HashSet<String>) {
        match expr {
            ExprNode::Name { id, .. } => {
                names.insert(id.clone());
            }
            ExprNode::Attribute { value, .. } => {
                visit_expr(value, names);
            }
            ExprNode::Tuple { elts, .. } => {
                for elt in elts {
                    visit_expr(elt, names);
                }
            }
            ExprNode::Await { value, .. } => {
                visit_expr(value, names);
            }
            ExprNode::Yield { value, .. } => {
                if let Some(value) = value {
                    visit_expr(value, names);
                }
            }
            ExprNode::Call { func, args, .. } => {
                visit_expr(func, names);
                for arg in args {
                    match arg {
                        Arg::Positional(expr) | Arg::Starred(expr) | Arg::KwStarred(expr) => {
                            visit_expr(expr, names)
                        }
                        Arg::Keyword { value, .. } => visit_expr(value, names),
                    }
                }
            }
            ExprNode::Raw { expr, .. } => {
                collect_load_names_from_raw_expr(expr, names);
            }
            ExprNode::Number { .. } | ExprNode::String { .. } | ExprNode::Bytes { .. } => {}
        }
    }

    fn visit_stmt<S: StmtInfo, E: ExprInfo>(stmt: &StmtNode<S, E>, names: &mut HashSet<String>) {
        match stmt {
            StmtNode::FunctionDef(def) => {
                let inner_used = collect_used_names(def);
                let inner_locals = collect_local_names(def);
                for name in inner_used {
                    if !inner_locals.contains(&name) {
                        names.insert(name);
                    }
                }
            }
            StmtNode::While {
                test, body, orelse, ..
            }
            | StmtNode::If {
                test, body, orelse, ..
            } => {
                visit_expr(test, names);
                for stmt in body {
                    visit_stmt(stmt, names);
                }
                for stmt in orelse {
                    visit_stmt(stmt, names);
                }
            }
            StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
                ..
            } => {
                for stmt in body {
                    visit_stmt(stmt, names);
                }
                if let Some(handler) = handler {
                    for stmt in handler {
                        visit_stmt(stmt, names);
                    }
                }
                for stmt in orelse {
                    visit_stmt(stmt, names);
                }
                for stmt in finalbody {
                    visit_stmt(stmt, names);
                }
            }
            StmtNode::Raise { exc, .. } => {
                if let Some(expr) = exc {
                    visit_expr(expr, names);
                }
            }
            StmtNode::Return { value, .. } => {
                if let Some(expr) = value {
                    visit_expr(expr, names);
                }
            }
            StmtNode::Expr { value, .. } => visit_expr(value, names),
            StmtNode::Assign { value, .. } => visit_expr(value, names),
            StmtNode::Delete { .. }
            | StmtNode::Break(_)
            | StmtNode::Continue(_)
            | StmtNode::Pass(_) => {}
        }
    }

    let mut names = HashSet::new();
    for stmt in &def.body {
        visit_stmt(stmt, &mut names);
    }
    for param in &def.params {
        let annotation = match param {
            Parameter::Positional { annotation, .. }
            | Parameter::VarArg { annotation, .. }
            | Parameter::KwOnly { annotation, .. }
            | Parameter::KwArg { annotation, .. } => annotation,
        };
        if let Some(annotation) = annotation {
            visit_expr(annotation, &mut names);
        }
    }
    if let Some(returns) = &def.returns {
        visit_expr(returns, &mut names);
    }
    names
}

fn collect_child_free_uses<S: StmtInfo, E: ExprInfo>(def: &FunctionDef<S, E>) -> HashSet<String> {
    fn visit_stmt<S: StmtInfo, E: ExprInfo>(stmt: &StmtNode<S, E>, names: &mut HashSet<String>) {
        match stmt {
            StmtNode::FunctionDef(inner) => {
                let inner_used = collect_used_names(inner);
                let inner_locals = collect_local_names(inner);
                for name in inner_used {
                    if !inner_locals.contains(&name) {
                        names.insert(name);
                    }
                }
            }
            StmtNode::While { body, orelse, .. } | StmtNode::If { body, orelse, .. } => {
                for stmt in body {
                    visit_stmt(stmt, names);
                }
                for stmt in orelse {
                    visit_stmt(stmt, names);
                }
            }
            StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
                ..
            } => {
                for stmt in body {
                    visit_stmt(stmt, names);
                }
                if let Some(handler) = handler {
                    for stmt in handler {
                        visit_stmt(stmt, names);
                    }
                }
                for stmt in orelse {
                    visit_stmt(stmt, names);
                }
                for stmt in finalbody {
                    visit_stmt(stmt, names);
                }
            }
            _ => {}
        }
    }

    let mut names = HashSet::new();
    for stmt in &def.body {
        visit_stmt(stmt, &mut names);
    }
    names
}

fn finalize_function_scope_metadata<S: StmtInfo, E: ExprInfo>(def: &mut FunctionDef<S, E>) {
    let locals = collect_local_names(def);
    let mut freevars = collect_used_names(def);
    for local in &locals {
        freevars.remove(local);
    }
    for global_name in &def.scope_vars.globals {
        freevars.remove(global_name);
    }

    let child_free_uses = collect_child_free_uses(def);
    let mut cellvars = HashSet::new();
    for name in child_free_uses {
        if locals.contains(&name) && !def.scope_vars.nonlocals.contains(&name) {
            cellvars.insert(name);
        }
    }

    let mut freevars = freevars.into_iter().collect::<Vec<_>>();
    freevars.sort();
    let mut cellvars = cellvars.into_iter().collect::<Vec<_>>();
    cellvars.sort();
    def.freevars = freevars;
    def.cellvars = cellvars;
}

impl StmtNode {
    fn from_stmts(
        body: ast::StmtBody,
        scope_vars: &mut OuterScopeVars,
        function_name_map: &HashMap<String, (String, String)>,
    ) -> Vec<Self> {
        let mut out = Vec::new();
        for stmt in body.body {
            match *stmt {
                Stmt::BodyStmt(inner) => {
                    out.extend(StmtNode::from_stmts(inner, scope_vars, function_name_map));
                }
                other => {
                    if let Some(node) = StmtNode::from_stmt(other, scope_vars, function_name_map) {
                        out.push(node);
                    }
                }
            }
        }
        out
    }

    fn from_stmt(
        stmt: Stmt,
        scope_vars: &mut OuterScopeVars,
        function_name_map: &HashMap<String, (String, String)>,
    ) -> Option<Self> {
        match stmt {
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                scope_vars
                    .globals
                    .extend(names.into_iter().map(|n| n.id.to_string()));
                None
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                scope_vars
                    .nonlocals
                    .extend(names.into_iter().map(|n| n.id.to_string()));
                None
            }
            Stmt::FunctionDef(ast::StmtFunctionDef {
                name,
                parameters,
                returns,
                body,
                is_async,
                type_params,
                ..
            }) => {
                let mut params = Vec::new();
                let mut type_params_out = Vec::new();
                if let Some(type_params) = type_params {
                    for param in type_params.type_params {
                        match param {
                            ast::TypeParam::TypeVar(type_var) => {
                                type_params_out.push(TypeParam::TypeVar {
                                    name: type_var.name.to_string(),
                                    bound: type_var.bound.map(|expr| ExprNode::from(*expr)),
                                    default: type_var.default.map(|expr| ExprNode::from(*expr)),
                                });
                            }
                            ast::TypeParam::TypeVarTuple(type_var_tuple) => {
                                type_params_out.push(TypeParam::TypeVarTuple {
                                    name: type_var_tuple.name.to_string(),
                                    default: type_var_tuple
                                        .default
                                        .map(|expr| ExprNode::from(*expr)),
                                });
                            }
                            ast::TypeParam::ParamSpec(param_spec) => {
                                type_params_out.push(TypeParam::ParamSpec {
                                    name: param_spec.name.to_string(),
                                    default: param_spec.default.map(|expr| ExprNode::from(*expr)),
                                });
                            }
                        }
                    }
                }
                let ast::Parameters {
                    posonlyargs,
                    args,
                    vararg,
                    kwonlyargs,
                    kwarg,
                    ..
                } = *parameters;
                for p in posonlyargs {
                    let ast::ParameterWithDefault {
                        parameter, default, ..
                    } = p;
                    let ast::Parameter {
                        name, annotation, ..
                    } = parameter;
                    params.push(Parameter::Positional {
                        name: name.to_string(),
                        annotation: annotation.map(|expr| ExprNode::from(*expr)),
                        default: default.map(|d| ExprNode::from(*d)),
                    });
                }
                for p in args {
                    let ast::ParameterWithDefault {
                        parameter, default, ..
                    } = p;
                    let ast::Parameter {
                        name, annotation, ..
                    } = parameter;
                    params.push(Parameter::Positional {
                        name: name.to_string(),
                        annotation: annotation.map(|expr| ExprNode::from(*expr)),
                        default: default.map(|d| ExprNode::from(*d)),
                    });
                }
                if let Some(p) = vararg {
                    let ast::Parameter {
                        name, annotation, ..
                    } = *p;
                    params.push(Parameter::VarArg {
                        name: name.to_string(),
                        annotation: annotation.map(|expr| ExprNode::from(*expr)),
                    });
                }
                for p in kwonlyargs {
                    let ast::ParameterWithDefault {
                        parameter, default, ..
                    } = p;
                    let ast::Parameter {
                        name, annotation, ..
                    } = parameter;
                    params.push(Parameter::KwOnly {
                        name: name.to_string(),
                        annotation: annotation.map(|expr| ExprNode::from(*expr)),
                        default: default.map(|d| ExprNode::from(*d)),
                    });
                }
                if let Some(p) = kwarg {
                    let ast::Parameter {
                        name, annotation, ..
                    } = *p;
                    params.push(Parameter::KwArg {
                        name: name.to_string(),
                        annotation: annotation.map(|expr| ExprNode::from(*expr)),
                    });
                }
                let mut fn_scope_vars = OuterScopeVars::default();
                let body = StmtNode::from_stmts(body, &mut fn_scope_vars, function_name_map);
                let rewritten_name = name.to_string();
                let (display_name, qualname) = function_name_map
                    .get(&rewritten_name)
                    .cloned()
                    .unwrap_or_else(|| (rewritten_name.clone(), rewritten_name.clone()));
                let mut def = FunctionDef {
                    info: (),
                    name: rewritten_name,
                    display_name,
                    qualname,
                    type_params: type_params_out,
                    params,
                    returns: returns.map(|expr| ExprNode::from(*expr)),
                    body,
                    is_async,
                    scope_vars: fn_scope_vars,
                    freevars: Vec::new(),
                    cellvars: Vec::new(),
                };
                finalize_function_scope_metadata(&mut def);
                Some(StmtNode::FunctionDef(def))
            }
            Stmt::ImportFrom(ast::StmtImportFrom { .. }) => {
                panic!("ImportFrom should be rewritten before min_ast conversion")
            }
            Stmt::While(ast::StmtWhile {
                test, body, orelse, ..
            }) => Some(StmtNode::While {
                info: (),
                test: ExprNode::from(*test),
                body: StmtNode::from_stmts(body, scope_vars, function_name_map),
                orelse: StmtNode::from_stmts(orelse, scope_vars, function_name_map),
            }),
            Stmt::If(ast::StmtIf {
                test,
                body,
                elif_else_clauses,
                ..
            }) => {
                let mut orelse = Vec::new();
                let mut seen_else = false;
                for clause in elif_else_clauses {
                    if clause.test.is_some() {
                        panic!("elif clauses are not supported in min_ast");
                    }
                    if seen_else {
                        panic!("multiple else clauses not supported in min_ast");
                    }
                    seen_else = true;
                    orelse = StmtNode::from_stmts(clause.body, scope_vars, function_name_map);
                }
                Some(StmtNode::If {
                    info: (),
                    test: ExprNode::from(*test),
                    body: StmtNode::from_stmts(body, scope_vars, function_name_map),
                    orelse,
                })
            }
            Stmt::Try(ast::StmtTry {
                body,
                handlers,
                orelse,
                finalbody,
                is_star,
                ..
            }) => {
                let handler = if handlers.is_empty() {
                    None
                } else if is_star {
                    let mut handler_body = Vec::new();
                    for handler in handlers {
                        match handler {
                            ast::ExceptHandler::ExceptHandler(
                                ast::ExceptHandlerExceptHandler { body: h_body, .. },
                            ) => handler_body.extend(StmtNode::from_stmts(
                                h_body,
                                scope_vars,
                                function_name_map,
                            )),
                        }
                    }
                    Some(handler_body)
                } else if handlers.len() == 1 {
                    match handlers.into_iter().next().unwrap() {
                        ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                            type_,
                            name,
                            body: h_body,
                            ..
                        }) => {
                            if type_.is_some() || name.is_some() {
                                panic!("only bare except handlers supported");
                            }
                            Some(StmtNode::from_stmts(h_body, scope_vars, function_name_map))
                        }
                    }
                } else {
                    panic!("multiple except handlers not supported");
                };
                Some(StmtNode::Try {
                    info: (),
                    body: StmtNode::from_stmts(body, scope_vars, function_name_map),
                    handler,
                    orelse: StmtNode::from_stmts(orelse, scope_vars, function_name_map),
                    finalbody: StmtNode::from_stmts(finalbody, scope_vars, function_name_map),
                })
            }
            Stmt::Break(_) => Some(StmtNode::Break(())),
            Stmt::Continue(_) => Some(StmtNode::Continue(())),
            Stmt::Return(ast::StmtReturn { value, .. }) => Some(StmtNode::Return {
                info: (),
                value: value.map(|v| ExprNode::from(*v)),
            }),
            Stmt::Raise(ast::StmtRaise { exc, cause, .. }) => {
                if cause.is_some() {
                    panic!("raise with cause not supported");
                }
                Some(StmtNode::Raise {
                    info: (),
                    exc: exc.map(|e| ExprNode::from(*e)),
                })
            }
            Stmt::Expr(ast::StmtExpr { value, .. }) => Some(StmtNode::Expr {
                info: (),
                value: ExprNode::from(*value),
            }),
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                let target_name = if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        id.to_string()
                    } else {
                        let s = ast::StmtAssign {
                            targets,
                            value,
                            node_index: AtomicNodeIndex::default(),
                            range: TextRange::default(),
                        };
                        panic!(
                            "unsupported assignment target {}",
                            ruff_ast_to_string(Stmt::Assign(s))
                        );
                    }
                } else {
                    panic!("unsupported assignment targets")
                };
                Some(StmtNode::Assign {
                    info: (),
                    target: target_name,
                    value: ExprNode::from(*value),
                })
            }
            Stmt::TypeAlias(ast::StmtTypeAlias { name, value, .. }) => match *name {
                Expr::Name(ast::ExprName { id, .. }) => Some(StmtNode::Assign {
                    info: (),
                    target: id.to_string(),
                    value: ExprNode::from(*value),
                }),
                other => {
                    let _ = ExprNode::from(other);
                    Some(StmtNode::Expr {
                        info: (),
                        value: ExprNode::from(*value),
                    })
                }
            },
            Stmt::AnnAssign(ast::StmtAnnAssign { annotation, .. }) => Some(StmtNode::Expr {
                info: (),
                value: ExprNode::from(*annotation),
            }),
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                let target_name = if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        id.to_string()
                    } else {
                        panic!("unsupported delete target")
                    }
                } else {
                    panic!("unsupported delete targets")
                };
                Some(StmtNode::Delete {
                    info: (),
                    target: target_name,
                })
            }
            Stmt::Pass(_) => Some(StmtNode::Pass(())),
            other => panic!("unsupported statement: {:?}", other),
        }
    }
}

impl From<Expr> for ExprNode {
    fn from(expr: Expr) -> Self {
        match expr {
            Expr::Name(ast::ExprName { id, .. }) => ExprNode::Name {
                info: (),
                id: id.to_string(),
            },
            Expr::NumberLiteral(ast::ExprNumberLiteral { value, .. }) => {
                let num = match value {
                    ast::Number::Int(i) => Number::Int(i.to_string()),
                    ast::Number::Float(f) => Number::Float(f.to_string()),
                    ast::Number::Complex { .. } => {
                        panic!("complex numbers should have been transformed away")
                    }
                };
                ExprNode::Number {
                    info: (),
                    value: num,
                }
            }
            Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => ExprNode::String {
                info: (),
                value: value.to_string(),
            },
            Expr::BytesLiteral(ast::ExprBytesLiteral { value, .. }) => {
                let bytes: Cow<[u8]> = (&value).into();
                ExprNode::Bytes {
                    info: (),
                    value: bytes.into_owned(),
                }
            }
            Expr::BooleanLiteral(ast::ExprBooleanLiteral { value, .. }) => ExprNode::Name {
                info: (),
                id: if value { "True" } else { "False" }.to_string(),
            },
            Expr::NoneLiteral(_) => ExprNode::Name {
                info: (),
                id: "None".to_string(),
            },
            Expr::EllipsisLiteral(_) => ExprNode::Name {
                info: (),
                id: "Ellipsis".to_string(),
            },
            Expr::Tuple(ast::ExprTuple { elts, .. }) => ExprNode::Tuple {
                info: (),
                elts: elts.into_iter().map(ExprNode::from).collect(),
            },
            Expr::Await(ast::ExprAwait { value, .. }) => ExprNode::Await {
                info: (),
                value: Box::new(ExprNode::from(*value)),
            },
            Expr::Yield(ast::ExprYield { value, .. }) => ExprNode::Yield {
                info: (),
                from: false,
                value: value.map(|v| Box::new(ExprNode::from(*v))),
            },
            Expr::YieldFrom(ast::ExprYieldFrom { value, .. }) => ExprNode::Yield {
                info: (),
                from: true,
                value: Some(Box::new(ExprNode::from(*value))),
            },
            Expr::Call(ast::ExprCall {
                func, arguments, ..
            }) => {
                let mut args_vec = Vec::new();
                for arg in arguments.args.into_vec() {
                    match arg {
                        Expr::Starred(ast::ExprStarred { value, .. }) => {
                            args_vec.push(Arg::Starred(ExprNode::from(*value)))
                        }
                        other => args_vec.push(Arg::Positional(ExprNode::from(other))),
                    }
                }
                for kw in arguments.keywords.into_vec() {
                    if let Some(arg) = kw.arg {
                        args_vec.push(Arg::Keyword {
                            name: arg.to_string(),
                            value: ExprNode::from(kw.value),
                        });
                    } else {
                        args_vec.push(Arg::KwStarred(ExprNode::from(kw.value)));
                    }
                }
                ExprNode::Call {
                    info: (),
                    func: Box::new(ExprNode::from(*func)),
                    args: args_vec,
                }
            }
            Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => ExprNode::Attribute {
                info: (),
                value: Box::new(ExprNode::from(*value)),
                attr: attr.id.to_string(),
            },
            other => ExprNode::Raw {
                info: (),
                expr: other,
            },
        }
    }
}

fn collect_load_names_from_raw_expr(expr: &Expr, names: &mut HashSet<String>) {
    use crate::transformer::{Transformer, walk_expr};
    use ruff_python_ast::ExprContext;

    struct LoadNameCollector<'a> {
        names: &'a mut HashSet<String>,
    }

    impl Transformer for LoadNameCollector<'_> {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
                if matches!(ctx, ExprContext::Load) {
                    self.names.insert(id.to_string());
                }
            }
            walk_expr(self, expr);
        }
    }

    let mut cloned = expr.clone();
    let mut collector = LoadNameCollector { names };
    collector.visit_expr(&mut cloned);
}

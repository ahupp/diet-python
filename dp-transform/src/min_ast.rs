// Minimal AST definitions for desugared language

use ruff_python_ast::{self as ast, AtomicNodeIndex, Expr, ModModule, Stmt};
use ruff_text_size::TextRange;
use std::borrow::Cow;

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
    ImportFrom {
        info: S,
        module: Option<String>,
        names: Vec<String>,
        level: usize,
    },
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
    pub params: Vec<Parameter<E>>,
    pub returns: Option<ExprNode<E>>,
    pub body: Vec<StmtNode<S, E>>,
    pub is_async: bool,
    pub scope_vars: OuterScopeVars,
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
        value: Option<Box<ExprNode<E>>>,
    },
    Call {
        info: E,
        func: Box<ExprNode<E>>,
        args: Vec<Arg<E>>,
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
        let mut lost = OuterScopeVars::default();
        let body = StmtNode::from_stmts(module.body, &mut lost);
        if !lost.nonlocals.is_empty() {
            panic!("nonlocal declarations at module scope");
        }
        Module { body }
    }
}

impl StmtNode {
    fn from_stmts(stmts: Vec<Stmt>, scope_vars: &mut OuterScopeVars) -> Vec<Self> {
        stmts
            .into_iter()
            .filter_map(|stmt| StmtNode::from_stmt(stmt, scope_vars))
            .collect()
    }

    fn from_stmt(stmt: Stmt, scope_vars: &mut OuterScopeVars) -> Option<Self> {
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
                ..
            }) => {
                let mut params = Vec::new();
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
                let body = StmtNode::from_stmts(body, &mut fn_scope_vars);
                Some(StmtNode::FunctionDef(FunctionDef {
                    info: (),
                    name: name.to_string(),
                    params,
                    returns: returns.map(|expr| ExprNode::from(*expr)),
                    body,
                    is_async,
                    scope_vars: fn_scope_vars,
                }))
            }
            Stmt::ImportFrom(ast::StmtImportFrom {
                module,
                names,
                level,
                ..
            }) => {
                let import_names = names
                    .into_iter()
                    .map(|alias| {
                        if alias.asname.is_some() {
                            panic!("unsupported import alias");
                        }
                        alias.name.id.to_string()
                    })
                    .collect::<Vec<_>>();
                Some(StmtNode::ImportFrom {
                    info: (),
                    module: module.map(|m| m.id.to_string()),
                    names: import_names,
                    level: level as usize,
                })
            }
            Stmt::While(ast::StmtWhile {
                test, body, orelse, ..
            }) => Some(StmtNode::While {
                info: (),
                test: ExprNode::from(*test),
                body: StmtNode::from_stmts(body, scope_vars),
                orelse: StmtNode::from_stmts(orelse, scope_vars),
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
                    orelse = StmtNode::from_stmts(clause.body, scope_vars);
                }
                Some(StmtNode::If {
                    info: (),
                    test: ExprNode::from(*test),
                    body: StmtNode::from_stmts(body, scope_vars),
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
                            ) => handler_body.extend(h_body),
                        }
                    }
                    Some(StmtNode::from_stmts(handler_body, scope_vars))
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
                            Some(StmtNode::from_stmts(h_body, scope_vars))
                        }
                    }
                } else {
                    panic!("multiple except handlers not supported");
                };
                Some(StmtNode::Try {
                    info: (),
                    body: StmtNode::from_stmts(body, scope_vars),
                    handler,
                    orelse: StmtNode::from_stmts(orelse, scope_vars),
                    finalbody: StmtNode::from_stmts(finalbody, scope_vars),
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
            Stmt::Assign(ast::StmtAssign {targets, value, .. }) => {
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
                        panic!("unsupported assignment target {}", ruff_ast_to_string(&[Stmt::Assign(s)]));
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
            Stmt::TypeAlias(ast::StmtTypeAlias { name, value, .. }) => {
                match *name {
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
                }
            }
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
                value: value.map(|v| Box::new(ExprNode::from(*v))),
            },
            Expr::YieldFrom(ast::ExprYieldFrom { value, .. }) => ExprNode::Yield {
                info: (),
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
            other => panic!("unsupported expr: {:?}", other),
        }
    }
}

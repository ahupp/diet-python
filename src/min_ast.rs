// Minimal AST definitions for desugared language

use std::borrow::Cow;

use ruff_python_ast::{self as ast, Expr, ModModule, Stmt};

pub trait AstInfo: Clone + std::fmt::Debug + PartialEq {}
impl<T: Clone + std::fmt::Debug + PartialEq> AstInfo for T {}

pub trait StmtInfo {
    type FunctionDefStmtInfo: AstInfo;
    type WhileStmtInfo: AstInfo;
    type IfStmtInfo: AstInfo;
    type TryStmtInfo: AstInfo;
    type RaiseStmtInfo: AstInfo;
    type BreakStmtInfo: AstInfo;
    type ContinueStmtInfo: AstInfo;
    type ReturnStmtInfo: AstInfo;
    type ExprStmtInfo: AstInfo;
    type AssignStmtInfo: AstInfo;
    type DeleteStmtInfo: AstInfo;
    type PassStmtInfo: AstInfo;
}

pub trait ExprInfo {
    type NameExprInfo: AstInfo;
    type NumberExprInfo: AstInfo;
    type StringExprInfo: AstInfo;
    type BytesExprInfo: AstInfo;
    type TupleExprInfo: AstInfo;
    type AwaitExprInfo: AstInfo;
    type YieldExprInfo: AstInfo;
    type CallExprInfo: AstInfo;
}

impl StmtInfo for () {
    type FunctionDefStmtInfo = ();
    type WhileStmtInfo = ();
    type IfStmtInfo = ();
    type TryStmtInfo = ();
    type RaiseStmtInfo = ();
    type BreakStmtInfo = ();
    type ContinueStmtInfo = ();
    type ReturnStmtInfo = ();
    type ExprStmtInfo = ();
    type AssignStmtInfo = ();
    type DeleteStmtInfo = ();
    type PassStmtInfo = ();
}

impl ExprInfo for () {
    type NameExprInfo = ();
    type NumberExprInfo = ();
    type StringExprInfo = ();
    type BytesExprInfo = ();
    type TupleExprInfo = ();
    type AwaitExprInfo = ();
    type YieldExprInfo = ();
    type CallExprInfo = ();
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module<S: StmtInfo = (), E: ExprInfo = ()> {
    pub body: Vec<StmtNode<S, E>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtNode<S: StmtInfo = (), E: ExprInfo = ()> {
    FunctionDef(FunctionDef<S, E>),
    While {
        info: S::WhileStmtInfo,
        test: ExprNode<E>,
        body: Vec<StmtNode<S, E>>,
        orelse: Vec<StmtNode<S, E>>,
    },
    If {
        info: S::IfStmtInfo,
        test: ExprNode<E>,
        body: Vec<StmtNode<S, E>>,
        orelse: Vec<StmtNode<S, E>>,
    },
    Try {
        info: S::TryStmtInfo,
        body: Vec<StmtNode<S, E>>,
        handler: Option<Vec<StmtNode<S, E>>>,
        orelse: Vec<StmtNode<S, E>>,
        finalbody: Vec<StmtNode<S, E>>,
    },
    Raise {
        info: S::RaiseStmtInfo,
        exc: Option<ExprNode<E>>,
    },
    Break(S::BreakStmtInfo),
    Continue(S::ContinueStmtInfo),
    Return {
        info: S::ReturnStmtInfo,
        value: Option<ExprNode<E>>,
    },
    Expr {
        info: S::ExprStmtInfo,
        value: ExprNode<E>,
    },
    Assign {
        info: S::AssignStmtInfo,
        target: String,
        value: ExprNode<E>,
    },
    Delete {
        info: S::DeleteStmtInfo,
        target: String,
    },
    Pass(S::PassStmtInfo),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct OuterScopeVars {
    pub globals: Vec<String>,
    pub nonlocals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef<S: StmtInfo = (), E: ExprInfo = ()> {
    pub info: S::FunctionDefStmtInfo,
    pub name: String,
    pub params: Vec<Parameter<E>>,
    pub body: Vec<StmtNode<S, E>>,
    pub is_async: bool,
    pub scope_vars: OuterScopeVars,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parameter<E: ExprInfo = ()> {
    Positional {
        name: String,
        default: Option<ExprNode<E>>,
    },
    VarArg {
        name: String,
    },
    KwOnly {
        name: String,
        default: Option<ExprNode<E>>,
    },
    KwArg {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprNode<E: ExprInfo = ()> {
    Name {
        info: E::NameExprInfo,
        id: String,
    },
    Number {
        info: E::NumberExprInfo,
        value: Number,
    },
    String {
        info: E::StringExprInfo,
        value: String,
    },
    Bytes {
        info: E::BytesExprInfo,
        value: Vec<u8>,
    },
    Tuple {
        info: E::TupleExprInfo,
        elts: Vec<ExprNode<E>>,
    },
    Await {
        info: E::AwaitExprInfo,
        value: Box<ExprNode<E>>,
    },
    Yield {
        info: E::YieldExprInfo,
        value: Option<Box<ExprNode<E>>>,
    },
    Call {
        info: E::CallExprInfo,
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
                    params.push(Parameter::Positional {
                        name: p.parameter.name.to_string(),
                        default: p.default.map(|d| ExprNode::from(*d)),
                    });
                }
                for p in args {
                    params.push(Parameter::Positional {
                        name: p.parameter.name.to_string(),
                        default: p.default.map(|d| ExprNode::from(*d)),
                    });
                }
                if let Some(p) = vararg {
                    params.push(Parameter::VarArg {
                        name: p.name.to_string(),
                    });
                }
                for p in kwonlyargs {
                    params.push(Parameter::KwOnly {
                        name: p.parameter.name.to_string(),
                        default: p.default.map(|d| ExprNode::from(*d)),
                    });
                }
                if let Some(p) = kwarg {
                    params.push(Parameter::KwArg {
                        name: p.name.to_string(),
                    });
                }
                let mut fn_scope_vars = OuterScopeVars::default();
                let body = StmtNode::from_stmts(body, &mut fn_scope_vars);
                Some(StmtNode::FunctionDef(FunctionDef {
                    info: (),
                    name: name.to_string(),
                    params,
                    body,
                    is_async,
                    scope_vars: fn_scope_vars,
                }))
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
                let orelse = elif_else_clauses
                    .into_iter()
                    .find(|clause| clause.test.is_none())
                    .map(|clause| StmtNode::from_stmts(clause.body, scope_vars))
                    .unwrap_or_default();
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
                ..
            }) => {
                let handler = if handlers.is_empty() {
                    None
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
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                let target_name = if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        id.to_string()
                    } else {
                        panic!("unsupported assignment target")
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
            other => panic!("unsupported expr: {:?}", other),
        }
    }
}

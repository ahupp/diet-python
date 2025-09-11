// Minimal AST definitions for desugared language

use std::borrow::Cow;

use ruff_python_ast::{self as ast, Expr, ModModule, Stmt};

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub body: Vec<StmtNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtNode {
    FunctionDef(FunctionDef),
    While {
        test: ExprNode,
        body: Vec<StmtNode>,
        orelse: Vec<StmtNode>,
    },
    If {
        test: ExprNode,
        body: Vec<StmtNode>,
        orelse: Vec<StmtNode>,
    },
    Try {
        body: Vec<StmtNode>,
        handlers: Vec<ExceptHandler>,
        orelse: Vec<StmtNode>,
        finalbody: Vec<StmtNode>,
    },
    Break,
    Continue,
    Return {
        value: Option<ExprNode>,
    },
    Expr(ExprNode),
    Assign {
        target: String,
        value: ExprNode,
    },
    Delete {
        target: String,
    },
    Global(String),
    Nonlocal(String),
    Pass,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: String,
    pub params: Vec<Parameter>,
    pub body: Vec<StmtNode>,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parameter {
    Positional {
        name: String,
        default: Option<ExprNode>,
    },
    VarArg {
        name: String,
    },
    KwOnly {
        name: String,
        default: Option<ExprNode>,
    },
    KwArg {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprNode {
    Name(String),
    Number(Number),
    String(String),
    Bytes(Vec<u8>),
    None,
    Tuple(Vec<ExprNode>),
    Await(Box<ExprNode>),
    Yield(Option<Box<ExprNode>>),
    Call { func: Box<ExprNode>, args: Vec<Arg> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Positional(ExprNode),
    Starred(ExprNode),
    Keyword { name: String, value: ExprNode },
    KwStarred(ExprNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExceptHandler {
    pub type_: Option<ExprNode>,
    pub name: Option<String>,
    pub body: Vec<StmtNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    Int(String),
    Float(String),
}

impl From<ModModule> for Module {
    fn from(module: ModModule) -> Self {
        Module {
            body: StmtNode::from_stmts(module.body),
        }
    }
}

impl StmtNode {
    fn from_stmts(stmts: Vec<Stmt>) -> Vec<Self> {
        stmts.into_iter().map(StmtNode::from_stmt).collect()
    }

    fn from_stmt(stmt: Stmt) -> Self {
        match stmt {
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
                StmtNode::FunctionDef(FunctionDef {
                    name: name.to_string(),
                    params,
                    body: StmtNode::from_stmts(body),
                    is_async,
                })
            }
            Stmt::While(ast::StmtWhile {
                test, body, orelse, ..
            }) => StmtNode::While {
                test: ExprNode::from(*test),
                body: StmtNode::from_stmts(body),
                orelse: StmtNode::from_stmts(orelse),
            },
            Stmt::If(ast::StmtIf {
                test,
                body,
                elif_else_clauses,
                ..
            }) => {
                let orelse = elif_else_clauses
                    .into_iter()
                    .find(|clause| clause.test.is_none())
                    .map(|clause| StmtNode::from_stmts(clause.body))
                    .unwrap_or_default();
                StmtNode::If {
                    test: ExprNode::from(*test),
                    body: StmtNode::from_stmts(body),
                    orelse,
                }
            }
            Stmt::Try(ast::StmtTry {
                body,
                handlers,
                orelse,
                finalbody,
                ..
            }) => StmtNode::Try {
                body: StmtNode::from_stmts(body),
                handlers: handlers
                    .into_iter()
                    .map(|handler| match handler {
                        ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                            type_,
                            name,
                            body,
                            ..
                        }) => ExceptHandler {
                            type_: type_.map(|t| ExprNode::from(*t)),
                            name: name.map(|n| n.to_string()),
                            body: StmtNode::from_stmts(body),
                        },
                    })
                    .collect(),
                orelse: StmtNode::from_stmts(orelse),
                finalbody: StmtNode::from_stmts(finalbody),
            },
            Stmt::Break(_) => StmtNode::Break,
            Stmt::Continue(_) => StmtNode::Continue,
            Stmt::Return(ast::StmtReturn { value, .. }) => StmtNode::Return {
                value: value.map(|v| ExprNode::from(*v)),
            },
            Stmt::Expr(ast::StmtExpr { value, .. }) => StmtNode::Expr(ExprNode::from(*value)),
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
                StmtNode::Assign {
                    target: target_name,
                    value: ExprNode::from(*value),
                }
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
                StmtNode::Delete {
                    target: target_name,
                }
            }
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                if names.len() == 1 {
                    let name = names.into_iter().next().unwrap().id;
                    StmtNode::Global(name.into())
                } else {
                    panic!("global statement should have been rewritten to single target")
                }
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                if names.len() == 1 {
                    let name = names.into_iter().next().unwrap().id;
                    StmtNode::Nonlocal(name.into())
                } else {
                    panic!("nonlocal statement should have been rewritten to single target")
                }
            }
            Stmt::Pass(_) => StmtNode::Pass,
            other => panic!("unsupported statement: {:?}", other),
        }
    }
}

impl From<Expr> for ExprNode {
    fn from(expr: Expr) -> Self {
        match expr {
            Expr::Name(ast::ExprName { id, .. }) => ExprNode::Name(id.to_string()),
            Expr::NumberLiteral(ast::ExprNumberLiteral { value, .. }) => {
                let num = match value {
                    ast::Number::Int(i) => Number::Int(i.to_string()),
                    ast::Number::Float(f) => Number::Float(f.to_string()),
                    ast::Number::Complex { .. } => {
                        panic!("complex numbers should have been transformed away")
                    }
                };
                ExprNode::Number(num)
            }
            Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) => {
                ExprNode::String(value.to_string())
            }
            Expr::BytesLiteral(ast::ExprBytesLiteral { value, .. }) => {
                let bytes: Cow<[u8]> = (&value).into();
                ExprNode::Bytes(bytes.into_owned())
            }
            Expr::BooleanLiteral(ast::ExprBooleanLiteral { value, .. }) => {
                ExprNode::Name(if value { "True" } else { "False" }.to_string())
            }
            Expr::NoneLiteral(_) => ExprNode::None,
            Expr::Tuple(ast::ExprTuple { elts, .. }) => {
                ExprNode::Tuple(elts.into_iter().map(ExprNode::from).collect())
            }
            Expr::Await(ast::ExprAwait { value, .. }) => {
                ExprNode::Await(Box::new(ExprNode::from(*value)))
            }
            Expr::Yield(ast::ExprYield { value, .. }) => {
                ExprNode::Yield(value.map(|v| Box::new(ExprNode::from(*v))))
            }
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
                    func: Box::new(ExprNode::from(*func)),
                    args: args_vec,
                }
            }
            other => panic!("unsupported expr: {:?}", other),
        }
    }
}

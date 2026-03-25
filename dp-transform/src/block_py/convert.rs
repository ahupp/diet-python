use super::*;
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};
use crate::py_expr;
use ruff_python_ast::str::Quote;
use ruff_python_ast::{
    self as ast, BytesLiteral, BytesLiteralFlags, StringLiteral, StringLiteralFlags,
    StringLiteralValue,
};

pub trait BlockPyModuleMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    BlockPyStmt<POut::Expr>: Into<POut::Stmt>,
{
    fn map_module(&self, module: BlockPyModule<PIn>) -> BlockPyModule<POut> {
        BlockPyModule {
            callable_defs: module
                .callable_defs
                .into_iter()
                .map(|function| self.map_fn(function))
                .collect(),
        }
    }

    fn map_fn(&self, func: BlockPyFunction<PIn>) -> BlockPyFunction<POut> {
        BlockPyFunction {
            function_id: func.function_id,
            name_gen: func.name_gen,
            names: func.names,
            kind: func.kind,
            params: func.params,
            blocks: func
                .blocks
                .into_iter()
                .map(|block| self.map_block(block))
                .collect(),
            doc: func.doc,
            closure_layout: func.closure_layout,
            facts: func.facts,
            semantic: func.semantic,
        }
    }

    fn map_block(&self, block: PassBlock<PIn>) -> PassBlock<POut> {
        CfgBlock {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| self.map_stmt(stmt.into_stmt()).into())
                .collect(),
            term: self.map_term(block.term),
            params: block.params,
            exc_edge: block.exc_edge,
        }
    }

    fn map_fragment(
        &self,
        fragment: BlockPyCfgFragment<BlockPyStmt<PassExpr<PIn>>, BlockPyTerm<PassExpr<PIn>>>,
    ) -> BlockPyCfgFragment<BlockPyStmt<PassExpr<POut>>, BlockPyTerm<PassExpr<POut>>> {
        BlockPyCfgFragment {
            body: fragment
                .body
                .into_iter()
                .map(|stmt| self.map_stmt(stmt))
                .collect(),
            term: fragment.term.map(|term| self.map_term(term)),
        }
    }

    fn map_stmt(&self, stmt: BlockPyStmt<PassExpr<PIn>>) -> BlockPyStmt<PassExpr<POut>> {
        match stmt {
            BlockPyStmt::Assign(assign) => self.map_assign(assign),
            BlockPyStmt::Expr(expr) => BlockPyStmt::Expr(self.map_expr(expr)),
            BlockPyStmt::Delete(delete) => BlockPyStmt::Delete(delete),
            BlockPyStmt::If(if_stmt) => BlockPyStmt::If(BlockPyIf {
                test: self.map_expr(if_stmt.test),
                body: self.map_fragment(if_stmt.body),
                orelse: self.map_fragment(if_stmt.orelse),
            }),
        }
    }

    fn map_assign(&self, assign: BlockPyAssign<PassExpr<PIn>>) -> BlockPyStmt<PassExpr<POut>> {
        BlockPyStmt::Assign(BlockPyAssign {
            target: assign.target,
            value: self.map_expr(assign.value),
        })
    }

    fn map_term(&self, term: BlockPyTerm<PassExpr<PIn>>) -> BlockPyTerm<PassExpr<POut>> {
        match term {
            BlockPyTerm::Jump(edge) => BlockPyTerm::Jump(BlockPyEdge {
                target: edge.target,
                args: edge.args,
            }),
            BlockPyTerm::IfTerm(if_term) => BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: self.map_expr(if_term.test),
                then_label: if_term.then_label,
                else_label: if_term.else_label,
            }),
            BlockPyTerm::BranchTable(branch) => BlockPyTerm::BranchTable(BlockPyBranchTable {
                index: self.map_expr(branch.index),
                targets: branch.targets,
                default_label: branch.default_label,
            }),
            BlockPyTerm::Raise(raise_stmt) => BlockPyTerm::Raise(BlockPyRaise {
                exc: raise_stmt.exc.map(|exc| self.map_expr(exc)),
            }),
            BlockPyTerm::Return(value) => BlockPyTerm::Return(self.map_expr(value)),
        }
    }

    fn map_expr(&self, expr: PassExpr<PIn>) -> PassExpr<POut>;
}

pub trait BlockPyModuleTryMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    BlockPyStmt<POut::Expr>: Into<POut::Stmt>,
{
    type Error;

    fn try_map_module(
        &self,
        module: BlockPyModule<PIn>,
    ) -> Result<BlockPyModule<POut>, Self::Error> {
        Ok(BlockPyModule {
            callable_defs: module
                .callable_defs
                .into_iter()
                .map(|function| self.try_map_fn(function))
                .collect::<Result<_, _>>()?,
        })
    }

    fn try_map_fn(&self, func: BlockPyFunction<PIn>) -> Result<BlockPyFunction<POut>, Self::Error> {
        Ok(BlockPyFunction {
            function_id: func.function_id,
            name_gen: func.name_gen,
            names: func.names,
            kind: func.kind,
            params: func.params,
            blocks: func
                .blocks
                .into_iter()
                .map(|block| self.try_map_block(block))
                .collect::<Result<_, _>>()?,
            doc: func.doc,
            closure_layout: func.closure_layout,
            facts: func.facts,
            semantic: func.semantic,
        })
    }

    fn try_map_block(&self, block: PassBlock<PIn>) -> Result<PassBlock<POut>, Self::Error> {
        Ok(CfgBlock {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| self.try_map_stmt(stmt.into_stmt()).map(Into::into))
                .collect::<Result<_, _>>()?,
            term: self.try_map_term(block.term)?,
            params: block.params,
            exc_edge: block.exc_edge,
        })
    }

    fn try_map_fragment(
        &self,
        fragment: BlockPyCfgFragment<BlockPyStmt<PassExpr<PIn>>, BlockPyTerm<PassExpr<PIn>>>,
    ) -> Result<
        BlockPyCfgFragment<BlockPyStmt<PassExpr<POut>>, BlockPyTerm<PassExpr<POut>>>,
        Self::Error,
    > {
        Ok(BlockPyCfgFragment {
            body: fragment
                .body
                .into_iter()
                .map(|stmt| self.try_map_stmt(stmt))
                .collect::<Result<_, _>>()?,
            term: fragment
                .term
                .map(|term| self.try_map_term(term))
                .transpose()?,
        })
    }

    fn try_map_stmt(
        &self,
        stmt: BlockPyStmt<PassExpr<PIn>>,
    ) -> Result<BlockPyStmt<PassExpr<POut>>, Self::Error> {
        match stmt {
            BlockPyStmt::Assign(assign) => self.try_map_assign(assign),
            BlockPyStmt::Expr(expr) => Ok(BlockPyStmt::Expr(self.try_map_expr(expr)?)),
            BlockPyStmt::Delete(delete) => Ok(BlockPyStmt::Delete(delete)),
            BlockPyStmt::If(if_stmt) => Ok(BlockPyStmt::If(BlockPyIf {
                test: self.try_map_expr(if_stmt.test)?,
                body: self.try_map_fragment(if_stmt.body)?,
                orelse: self.try_map_fragment(if_stmt.orelse)?,
            })),
        }
    }

    fn try_map_assign(
        &self,
        assign: BlockPyAssign<PassExpr<PIn>>,
    ) -> Result<BlockPyStmt<PassExpr<POut>>, Self::Error> {
        Ok(BlockPyStmt::Assign(BlockPyAssign {
            target: assign.target,
            value: self.try_map_expr(assign.value)?,
        }))
    }

    fn try_map_term(
        &self,
        term: BlockPyTerm<PassExpr<PIn>>,
    ) -> Result<BlockPyTerm<PassExpr<POut>>, Self::Error> {
        match term {
            BlockPyTerm::Jump(edge) => Ok(BlockPyTerm::Jump(BlockPyEdge {
                target: edge.target,
                args: edge.args,
            })),
            BlockPyTerm::IfTerm(if_term) => Ok(BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: self.try_map_expr(if_term.test)?,
                then_label: if_term.then_label,
                else_label: if_term.else_label,
            })),
            BlockPyTerm::BranchTable(branch) => Ok(BlockPyTerm::BranchTable(BlockPyBranchTable {
                index: self.try_map_expr(branch.index)?,
                targets: branch.targets,
                default_label: branch.default_label,
            })),
            BlockPyTerm::Raise(raise_stmt) => Ok(BlockPyTerm::Raise(BlockPyRaise {
                exc: raise_stmt
                    .exc
                    .map(|exc| self.try_map_expr(exc))
                    .transpose()?,
            })),
            BlockPyTerm::Return(value) => Ok(BlockPyTerm::Return(self.try_map_expr(value)?)),
        }
    }

    fn try_map_expr(&self, expr: PassExpr<PIn>) -> Result<PassExpr<POut>, Self::Error>;
}

impl<PIn> BlockPyModule<PIn>
where
    PIn: BlockPyPass,
{
    pub fn map_module<POut>(self, mapper: &impl BlockPyModuleMap<PIn, POut>) -> BlockPyModule<POut>
    where
        POut: BlockPyPass,
        BlockPyStmt<POut::Expr>: Into<POut::Stmt>,
    {
        mapper.map_module(self)
    }

    pub fn try_map_module<POut, M>(self, mapper: &M) -> Result<BlockPyModule<POut>, M::Error>
    where
        POut: BlockPyPass,
        BlockPyStmt<POut::Expr>: Into<POut::Stmt>,
        M: BlockPyModuleTryMap<PIn, POut>,
    {
        mapper.try_map_module(self)
    }
}

impl From<CoreBlockPyExprWithAwaitAndYield> for Expr {
    fn from(value: CoreBlockPyExprWithAwaitAndYield) -> Self {
        match value {
            CoreBlockPyExprWithAwaitAndYield::Literal(literal) => core_literal_to_expr(literal),
            CoreBlockPyExprWithAwaitAndYield::Call(node) => call_like_to_ast(
                Expr::from(*node.func),
                node.node_index,
                node.range,
                node.args,
                node.keywords,
            ),
            CoreBlockPyExprWithAwaitAndYield::Intrinsic(node) => call_like_to_ast(
                Expr::Name(intrinsic_name_expr(node.intrinsic)),
                node.node_index,
                node.range,
                node.args,
                node.keywords,
            ),
            CoreBlockPyExprWithAwaitAndYield::Await(node) => Expr::Await(ast::ExprAwait {
                node_index: node.node_index,
                range: node.range,
                value: Box::new(Expr::from(*node.value)),
            }),
            CoreBlockPyExprWithAwaitAndYield::Yield(node) => Expr::Yield(ast::ExprYield {
                node_index: node.node_index,
                range: node.range,
                value: node.value.map(|value| Box::new(Expr::from(*value))),
            }),
            CoreBlockPyExprWithAwaitAndYield::YieldFrom(node) => {
                Expr::YieldFrom(ast::ExprYieldFrom {
                    node_index: node.node_index,
                    range: node.range,
                    value: Box::new(Expr::from(*node.value)),
                })
            }
            CoreBlockPyExprWithAwaitAndYield::Name(node) => Expr::Name(node),
        }
    }
}

impl From<CoreBlockPyExprWithYield> for Expr {
    fn from(value: CoreBlockPyExprWithYield) -> Self {
        match value {
            CoreBlockPyExprWithYield::Literal(literal) => core_literal_to_expr(literal),
            CoreBlockPyExprWithYield::Call(node) => call_like_to_ast(
                Expr::from(*node.func),
                node.node_index,
                node.range,
                node.args,
                node.keywords,
            ),
            CoreBlockPyExprWithYield::Intrinsic(node) => call_like_to_ast(
                Expr::Name(intrinsic_name_expr(node.intrinsic)),
                node.node_index,
                node.range,
                node.args,
                node.keywords,
            ),
            CoreBlockPyExprWithYield::Yield(node) => Expr::Yield(ast::ExprYield {
                node_index: node.node_index,
                range: node.range,
                value: node.value.map(|value| Box::new(Expr::from(*value))),
            }),
            CoreBlockPyExprWithYield::YieldFrom(node) => Expr::YieldFrom(ast::ExprYieldFrom {
                node_index: node.node_index,
                range: node.range,
                value: Box::new(Expr::from(*node.value)),
            }),
            CoreBlockPyExprWithYield::Name(node) => Expr::Name(node),
        }
    }
}

impl From<CoreBlockPyExpr> for Expr {
    fn from(value: CoreBlockPyExpr) -> Self {
        match value {
            CoreBlockPyExpr::Literal(literal) => core_literal_to_expr(literal),
            CoreBlockPyExpr::Call(node) => call_like_to_ast(
                Expr::from(*node.func),
                node.node_index,
                node.range,
                node.args,
                node.keywords,
            ),
            CoreBlockPyExpr::Intrinsic(node) => call_like_to_ast(
                Expr::Name(intrinsic_name_expr(node.intrinsic)),
                node.node_index,
                node.range,
                node.args,
                node.keywords,
            ),
            CoreBlockPyExpr::Name(node) => Expr::Name(node),
        }
    }
}

fn core_literal_to_expr(literal: CoreBlockPyLiteral) -> Expr {
    match literal {
        CoreBlockPyLiteral::StringLiteral(node) => {
            let node_index = node.node_index.clone();
            Expr::StringLiteral(ast::ExprStringLiteral {
                node_index: node_index.clone(),
                range: node.range,
                value: StringLiteralValue::single(StringLiteral {
                    node_index,
                    range: node.range,
                    value: node.value.into(),
                    flags: StringLiteralFlags::empty().with_quote_style(Quote::Double),
                }),
            })
        }
        CoreBlockPyLiteral::BytesLiteral(node) => {
            let node_index = node.node_index.clone();
            Expr::BytesLiteral(ast::ExprBytesLiteral {
                node_index: node_index.clone(),
                range: node.range,
                value: ast::BytesLiteralValue::single(BytesLiteral {
                    node_index,
                    range: node.range,
                    value: node.value.into(),
                    flags: BytesLiteralFlags::empty().with_quote_style(Quote::Double),
                }),
            })
        }
        CoreBlockPyLiteral::NumberLiteral(node) => Expr::NumberLiteral(ast::ExprNumberLiteral {
            node_index: node.node_index,
            range: node.range,
            value: match node.value {
                CoreNumberLiteralValue::Int(value) => ast::Number::Int(value),
                CoreNumberLiteralValue::Float(value) => ast::Number::Float(value),
            },
        }),
    }
}

fn intrinsic_name_expr(intrinsic: &'static dyn intrinsics::Intrinsic) -> ast::ExprName {
    let Expr::Name(name) = py_expr!("{id:id}", id = intrinsic.name()) else {
        unreachable!();
    };
    name
}

fn call_args_to_ast<E: Into<Expr>>(args: Vec<CoreBlockPyCallArg<E>>) -> Box<[Expr]> {
    args.into_iter()
        .map(|arg| match arg {
            CoreBlockPyCallArg::Positional(expr) => expr.into(),
            CoreBlockPyCallArg::Starred(expr) => Expr::Starred(ast::ExprStarred {
                value: Box::new(expr.into()),
                ctx: ast::ExprContext::Load,
                range: Default::default(),
                node_index: ast::AtomicNodeIndex::default(),
            }),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn call_keywords_to_ast<E: Into<Expr>>(
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> Box<[ast::Keyword]> {
    keywords
        .into_iter()
        .map(|keyword| match keyword {
            CoreBlockPyKeywordArg::Named { arg, value } => ast::Keyword {
                arg: Some(arg),
                value: value.into(),
                range: Default::default(),
                node_index: ast::AtomicNodeIndex::default(),
            },
            CoreBlockPyKeywordArg::Starred(expr) => ast::Keyword {
                arg: None,
                value: expr.into(),
                range: Default::default(),
                node_index: ast::AtomicNodeIndex::default(),
            },
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn call_like_to_ast<E: Into<Expr>>(
    func: Expr,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> Expr {
    Expr::Call(ast::ExprCall {
        node_index,
        range,
        func: Box::new(func),
        arguments: ast::Arguments {
            args: call_args_to_ast(args),
            keywords: call_keywords_to_ast(keywords),
            range: Default::default(),
            node_index: ast::AtomicNodeIndex::default(),
        },
    })
}

impl TryFrom<CoreBlockPyExprWithAwaitAndYield> for CoreBlockPyExprWithYield {
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(value: CoreBlockPyExprWithAwaitAndYield) -> Result<Self, Self::Error> {
        match value {
            CoreBlockPyExprWithAwaitAndYield::Name(node) => Ok(Self::Name(node)),
            CoreBlockPyExprWithAwaitAndYield::Literal(literal) => Ok(Self::Literal(literal)),
            CoreBlockPyExprWithAwaitAndYield::Call(call) => Ok(Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::try_from(*call.func)?),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Positional)
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => Self::try_from(value)
                            .map(|value| CoreBlockPyKeywordArg::Named { arg, value }),
                        CoreBlockPyKeywordArg::Starred(value) => {
                            Self::try_from(value).map(CoreBlockPyKeywordArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
            })),
            CoreBlockPyExprWithAwaitAndYield::Intrinsic(call) => {
                Ok(Self::Intrinsic(IntrinsicCall {
                    intrinsic: call.intrinsic,
                    node_index: call.node_index,
                    range: call.range,
                    args: call
                        .args
                        .into_iter()
                        .map(|arg| match arg {
                            CoreBlockPyCallArg::Positional(expr) => {
                                Self::try_from(expr).map(CoreBlockPyCallArg::Positional)
                            }
                            CoreBlockPyCallArg::Starred(expr) => {
                                Self::try_from(expr).map(CoreBlockPyCallArg::Starred)
                            }
                        })
                        .collect::<Result<_, _>>()?,
                    keywords: call
                        .keywords
                        .into_iter()
                        .map(|keyword| match keyword {
                            CoreBlockPyKeywordArg::Named { arg, value } => Self::try_from(value)
                                .map(|value| CoreBlockPyKeywordArg::Named { arg, value }),
                            CoreBlockPyKeywordArg::Starred(value) => {
                                Self::try_from(value).map(CoreBlockPyKeywordArg::Starred)
                            }
                        })
                        .collect::<Result<_, _>>()?,
                }))
            }
            CoreBlockPyExprWithAwaitAndYield::Yield(yield_expr) => {
                Ok(Self::Yield(CoreBlockPyYield {
                    node_index: yield_expr.node_index,
                    range: yield_expr.range,
                    value: yield_expr
                        .value
                        .map(|value| Self::try_from(*value).map(Box::new))
                        .transpose()?,
                }))
            }
            CoreBlockPyExprWithAwaitAndYield::YieldFrom(yield_from_expr) => {
                Ok(Self::YieldFrom(CoreBlockPyYieldFrom {
                    node_index: yield_from_expr.node_index,
                    range: yield_from_expr.range,
                    value: Box::new(Self::try_from(*yield_from_expr.value)?),
                }))
            }
            CoreBlockPyExprWithAwaitAndYield::Await(_) => Err(value),
        }
    }
}

struct ElideAwaitExprTryMap;

impl BlockPyModuleTryMap<CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield>
    for ElideAwaitExprTryMap
{
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_map_expr(
        &self,
        expr: CoreBlockPyExprWithAwaitAndYield,
    ) -> Result<CoreBlockPyExprWithYield, Self::Error> {
        expr.try_into()
    }
}

impl TryFrom<BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>
    for BlockPyStmt<CoreBlockPyExprWithYield>
{
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(value: BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>) -> Result<Self, Self::Error> {
        ElideAwaitExprTryMap.try_map_stmt(value)
    }
}

impl TryFrom<BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>>
    for BlockPyTerm<CoreBlockPyExprWithYield>
{
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(value: BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>) -> Result<Self, Self::Error> {
        ElideAwaitExprTryMap.try_map_term(value)
    }
}

impl
    TryFrom<
        BlockPyCfgFragment<
            BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
            BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
        >,
    >
    for BlockPyCfgFragment<
        BlockPyStmt<CoreBlockPyExprWithYield>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >
{
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(
        value: BlockPyCfgFragment<
            BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
            BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
        >,
    ) -> Result<Self, Self::Error> {
        ElideAwaitExprTryMap.try_map_fragment(value)
    }
}

impl
    TryFrom<
        CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
            BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
        >,
    > for CfgBlock<BlockPyStmt<CoreBlockPyExprWithYield>, BlockPyTerm<CoreBlockPyExprWithYield>>
{
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(
        value: CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
            BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
        >,
    ) -> Result<Self, Self::Error> {
        ElideAwaitExprTryMap.try_map_block(value)
    }
}

impl From<CoreBlockPyExprWithYield> for CoreBlockPyExprWithAwaitAndYield {
    fn from(value: CoreBlockPyExprWithYield) -> Self {
        match value {
            CoreBlockPyExprWithYield::Name(node) => Self::Name(node),
            CoreBlockPyExprWithYield::Literal(literal) => Self::Literal(literal),
            CoreBlockPyExprWithYield::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::from(*call.func)),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            CoreBlockPyCallArg::Positional(Self::from(expr))
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            CoreBlockPyCallArg::Starred(Self::from(expr))
                        }
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: Self::from(value),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => {
                            CoreBlockPyKeywordArg::Starred(Self::from(value))
                        }
                    })
                    .collect(),
            }),
            CoreBlockPyExprWithYield::Intrinsic(call) => Self::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            CoreBlockPyCallArg::Positional(Self::from(expr))
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            CoreBlockPyCallArg::Starred(Self::from(expr))
                        }
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: Self::from(value),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => {
                            CoreBlockPyKeywordArg::Starred(Self::from(value))
                        }
                    })
                    .collect(),
            }),
            CoreBlockPyExprWithYield::Yield(yield_expr) => Self::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr.value.map(|value| Box::new(Self::from(*value))),
            }),
            CoreBlockPyExprWithYield::YieldFrom(yield_from_expr) => {
                Self::YieldFrom(CoreBlockPyYieldFrom {
                    node_index: yield_from_expr.node_index,
                    range: yield_from_expr.range,
                    value: Box::new(Self::from(*yield_from_expr.value)),
                })
            }
        }
    }
}

impl TryFrom<CoreBlockPyExprWithYield> for CoreBlockPyExpr {
    type Error = CoreBlockPyExprWithYield;

    fn try_from(value: CoreBlockPyExprWithYield) -> Result<Self, Self::Error> {
        match value {
            CoreBlockPyExprWithYield::Name(node) => Ok(Self::Name(node)),
            CoreBlockPyExprWithYield::Literal(literal) => Ok(Self::Literal(literal)),
            CoreBlockPyExprWithYield::Call(call) => Ok(Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::try_from(*call.func)?),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Positional)
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => Self::try_from(value)
                            .map(|value| CoreBlockPyKeywordArg::Named { arg, value }),
                        CoreBlockPyKeywordArg::Starred(value) => {
                            Self::try_from(value).map(CoreBlockPyKeywordArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
            })),
            CoreBlockPyExprWithYield::Intrinsic(call) => Ok(Self::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Positional)
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => Self::try_from(value)
                            .map(|value| CoreBlockPyKeywordArg::Named { arg, value }),
                        CoreBlockPyKeywordArg::Starred(value) => {
                            Self::try_from(value).map(CoreBlockPyKeywordArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
            })),
            CoreBlockPyExprWithYield::Yield(_) | CoreBlockPyExprWithYield::YieldFrom(_) => {
                Err(value)
            }
        }
    }
}

struct ElideYieldExprTryMap;

impl BlockPyModuleTryMap<CoreBlockPyPassWithYield, CoreBlockPyPass> for ElideYieldExprTryMap {
    type Error = CoreBlockPyExprWithYield;

    fn try_map_expr(&self, expr: CoreBlockPyExprWithYield) -> Result<CoreBlockPyExpr, Self::Error> {
        expr.try_into()
    }
}

impl TryFrom<BlockPyStmt<CoreBlockPyExprWithYield>> for BlockPyStmt<CoreBlockPyExpr> {
    type Error = CoreBlockPyExprWithYield;

    fn try_from(value: BlockPyStmt<CoreBlockPyExprWithYield>) -> Result<Self, Self::Error> {
        ElideYieldExprTryMap.try_map_stmt(value)
    }
}

impl TryFrom<BlockPyTerm<CoreBlockPyExprWithYield>> for BlockPyTerm<CoreBlockPyExpr> {
    type Error = CoreBlockPyExprWithYield;

    fn try_from(value: BlockPyTerm<CoreBlockPyExprWithYield>) -> Result<Self, Self::Error> {
        ElideYieldExprTryMap.try_map_term(value)
    }
}

impl
    TryFrom<
        BlockPyCfgFragment<
            BlockPyStmt<CoreBlockPyExprWithYield>,
            BlockPyTerm<CoreBlockPyExprWithYield>,
        >,
    > for BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>
{
    type Error = CoreBlockPyExprWithYield;

    fn try_from(
        value: BlockPyCfgFragment<
            BlockPyStmt<CoreBlockPyExprWithYield>,
            BlockPyTerm<CoreBlockPyExprWithYield>,
        >,
    ) -> Result<Self, Self::Error> {
        ElideYieldExprTryMap.try_map_fragment(value)
    }
}

impl TryFrom<CfgBlock<BlockPyStmt<CoreBlockPyExprWithYield>, BlockPyTerm<CoreBlockPyExprWithYield>>>
    for CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>
{
    type Error = CoreBlockPyExprWithYield;

    fn try_from(
        value: CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithYield>,
            BlockPyTerm<CoreBlockPyExprWithYield>,
        >,
    ) -> Result<Self, Self::Error> {
        ElideYieldExprTryMap.try_map_block(value)
    }
}

impl TryFrom<BlockPyFunction<CoreBlockPyPassWithYield>> for BlockPyFunction<CoreBlockPyPass> {
    type Error = CoreBlockPyExprWithYield;

    fn try_from(value: BlockPyFunction<CoreBlockPyPassWithYield>) -> Result<Self, Self::Error> {
        ElideYieldExprTryMap.try_map_fn(value)
    }
}

impl From<CoreBlockPyExpr> for CoreBlockPyExprWithYield {
    fn from(value: CoreBlockPyExpr) -> Self {
        match value {
            CoreBlockPyExpr::Name(node) => Self::Name(node),
            CoreBlockPyExpr::Literal(literal) => Self::Literal(literal),
            CoreBlockPyExpr::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::from(*call.func)),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            CoreBlockPyCallArg::Positional(Self::from(expr))
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            CoreBlockPyCallArg::Starred(Self::from(expr))
                        }
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: Self::from(value),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => {
                            CoreBlockPyKeywordArg::Starred(Self::from(value))
                        }
                    })
                    .collect(),
            }),
            CoreBlockPyExpr::Intrinsic(call) => Self::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            CoreBlockPyCallArg::Positional(Self::from(expr))
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            CoreBlockPyCallArg::Starred(Self::from(expr))
                        }
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: Self::from(value),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => {
                            CoreBlockPyKeywordArg::Starred(Self::from(value))
                        }
                    })
                    .collect(),
            }),
        }
    }
}

impl From<CoreBlockPyExpr> for CoreBlockPyExprWithAwaitAndYield {
    fn from(value: CoreBlockPyExpr) -> Self {
        Self::from(CoreBlockPyExprWithYield::from(value))
    }
}

impl CoreBlockPyExprWithAwaitAndYield {
    pub fn to_expr(&self) -> Expr {
        self.clone().into()
    }

    pub fn rewrite_mut(&mut self, f: impl FnOnce(&mut Expr)) {
        let mut expr = self.to_expr();
        f(&mut expr);
        *self = expr.into();
    }
}

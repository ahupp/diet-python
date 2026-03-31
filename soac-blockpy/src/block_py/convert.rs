use super::*;
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};
use std::marker::PhantomData;

pub(crate) fn map_call_args_with<EIn, EOut>(
    args: Vec<CoreBlockPyCallArg<EIn>>,
    mut map_expr: impl FnMut(EIn) -> EOut,
) -> Vec<CoreBlockPyCallArg<EOut>> {
    args.into_iter()
        .map(|arg| arg.map_expr(&mut map_expr))
        .collect()
}

pub(crate) fn map_keyword_args_with<EIn, EOut>(
    keywords: Vec<CoreBlockPyKeywordArg<EIn>>,
    mut map_expr: impl FnMut(EIn) -> EOut,
) -> Vec<CoreBlockPyKeywordArg<EOut>> {
    keywords
        .into_iter()
        .map(|keyword| keyword.map_expr(&mut map_expr))
        .collect()
}

pub(crate) fn try_map_call_args_with<EIn, EOut, Error>(
    args: Vec<CoreBlockPyCallArg<EIn>>,
    mut map_expr: impl FnMut(EIn) -> Result<EOut, Error>,
) -> Result<Vec<CoreBlockPyCallArg<EOut>>, Error> {
    args.into_iter()
        .map(|arg| arg.try_map_expr(&mut map_expr))
        .collect()
}

pub(crate) fn try_map_keyword_args_with<EIn, EOut, Error>(
    keywords: Vec<CoreBlockPyKeywordArg<EIn>>,
    mut map_expr: impl FnMut(EIn) -> Result<EOut, Error>,
) -> Result<Vec<CoreBlockPyKeywordArg<EOut>>, Error> {
    keywords
        .into_iter()
        .map(|keyword| keyword.try_map_expr(&mut map_expr))
        .collect()
}

fn map_stmt_with<PIn, POut>(
    mapper: &(impl BlockPyModuleMap<PIn, POut> + ?Sized),
    stmt: BlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
) -> BlockPyStmt<PassExpr<POut>, PassName<POut>>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PassExpr<PIn>: MapExpr<PassExpr<POut>>,
    PIn::Stmt: Into<BlockPyStmt<PIn::Expr, PIn::Name>>,
    PassName<POut>: From<PassName<PIn>>,
    POut::Stmt: From<BlockPyStmt<POut::Expr, POut::Name>>,
{
    match stmt {
        BlockPyStmt::Assign(assign) => BlockPyStmt::Assign(BlockPyAssign {
            target: mapper.map_name(assign.target),
            value: mapper.map_expr(assign.value),
        }),
        BlockPyStmt::Expr(expr) => BlockPyStmt::Expr(mapper.map_expr(expr)),
        BlockPyStmt::Delete(delete) => BlockPyStmt::Delete(BlockPyDelete {
            target: mapper.map_name(delete.target),
        }),
    }
}

fn try_map_stmt_with<PIn: BlockPyPass, POut: BlockPyPass, M>(
    mapper: &M,
    stmt: BlockPyStmt<PIn::Expr, PIn::Name>,
) -> Result<BlockPyStmt<POut::Expr, POut::Name>, M::Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    M: BlockPyModuleTryMap<PIn, POut> + ?Sized,
    PIn::Expr: TryMapExpr<POut::Expr, M::Error>,
    PIn::Stmt: Into<BlockPyStmt<PIn::Expr, PIn::Name>>,
    POut::Name: From<PIn::Name>,
    POut::Stmt: From<BlockPyStmt<POut::Expr, POut::Name>>,
{
    match stmt {
        BlockPyStmt::Assign(assign) => Ok(BlockPyStmt::Assign(BlockPyAssign {
            target: mapper.try_map_name(assign.target)?,
            value: mapper.try_map_expr(assign.value)?,
        })),
        BlockPyStmt::Expr(expr) => Ok(BlockPyStmt::Expr(mapper.try_map_expr(expr)?)),
        BlockPyStmt::Delete(delete) => Ok(BlockPyStmt::Delete(BlockPyDelete {
            target: mapper.try_map_name(delete.target)?,
        })),
    }
}

pub(crate) trait BlockPyModuleMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PIn::Expr: MapExpr<POut::Expr>,
    PIn::Stmt: Into<BlockPyStmt<PIn::Expr, PIn::Name>>,
    POut::Name: From<PIn::Name>,
    POut::Stmt: From<BlockPyStmt<POut::Expr, POut::Name>>,
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
            storage_layout: func.storage_layout,
            semantic: func.semantic,
        }
    }

    fn map_block(&self, block: PassBlock<PIn>) -> PassBlock<POut> {
        CfgBlock {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| self.map_stmt(stmt))
                .collect(),
            term: self.map_term(block.term),
            params: block.params,
            exc_edge: block.exc_edge,
        }
    }

    fn map_stmt(&self, stmt: PIn::Stmt) -> POut::Stmt {
        map_stmt_with(self, stmt.into()).into()
    }

    fn map_term(&self, term: BlockPyTerm<PIn::Expr>) -> BlockPyTerm<POut::Expr> {
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

    fn map_name(&self, name: PIn::Name) -> POut::Name {
        <POut::Name as From<PIn::Name>>::from(name)
    }

    fn map_nested_expr(&self, expr: PIn::Expr) -> POut::Expr {
        expr.map_expr(&mut |child| self.map_expr(child))
    }

    fn map_expr(&self, expr: PIn::Expr) -> POut::Expr {
        self.map_nested_expr(expr)
    }
}

pub(crate) trait BlockPyModuleTryMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PIn::Expr: TryMapExpr<POut::Expr, Self::Error>,
    PIn::Stmt: Into<BlockPyStmt<PIn::Expr, PIn::Name>>,
    POut::Name: From<PIn::Name>,
    POut::Stmt: From<BlockPyStmt<POut::Expr, POut::Name>>,
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
            storage_layout: func.storage_layout,
            semantic: func.semantic,
        })
    }

    fn try_map_block(&self, block: PassBlock<PIn>) -> Result<PassBlock<POut>, Self::Error> {
        Ok(CfgBlock {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| self.try_map_stmt(stmt))
                .collect::<Result<_, _>>()?,
            term: self.try_map_term(block.term)?,
            params: block.params,
            exc_edge: block.exc_edge,
        })
    }

    fn try_map_stmt(&self, stmt: PIn::Stmt) -> Result<POut::Stmt, Self::Error> {
        try_map_stmt_with(self, stmt.into()).map(Into::into)
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

    fn try_map_name(&self, name: PIn::Name) -> Result<POut::Name, Self::Error> {
        Ok(<POut::Name as From<PIn::Name>>::from(name))
    }

    fn try_map_nested_expr(&self, expr: PIn::Expr) -> Result<POut::Expr, Self::Error> {
        expr.try_map_expr(&mut |child| self.try_map_expr(child))
    }

    fn try_map_expr(&self, expr: PIn::Expr) -> Result<POut::Expr, Self::Error> {
        self.try_map_nested_expr(expr)
    }
}

pub(crate) struct ExprTryMap<PIn, POut>(PhantomData<fn() -> (PIn, POut)>);

impl<PIn, POut> ExprTryMap<PIn, POut> {
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<PIn, POut> ExprTryMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PIn::Expr: TryMapExpr<POut::Expr, <POut::Expr as TryFrom<PIn::Expr>>::Error>,
    PIn::Stmt: Into<BlockPyStmt<PIn::Expr, PIn::Name>>,
    POut::Expr: TryFrom<PIn::Expr>,
    POut::Name: From<PIn::Name>,
    POut::Stmt: From<BlockPyStmt<POut::Expr, POut::Name>>,
{
    pub(crate) fn try_map_stmt(
        &self,
        stmt: BlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
    ) -> Result<
        BlockPyStmt<PassExpr<POut>, PassName<POut>>,
        <POut::Expr as TryFrom<PIn::Expr>>::Error,
    > {
        try_map_stmt_with(self, stmt)
    }

    pub(crate) fn try_map_term(
        &self,
        term: BlockPyTerm<PassExpr<PIn>>,
    ) -> Result<BlockPyTerm<PassExpr<POut>>, <POut::Expr as TryFrom<PIn::Expr>>::Error> {
        <Self as BlockPyModuleTryMap<PIn, POut>>::try_map_term(self, term)
    }

    pub(crate) fn try_map_block(
        &self,
        block: CfgBlock<PIn::Stmt, BlockPyTerm<PassExpr<PIn>>>,
    ) -> Result<
        CfgBlock<POut::Stmt, BlockPyTerm<PassExpr<POut>>>,
        <POut::Expr as TryFrom<PIn::Expr>>::Error,
    > {
        <Self as BlockPyModuleTryMap<PIn, POut>>::try_map_block(self, block)
    }

    pub(crate) fn try_map_fn(
        &self,
        function: BlockPyFunction<PIn>,
    ) -> Result<BlockPyFunction<POut>, <POut::Expr as TryFrom<PIn::Expr>>::Error> {
        <Self as BlockPyModuleTryMap<PIn, POut>>::try_map_fn(self, function)
    }
}

impl<PIn, POut> BlockPyModuleTryMap<PIn, POut> for ExprTryMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PIn::Expr: TryMapExpr<POut::Expr, <POut::Expr as TryFrom<PIn::Expr>>::Error>,
    PIn::Stmt: Into<BlockPyStmt<PIn::Expr, PIn::Name>>,
    POut::Expr: TryFrom<PIn::Expr>,
    POut::Name: From<PIn::Name>,
    POut::Stmt: From<BlockPyStmt<POut::Expr, POut::Name>>,
{
    type Error = <POut::Expr as TryFrom<PIn::Expr>>::Error;

    fn try_map_expr(&self, expr: PIn::Expr) -> Result<POut::Expr, Self::Error> {
        expr.try_into()
    }
}

impl<PIn> BlockPyModule<PIn>
where
    PIn: BlockPyPass,
{
    pub(crate) fn map_module<POut>(
        self,
        mapper: &impl BlockPyModuleMap<PIn, POut>,
    ) -> BlockPyModule<POut>
    where
        POut: BlockPyPass,
        PIn::Expr: MapExpr<POut::Expr>,
        PIn::Stmt: Into<BlockPyStmt<PIn::Expr, PIn::Name>>,
        POut::Name: From<PIn::Name>,
        POut::Stmt: From<BlockPyStmt<POut::Expr, POut::Name>>,
    {
        mapper.map_module(self)
    }

    pub(crate) fn try_map_module<POut, M>(self, mapper: &M) -> Result<BlockPyModule<POut>, M::Error>
    where
        POut: BlockPyPass,
        PassExpr<PIn>: TryMapExpr<PassExpr<POut>, M::Error>,
        PIn::Stmt: Into<BlockPyStmt<PassExpr<PIn>, PassName<PIn>>>,
        PassName<POut>: From<PassName<PIn>>,
        POut::Stmt: From<BlockPyStmt<POut::Expr, POut::Name>>,
        M: BlockPyModuleTryMap<PIn, POut>,
    {
        mapper.try_map_module(self)
    }
}

impl TryFrom<CoreBlockPyExprWithAwaitAndYield> for CoreBlockPyExprWithYield {
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(value: CoreBlockPyExprWithAwaitAndYield) -> Result<Self, Self::Error> {
        value.try_map_expr(&mut |child| child.try_into())
    }
}

impl From<CoreBlockPyExprWithYield> for CoreBlockPyExprWithAwaitAndYield {
    fn from(value: CoreBlockPyExprWithYield) -> Self {
        match value {
            CoreBlockPyExprWithYield::Name(node) => Self::Name(node),
            CoreBlockPyExprWithYield::Literal(literal) => Self::Literal(literal),
            CoreBlockPyExprWithYield::Op(operation) => {
                Self::Op(Box::new(operation.map_expr(&mut Self::from)))
            }
            CoreBlockPyExprWithYield::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::from(*call.func)),
                args: map_call_args_with(call.args, Self::from),
                keywords: map_keyword_args_with(call.keywords, Self::from),
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
        value.try_map_expr(&mut |child| child.try_into())
    }
}

impl From<CoreBlockPyExpr> for CoreBlockPyExprWithYield {
    fn from(value: CoreBlockPyExpr) -> Self {
        match value {
            CoreBlockPyExpr::Name(node) => Self::Name(node.into()),
            CoreBlockPyExpr::Literal(literal) => Self::Literal(literal),
            CoreBlockPyExpr::Op(operation) => {
                Self::Op(Box::new(operation.map_expr(&mut Self::from)))
            }
            CoreBlockPyExpr::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::from(*call.func)),
                args: map_call_args_with(call.args, Self::from),
                keywords: map_keyword_args_with(call.keywords, Self::from),
            }),
        }
    }
}

impl From<CoreBlockPyExpr> for CoreBlockPyExprWithAwaitAndYield {
    fn from(value: CoreBlockPyExpr) -> Self {
        Self::from(CoreBlockPyExprWithYield::from(value))
    }
}

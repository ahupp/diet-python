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
    stmt: PassStmt<PIn>,
) -> PassStmt<POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PassExpr<PIn>: MapExpr<PassExpr<POut>>,
    PIn::Stmt: Into<PassStmt<PIn>>,
    PassName<POut>: From<PassName<PIn>>,
    POut::Stmt: From<PassStmt<POut>>,
{
    match stmt {
        BlockPyStmt::Expr(expr) => BlockPyStmt::Expr(mapper.map_expr(expr)),
        BlockPyStmt::_Marker(_) => unreachable!("linear stmt marker should not appear"),
    }
}

fn try_map_stmt_with<PIn: BlockPyPass, POut: BlockPyPass, M>(
    mapper: &M,
    stmt: PassStmt<PIn>,
) -> Result<PassStmt<POut>, M::Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    M: BlockPyModuleTryMap<PIn, POut> + ?Sized,
    PassExpr<PIn>: TryMapExpr<PassExpr<POut>, M::Error>,
    PIn::Stmt: Into<PassStmt<PIn>>,
    PassName<POut>: From<PassName<PIn>>,
    POut::Stmt: From<PassStmt<POut>>,
{
    match stmt {
        BlockPyStmt::Expr(expr) => Ok(BlockPyStmt::Expr(mapper.try_map_expr(expr)?)),
        BlockPyStmt::_Marker(_) => unreachable!("linear stmt marker should not appear"),
    }
}

pub(crate) trait BlockPyModuleMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PassExpr<PIn>: MapExpr<PassExpr<POut>>,
    PIn::Stmt: Into<PassStmt<PIn>>,
    PassName<POut>: From<PassName<PIn>>,
    POut::Stmt: From<PassStmt<POut>>,
{
    fn map_module(&self, module: BlockPyModule<PIn>) -> BlockPyModule<POut> {
        BlockPyModule {
            callable_defs: module
                .callable_defs
                .into_iter()
                .map(|function| self.map_fn(function))
                .collect(),
            module_constants: module.module_constants,
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

    fn map_name(&self, name: PassName<PIn>) -> PassName<POut> {
        <PassName<POut> as From<PassName<PIn>>>::from(name)
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
    PassExpr<PIn>: TryMapExpr<PassExpr<POut>, Self::Error>,
    PIn::Stmt: Into<PassStmt<PIn>>,
    PassName<POut>: From<PassName<PIn>>,
    POut::Stmt: From<PassStmt<POut>>,
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
            module_constants: module.module_constants,
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

    fn try_map_name(&self, name: PassName<PIn>) -> Result<PassName<POut>, Self::Error> {
        Ok(<PassName<POut> as From<PassName<PIn>>>::from(name))
    }

    fn try_map_nested_expr(&self, expr: PIn::Expr) -> Result<POut::Expr, Self::Error> {
        expr.try_map_expr(&mut |child| self.try_map_expr(child))
    }

    fn try_map_expr(&self, expr: PIn::Expr) -> Result<POut::Expr, Self::Error> {
        self.try_map_nested_expr(expr)
    }
}

pub(crate) struct ExprTryMap<PIn: BlockPyPass, POut: BlockPyPass, Error> {
    lower_expr: fn(PIn::Expr) -> Result<POut::Expr, Error>,
    _marker: PhantomData<fn() -> (PIn, POut, Error)>,
}

impl<PIn: BlockPyPass, POut: BlockPyPass, Error> ExprTryMap<PIn, POut, Error> {
    pub(crate) const fn new(lower_expr: fn(PIn::Expr) -> Result<POut::Expr, Error>) -> Self {
        Self {
            lower_expr,
            _marker: PhantomData,
        }
    }
}

impl
    ExprTryMap<
        CoreBlockPyPassWithAwaitAndYield,
        CoreBlockPyPassWithYield,
        CoreBlockPyExprWithAwaitAndYield,
    >
{
    pub(crate) const fn without_await() -> Self {
        Self::new(try_lower_core_expr_without_await)
    }
}

impl ExprTryMap<CoreBlockPyPassWithYield, CoreBlockPyPass, CoreBlockPyExprWithYield> {
    pub(crate) const fn without_yield() -> Self {
        Self::new(try_lower_core_expr_without_yield)
    }
}

impl<PIn, POut, Error> ExprTryMap<PIn, POut, Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PassExpr<PIn>: TryMapExpr<PassExpr<POut>, Error>,
    PIn::Stmt: Into<PassStmt<PIn>>,
    PassName<POut>: From<PassName<PIn>>,
    POut::Stmt: From<PassStmt<POut>>,
{
    pub(crate) fn try_map_stmt(&self, stmt: PassStmt<PIn>) -> Result<PassStmt<POut>, Error> {
        try_map_stmt_with(self, stmt)
    }

    pub(crate) fn try_map_term(
        &self,
        term: BlockPyTerm<PassExpr<PIn>>,
    ) -> Result<BlockPyTerm<PassExpr<POut>>, Error> {
        <Self as BlockPyModuleTryMap<PIn, POut>>::try_map_term(self, term)
    }

    pub(crate) fn try_map_block(
        &self,
        block: CfgBlock<PIn::Stmt, BlockPyTerm<PassExpr<PIn>>>,
    ) -> Result<CfgBlock<POut::Stmt, BlockPyTerm<PassExpr<POut>>>, Error> {
        <Self as BlockPyModuleTryMap<PIn, POut>>::try_map_block(self, block)
    }

    pub(crate) fn try_map_fn(
        &self,
        function: BlockPyFunction<PIn>,
    ) -> Result<BlockPyFunction<POut>, Error> {
        <Self as BlockPyModuleTryMap<PIn, POut>>::try_map_fn(self, function)
    }
}

impl<PIn, POut, Error> BlockPyModuleTryMap<PIn, POut> for ExprTryMap<PIn, POut, Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PassExpr<PIn>: TryMapExpr<PassExpr<POut>, Error>,
    PIn::Stmt: Into<PassStmt<PIn>>,
    PassName<POut>: From<PassName<PIn>>,
    POut::Stmt: From<PassStmt<POut>>,
{
    type Error = Error;

    fn try_map_expr(&self, expr: PIn::Expr) -> Result<POut::Expr, Self::Error> {
        (self.lower_expr)(expr)
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
        PassExpr<PIn>: MapExpr<PassExpr<POut>>,
        PIn::Stmt: Into<PassStmt<PIn>>,
        PassName<POut>: From<PassName<PIn>>,
        POut::Stmt: From<PassStmt<POut>>,
    {
        mapper.map_module(self)
    }

    pub(crate) fn try_map_module<POut, M>(self, mapper: &M) -> Result<BlockPyModule<POut>, M::Error>
    where
        POut: BlockPyPass,
        PassExpr<PIn>: TryMapExpr<PassExpr<POut>, M::Error>,
        PIn::Stmt: Into<PassStmt<PIn>>,
        PassName<POut>: From<PassName<PIn>>,
        POut::Stmt: From<PassStmt<POut>>,
        M: BlockPyModuleTryMap<PIn, POut>,
    {
        mapper.try_map_module(self)
    }
}

pub(crate) fn try_lower_core_expr_without_await(
    value: CoreBlockPyExprWithAwaitAndYield,
) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield> {
    value.try_map_expr(&mut try_lower_core_expr_without_await)
}

pub(crate) fn try_lower_core_expr_without_yield(
    value: CoreBlockPyExprWithYield,
) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield> {
    value.try_map_expr(&mut try_lower_core_expr_without_yield)
}

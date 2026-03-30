use super::*;
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};
use ruff_python_ast::{self as ast};

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

fn map_structured_stmt_with<PIn, POut>(
    mapper: &(impl BlockPyModuleMap<PIn, POut> + ?Sized),
    stmt: StructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
) -> StructuredBlockPyStmt<PassExpr<POut>, PassName<POut>>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PassExpr<PIn>: MapExpr<PassExpr<POut>>,
    PIn::Stmt: IntoStructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
    PassName<POut>: From<PassName<PIn>>,
    StructuredBlockPyStmt<POut::Expr, POut::Name>: Into<POut::Stmt>,
{
    match stmt {
        StructuredBlockPyStmt::Assign(assign) => StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: mapper.map_name(assign.target),
            value: mapper.map_expr(assign.value),
        }),
        StructuredBlockPyStmt::Expr(expr) => StructuredBlockPyStmt::Expr(mapper.map_expr(expr)),
        StructuredBlockPyStmt::Delete(delete) => StructuredBlockPyStmt::Delete(BlockPyDelete {
            target: mapper.map_name(delete.target),
        }),
        StructuredBlockPyStmt::If(if_stmt) => StructuredBlockPyStmt::If(BlockPyIf {
            test: mapper.map_expr(if_stmt.test),
            body: map_fragment_with(mapper, if_stmt.body),
            orelse: map_fragment_with(mapper, if_stmt.orelse),
        }),
    }
}

fn map_fragment_with<PIn, POut>(
    mapper: &(impl BlockPyModuleMap<PIn, POut> + ?Sized),
    fragment: BlockPyCfgFragment<
        StructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
        BlockPyTerm<PassExpr<PIn>>,
    >,
) -> BlockPyCfgFragment<
    StructuredBlockPyStmt<PassExpr<POut>, PassName<POut>>,
    BlockPyTerm<PassExpr<POut>>,
>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PassExpr<PIn>: MapExpr<PassExpr<POut>>,
    PIn::Stmt: IntoStructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
    PassName<POut>: From<PassName<PIn>>,
    StructuredBlockPyStmt<POut::Expr, POut::Name>: Into<POut::Stmt>,
{
    BlockPyCfgFragment {
        body: fragment
            .body
            .into_iter()
            .map(|stmt| map_structured_stmt_with(mapper, stmt))
            .collect(),
        term: fragment.term.map(|term| mapper.map_term(term)),
    }
}

fn try_map_structured_stmt_with<PIn, POut, M>(
    mapper: &M,
    stmt: StructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
) -> Result<StructuredBlockPyStmt<PassExpr<POut>, PassName<POut>>, M::Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PIn::Stmt: IntoStructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
    M: BlockPyModuleTryMap<PIn, POut> + ?Sized,
    PassExpr<PIn>: TryMapExpr<PassExpr<POut>, M::Error>,
    PassName<POut>: From<PassName<PIn>>,
    StructuredBlockPyStmt<POut::Expr, POut::Name>: Into<POut::Stmt>,
{
    match stmt {
        StructuredBlockPyStmt::Assign(assign) => Ok(StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: mapper.try_map_name(assign.target)?,
            value: mapper.try_map_expr(assign.value)?,
        })),
        StructuredBlockPyStmt::Expr(expr) => {
            Ok(StructuredBlockPyStmt::Expr(mapper.try_map_expr(expr)?))
        }
        StructuredBlockPyStmt::Delete(delete) => Ok(StructuredBlockPyStmt::Delete(BlockPyDelete {
            target: mapper.try_map_name(delete.target)?,
        })),
        StructuredBlockPyStmt::If(if_stmt) => Ok(StructuredBlockPyStmt::If(BlockPyIf {
            test: mapper.try_map_expr(if_stmt.test)?,
            body: try_map_fragment_with(mapper, if_stmt.body)?,
            orelse: try_map_fragment_with(mapper, if_stmt.orelse)?,
        })),
    }
}

fn try_map_fragment_with<PIn, POut, M>(
    mapper: &M,
    fragment: BlockPyCfgFragment<
        StructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
        BlockPyTerm<PassExpr<PIn>>,
    >,
) -> Result<
    BlockPyCfgFragment<
        StructuredBlockPyStmt<PassExpr<POut>, PassName<POut>>,
        BlockPyTerm<PassExpr<POut>>,
    >,
    M::Error,
>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PIn::Stmt: IntoStructuredBlockPyStmt<PIn::Expr, PIn::Name>,
    M: BlockPyModuleTryMap<PIn, POut> + ?Sized,
    PIn::Expr: TryMapExpr<POut::Expr, M::Error>,
    POut::Name: From<PIn::Name>,
    StructuredBlockPyStmt<POut::Expr, POut::Name>: Into<POut::Stmt>,
{
    Ok(BlockPyCfgFragment {
        body: fragment
            .body
            .into_iter()
            .map(|stmt| try_map_structured_stmt_with(mapper, stmt))
            .collect::<Result<_, _>>()?,
        term: fragment
            .term
            .map(|term| mapper.try_map_term(term))
            .transpose()?,
    })
}

pub(crate) trait BlockPyModuleMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    PIn::Expr: MapExpr<POut::Expr>,
    PIn::Stmt: IntoStructuredBlockPyStmt<PIn::Expr, PIn::Name>,
    POut::Name: From<PIn::Name>,
    StructuredBlockPyStmt<POut::Expr, POut::Name>: Into<POut::Stmt>,
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
        map_structured_stmt_with(self, stmt.into_structured_stmt()).into()
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
    PIn::Stmt: IntoStructuredBlockPyStmt<PIn::Expr, PIn::Name>,
    POut::Name: From<PIn::Name>,
    StructuredBlockPyStmt<POut::Expr, POut::Name>: Into<POut::Stmt>,
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
        try_map_structured_stmt_with(self, stmt.into_structured_stmt()).map(Into::into)
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
        PIn::Stmt: IntoStructuredBlockPyStmt<PIn::Expr, PIn::Name>,
        POut::Name: From<PIn::Name>,
        StructuredBlockPyStmt<POut::Expr, POut::Name>: Into<POut::Stmt>,
    {
        mapper.map_module(self)
    }

    pub(crate) fn try_map_module<POut, M>(self, mapper: &M) -> Result<BlockPyModule<POut>, M::Error>
    where
        POut: BlockPyPass,
        PassExpr<PIn>: TryMapExpr<PassExpr<POut>, M::Error>,
        PIn::Stmt: IntoStructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
        PassName<POut>: From<PassName<PIn>>,
        StructuredBlockPyStmt<POut::Expr, POut::Name>: Into<POut::Stmt>,
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

impl TryFrom<StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>
    for StructuredBlockPyStmt<CoreBlockPyExprWithYield>
{
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(
        value: StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
    ) -> Result<Self, Self::Error> {
        try_map_structured_stmt_with(&ElideAwaitExprTryMap, value)
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
            StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
            BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
        >,
    >
    for BlockPyCfgFragment<
        StructuredBlockPyStmt<CoreBlockPyExprWithYield>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >
{
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(
        value: BlockPyCfgFragment<
            StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
            BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
        >,
    ) -> Result<Self, Self::Error> {
        try_map_fragment_with(&ElideAwaitExprTryMap, value)
    }
}

impl
    TryFrom<
        CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithAwaitAndYield, ast::ExprName>,
            BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
        >,
    >
    for CfgBlock<
        BlockPyStmt<CoreBlockPyExprWithYield, ast::ExprName>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >
{
    type Error = CoreBlockPyExprWithAwaitAndYield;

    fn try_from(
        value: CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithAwaitAndYield, ast::ExprName>,
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

struct ElideYieldExprTryMap;

impl BlockPyModuleTryMap<CoreBlockPyPassWithYield, CoreBlockPyPass> for ElideYieldExprTryMap {
    type Error = CoreBlockPyExprWithYield;

    fn try_map_expr(&self, expr: CoreBlockPyExprWithYield) -> Result<CoreBlockPyExpr, Self::Error> {
        expr.try_into()
    }
}

impl TryFrom<StructuredBlockPyStmt<CoreBlockPyExprWithYield>>
    for StructuredBlockPyStmt<CoreBlockPyExpr>
{
    type Error = CoreBlockPyExprWithYield;

    fn try_from(
        value: StructuredBlockPyStmt<CoreBlockPyExprWithYield>,
    ) -> Result<Self, Self::Error> {
        try_map_structured_stmt_with(&ElideYieldExprTryMap, value)
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
            StructuredBlockPyStmt<CoreBlockPyExprWithYield>,
            BlockPyTerm<CoreBlockPyExprWithYield>,
        >,
    > for BlockPyCfgFragment<StructuredBlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>
{
    type Error = CoreBlockPyExprWithYield;

    fn try_from(
        value: BlockPyCfgFragment<
            StructuredBlockPyStmt<CoreBlockPyExprWithYield>,
            BlockPyTerm<CoreBlockPyExprWithYield>,
        >,
    ) -> Result<Self, Self::Error> {
        try_map_fragment_with(&ElideYieldExprTryMap, value)
    }
}

impl
    TryFrom<
        CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithYield, ast::ExprName>,
            BlockPyTerm<CoreBlockPyExprWithYield>,
        >,
    > for CfgBlock<BlockPyStmt<CoreBlockPyExpr, ast::ExprName>, BlockPyTerm<CoreBlockPyExpr>>
{
    type Error = CoreBlockPyExprWithYield;

    fn try_from(
        value: CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithYield, ast::ExprName>,
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

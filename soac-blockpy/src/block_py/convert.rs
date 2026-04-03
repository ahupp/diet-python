use super::*;
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithYield};
use std::marker::PhantomData;

pub(crate) trait BlockPyModuleMap<PIn, POut>: MapExpr<PIn::Expr, POut::Expr>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    <POut::Expr as Instr>::Name: From<<PIn::Expr as Instr>::Name>,
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

    fn map_block(&self, block: Block<PIn::Expr>) -> Block<POut::Expr> {
        Block {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| self.map_expr(stmt))
                .collect(),
            term: self.map_term(block.term),
            params: block.params,
            exc_edge: block.exc_edge,
        }
    }

    fn map_term(&self, term: BlockTerm<PIn::Expr>) -> BlockTerm<POut::Expr> {
        match term {
            BlockTerm::Jump(edge) => BlockTerm::Jump(BlockEdge {
                target: edge.target,
                args: edge.args,
            }),
            BlockTerm::IfTerm(if_term) => BlockTerm::IfTerm(TermIf {
                test: self.map_expr(if_term.test),
                then_label: if_term.then_label,
                else_label: if_term.else_label,
            }),
            BlockTerm::BranchTable(branch) => BlockTerm::BranchTable(TermBranchTable {
                index: self.map_expr(branch.index),
                targets: branch.targets,
                default_label: branch.default_label,
            }),
            BlockTerm::Raise(raise_stmt) => BlockTerm::Raise(TermRaise {
                exc: raise_stmt.exc.map(|exc| self.map_expr(exc)),
            }),
            BlockTerm::Return(value) => BlockTerm::Return(self.map_expr(value)),
        }
    }

    fn map_name(&self, name: <PIn::Expr as Instr>::Name) -> <POut::Expr as Instr>::Name {
        <<POut::Expr as Instr>::Name as From<<PIn::Expr as Instr>::Name>>::from(name)
    }
}

pub(crate) trait BlockPyModuleTryMap<PIn, POut>:
    TryMapExpr<PIn::Expr, POut::Expr, Self::Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    <POut::Expr as Instr>::Name: From<<PIn::Expr as Instr>::Name>,
{
    type Error;

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

    fn try_map_block(&self, block: Block<PIn::Expr>) -> Result<Block<POut::Expr>, Self::Error> {
        Ok(Block {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| self.try_map_expr(stmt))
                .collect::<Result<_, _>>()?,
            term: self.try_map_term(block.term)?,
            params: block.params,
            exc_edge: block.exc_edge,
        })
    }

    fn try_map_term(
        &self,
        term: BlockTerm<PIn::Expr>,
    ) -> Result<BlockTerm<POut::Expr>, Self::Error> {
        match term {
            BlockTerm::Jump(edge) => Ok(BlockTerm::Jump(BlockEdge {
                target: edge.target,
                args: edge.args,
            })),
            BlockTerm::IfTerm(if_term) => Ok(BlockTerm::IfTerm(TermIf {
                test: self.try_map_expr(if_term.test)?,
                then_label: if_term.then_label,
                else_label: if_term.else_label,
            })),
            BlockTerm::BranchTable(branch) => Ok(BlockTerm::BranchTable(TermBranchTable {
                index: self.try_map_expr(branch.index)?,
                targets: branch.targets,
                default_label: branch.default_label,
            })),
            BlockTerm::Raise(raise_stmt) => Ok(BlockTerm::Raise(TermRaise {
                exc: raise_stmt
                    .exc
                    .map(|exc| self.try_map_expr(exc))
                    .transpose()?,
            })),
            BlockTerm::Return(value) => Ok(BlockTerm::Return(self.try_map_expr(value)?)),
        }
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

impl ExprTryMap<CoreBlockPyPassWithYield, CoreBlockPyPass, CoreBlockPyExprWithYield> {
    pub(crate) const fn without_yield() -> Self {
        Self::new(try_lower_core_expr_without_yield)
    }
}

impl<PIn, POut, Error> ExprTryMap<PIn, POut, Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    <POut::Expr as Instr>::Name: From<<PIn::Expr as Instr>::Name>,
{
    pub(crate) fn try_map_expr(&self, expr: PIn::Expr) -> Result<POut::Expr, Error> {
        (self.lower_expr)(expr)
    }

    pub(crate) fn try_map_term(
        &self,
        term: BlockTerm<PIn::Expr>,
    ) -> Result<BlockTerm<POut::Expr>, Error> {
        <Self as BlockPyModuleTryMap<PIn, POut>>::try_map_term(self, term)
    }

    pub(crate) fn try_map_fn(
        &self,
        function: BlockPyFunction<PIn>,
    ) -> Result<BlockPyFunction<POut>, Error> {
        <Self as BlockPyModuleTryMap<PIn, POut>>::try_map_fn(self, function)
    }
}

impl<PIn, POut, Error> TryMapExpr<PIn::Expr, POut::Expr, Error> for ExprTryMap<PIn, POut, Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    <POut::Expr as Instr>::Name: From<<PIn::Expr as Instr>::Name>,
{
    fn try_map_expr(&self, expr: PIn::Expr) -> Result<POut::Expr, Error> {
        (self.lower_expr)(expr)
    }
}

impl<PIn, POut, Error> BlockPyModuleTryMap<PIn, POut> for ExprTryMap<PIn, POut, Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    <POut::Expr as Instr>::Name: From<<PIn::Expr as Instr>::Name>,
{
    type Error = Error;
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
        <POut::Expr as Instr>::Name: From<<PIn::Expr as Instr>::Name>,
    {
        mapper.map_module(self)
    }
}

struct ErrOnAwait;

impl
    TryMapExpr<
        CoreBlockPyExprWithAwaitAndYield,
        CoreBlockPyExprWithYield,
        CoreBlockPyExprWithAwaitAndYield,
    > for ErrOnAwait
{
    fn try_map_expr(
        &self,
        expr: CoreBlockPyExprWithAwaitAndYield,
    ) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield> {
        match expr {
            CoreBlockPyExprWithAwaitAndYield::Await(_) => Err(expr),
            CoreBlockPyExprWithAwaitAndYield::Literal(node) => Ok(node.into()),
            CoreBlockPyExprWithAwaitAndYield::BinOp(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::UnaryOp(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::Call(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::GetAttr(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::SetAttr(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::GetItem(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::SetItem(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::DelItem(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::Load(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::Store(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::Del(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::MakeCell(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::CellRefForName(node) => Ok(node.into()),
            CoreBlockPyExprWithAwaitAndYield::CellRef(node) => Ok(node.into()),
            CoreBlockPyExprWithAwaitAndYield::MakeFunction(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::Yield(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithAwaitAndYield::YieldFrom(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
        }
    }
}

pub(crate) fn try_lower_core_expr_without_await(
    value: CoreBlockPyExprWithAwaitAndYield,
) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield> {
    ErrOnAwait.try_map_expr(value)
}

struct ErrOnYield;

impl TryMapExpr<CoreBlockPyExprWithYield, CoreBlockPyExpr, CoreBlockPyExprWithYield>
    for ErrOnYield
{
    fn try_map_expr(
        &self,
        expr: CoreBlockPyExprWithYield,
    ) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield> {
        match expr {
            CoreBlockPyExprWithYield::Yield(_) => Err(expr),
            CoreBlockPyExprWithYield::YieldFrom(_) => Err(expr),
            CoreBlockPyExprWithYield::Literal(node) => Ok(node.into()),
            CoreBlockPyExprWithYield::BinOp(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::UnaryOp(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::Call(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::GetAttr(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::SetAttr(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::GetItem(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::SetItem(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::DelItem(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::Load(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::Store(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::Del(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::MakeCell(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
            CoreBlockPyExprWithYield::CellRefForName(node) => Ok(node.into()),
            CoreBlockPyExprWithYield::CellRef(node) => Ok(node.into()),
            CoreBlockPyExprWithYield::MakeFunction(node) => Ok(node
                .try_map_typed_children(&mut |child| self.try_map_expr(child))?
                .into()),
        }
    }
}

pub(crate) fn try_lower_core_expr_without_yield(
    value: CoreBlockPyExprWithYield,
) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield> {
    ErrOnYield.try_map_expr(value)
}

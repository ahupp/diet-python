use super::*;
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};

pub(crate) trait BlockPyModuleMap<PIn, POut>: MapExpr<PIn::Expr, POut::Expr>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
{
    fn map_module(&mut self, module: BlockPyModule<PIn>) -> BlockPyModule<POut> {
        BlockPyModule {
            callable_defs: module
                .callable_defs
                .into_iter()
                .map(|function| self.map_fn(function))
                .collect(),
            module_constants: module.module_constants,
        }
    }

    fn map_fn(&mut self, func: BlockPyFunction<PIn>) -> BlockPyFunction<POut> {
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

    fn map_block(&mut self, block: Block<PIn::Expr>) -> Block<POut::Expr> {
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

    fn map_term(&mut self, term: BlockTerm<PIn::Expr>) -> BlockTerm<POut::Expr> {
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
}

pub(crate) trait BlockPyModuleTryMap<PIn, POut>:
    TryMapExpr<PIn::Expr, POut::Expr, Self::Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
{
    type Error;

    fn try_map_fn(
        &mut self,
        func: BlockPyFunction<PIn>,
    ) -> Result<BlockPyFunction<POut>, Self::Error> {
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

    fn try_map_block(&mut self, block: Block<PIn::Expr>) -> Result<Block<POut::Expr>, Self::Error> {
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
        &mut self,
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

pub(crate) struct ErrOnAwait;

impl
    TryMapExpr<
        CoreBlockPyExprWithAwaitAndYield,
        CoreBlockPyExprWithYield,
        CoreBlockPyExprWithAwaitAndYield,
    > for ErrOnAwait
{
    fn try_map_expr(
        &mut self,
        expr: CoreBlockPyExprWithAwaitAndYield,
    ) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield> {
        try_lower_core_expr_without_await_with_mapper(expr, self)
    }

    fn try_map_name(
        &mut self,
        name: UnresolvedName,
    ) -> Result<UnresolvedName, CoreBlockPyExprWithAwaitAndYield> {
        Ok(name)
    }
}

impl BlockPyModuleTryMap<CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield>
    for ErrOnAwait
{
    type Error = CoreBlockPyExprWithAwaitAndYield;
}

pub(crate) struct ErrOnYield;

impl TryMapExpr<CoreBlockPyExprWithYield, CoreBlockPyExpr, CoreBlockPyExprWithYield>
    for ErrOnYield
{
    fn try_map_expr(
        &mut self,
        expr: CoreBlockPyExprWithYield,
    ) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield> {
        try_lower_core_expr_without_yield_with_mapper(expr, self)
    }

    fn try_map_name(
        &mut self,
        name: UnresolvedName,
    ) -> Result<UnresolvedName, CoreBlockPyExprWithYield> {
        Ok(name)
    }
}

impl BlockPyModuleTryMap<CoreBlockPyPassWithYield, CoreBlockPyPass> for ErrOnYield {
    type Error = CoreBlockPyExprWithYield;
}

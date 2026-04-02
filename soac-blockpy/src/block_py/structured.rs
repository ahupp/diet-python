#[cfg(test)]
use super::{
    BlockPyCfgFragment, BlockPyFunction, BlockPyLabel, BlockPyModule, BlockPyStructuredPass,
    BlockPyTerm, MapExpr, PassBlock, PassExpr, PassStructuredFragment, PassStructuredStmt,
    PassTerm,
};
use super::{BlockPyStmt, BlockPyStmtFor, Instr, StructuredBlockPyStmt, StructuredBlockPyStmtFor};
use std::fmt;

pub(crate) trait IntoStructuredBlockPyStmt<I>: Clone + fmt::Debug
where
    I: Instr,
{
    fn into_structured_stmt(self) -> StructuredBlockPyStmtFor<I>;
}

impl<EIn, EOut, N> From<StructuredBlockPyStmt<EIn, N>> for BlockPyStmt<EOut, N>
where
    EOut: From<EIn>,
{
    fn from(value: StructuredBlockPyStmt<EIn, N>) -> Self {
        match value {
            StructuredBlockPyStmt::Expr(expr) => Self::Expr(expr.into()),
            StructuredBlockPyStmt::If(_) => {
                panic!("structured BlockPy If reached BlockPyStmt conversion")
            }
            StructuredBlockPyStmt::_Marker(_) => {
                unreachable!("structured stmt marker should not appear")
            }
        }
    }
}

impl<I> IntoStructuredBlockPyStmt<I> for BlockPyStmtFor<I>
where
    I: Instr,
{
    fn into_structured_stmt(self) -> StructuredBlockPyStmtFor<I> {
        match self {
            BlockPyStmt::Expr(expr) => StructuredBlockPyStmt::Expr(expr),
            BlockPyStmt::_Marker(_) => unreachable!("linear stmt marker should not appear"),
        }
    }
}

impl<I> IntoStructuredBlockPyStmt<I> for StructuredBlockPyStmtFor<I>
where
    I: Instr,
{
    fn into_structured_stmt(self) -> StructuredBlockPyStmtFor<I> {
        self
    }
}

#[cfg(test)]
pub(crate) trait BlockPyModuleVisitor<P>
where
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    fn visit_module(&mut self, module: &BlockPyModule<P>) {
        walk_module(self, module);
    }

    fn visit_fn(&mut self, func: &BlockPyFunction<P>) {
        walk_fn(self, func);
    }

    fn visit_block(&mut self, block: &PassBlock<P>) {
        walk_block(self, block);
    }

    fn visit_fragment(&mut self, fragment: &PassStructuredFragment<P>) {
        walk_fragment(self, fragment);
    }

    fn visit_stmt(&mut self, stmt: &PassStructuredStmt<P>) {
        walk_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &PassTerm<P>) {
        walk_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockPyLabel) {
        walk_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, expr: &PassExpr<P>) {
        walk_expr(self, expr);
    }
}

#[cfg(test)]
pub(crate) fn walk_module<V, P>(visitor: &mut V, module: &BlockPyModule<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

#[cfg(test)]
pub(crate) fn walk_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

#[cfg(test)]
pub(crate) fn walk_block<V, P>(visitor: &mut V, block: &PassBlock<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    for stmt in &block.body {
        let stmt = stmt.clone().into_structured_stmt();
        visitor.visit_stmt(&stmt);
    }
    if let Some(exc_edge) = &block.exc_edge {
        visitor.visit_label(&exc_edge.target);
    }
    visitor.visit_term(&block.term);
}

#[cfg(test)]
pub(crate) fn walk_fragment<V, P>(visitor: &mut V, fragment: &PassStructuredFragment<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    for stmt in &fragment.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(term) = &fragment.term {
        visitor.visit_term(term);
    }
}

#[cfg(test)]
pub(crate) fn walk_stmt<V, P>(visitor: &mut V, stmt: &PassStructuredStmt<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    match stmt {
        StructuredBlockPyStmt::Expr(expr) => visitor.visit_expr(expr),
        StructuredBlockPyStmt::If(if_stmt) => {
            visitor.visit_expr(&if_stmt.test);
            visitor.visit_fragment(&if_stmt.body);
            visitor.visit_fragment(&if_stmt.orelse);
        }
        StructuredBlockPyStmt::_Marker(_) => {
            unreachable!("structured stmt marker should not appear")
        }
    }
}

#[cfg(test)]
pub(crate) fn walk_label<V, P>(visitor: &mut V, label: &BlockPyLabel)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    let _ = visitor;
    let _ = label;
}

#[cfg(test)]
pub(crate) fn walk_term<V, P>(visitor: &mut V, term: &PassTerm<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    match term {
        BlockPyTerm::Jump(edge) => {
            visitor.visit_label(&edge.target);
        }
        BlockPyTerm::IfTerm(if_term) => {
            visitor.visit_expr(&if_term.test);
            visitor.visit_label(&if_term.then_label);
            visitor.visit_label(&if_term.else_label);
        }
        BlockPyTerm::BranchTable(branch) => {
            visitor.visit_expr(&branch.index);
            for target in &branch.targets {
                visitor.visit_label(target);
            }
            visitor.visit_label(&branch.default_label);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                visitor.visit_expr(exc);
            }
        }
        BlockPyTerm::Return(value) => visitor.visit_expr(value),
    }
}

#[cfg(test)]
pub(crate) fn walk_expr<V, P>(visitor: &mut V, expr: &PassExpr<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyStructuredPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    let _ = expr.clone().map_expr(&mut |child| {
        visitor.visit_expr(&child);
        child
    });
}

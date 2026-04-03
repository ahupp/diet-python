#[cfg(test)]
use super::{
    BlockPyCfgFragment, BlockPyFunction, BlockPyLabel, BlockPyModule, BlockPyPass, BlockPyTerm,
    CfgBlock, Walkable,
};
use super::{Instr, StructuredInstr};
use std::fmt;

pub(crate) trait IntoStructuredInstr<I>: Clone + fmt::Debug
where
    I: Instr,
{
    fn into_structured_instr(self) -> StructuredInstr<I>;
}

impl<I> IntoStructuredInstr<I> for I
where
    I: Instr,
{
    fn into_structured_instr(self) -> StructuredInstr<I> {
        StructuredInstr::Expr(self)
    }
}

impl<I> IntoStructuredInstr<I> for StructuredInstr<I>
where
    I: Instr,
{
    fn into_structured_instr(self) -> StructuredInstr<I> {
        self
    }
}

#[cfg(test)]
pub(crate) trait BlockPyModuleVisitor<P>
where
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    fn visit_module(&mut self, module: &BlockPyModule<P, StructuredInstr<P::Expr>>) {
        walk_module(self, module);
    }

    fn visit_fn(&mut self, func: &BlockPyFunction<P, StructuredInstr<P::Expr>>) {
        walk_fn(self, func);
    }

    fn visit_block(&mut self, block: &CfgBlock<StructuredInstr<P::Expr>, BlockPyTerm<P::Expr>>) {
        walk_block(self, block);
    }

    fn visit_fragment(
        &mut self,
        fragment: &BlockPyCfgFragment<StructuredInstr<P::Expr>, BlockPyTerm<P::Expr>>,
    ) {
        walk_fragment(self, fragment);
    }

    fn visit_stmt(&mut self, stmt: &StructuredInstr<P::Expr>) {
        walk_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockPyTerm<P::Expr>) {
        walk_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockPyLabel) {
        walk_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, expr: &P::Expr) {
        walk_expr(self, expr);
    }
}

#[cfg(test)]
pub(crate) fn walk_module<V, P>(
    visitor: &mut V,
    module: &BlockPyModule<P, StructuredInstr<P::Expr>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

#[cfg(test)]
pub(crate) fn walk_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P, StructuredInstr<P::Expr>>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

#[cfg(test)]
pub(crate) fn walk_block<V, P>(
    visitor: &mut V,
    block: &CfgBlock<StructuredInstr<P::Expr>, BlockPyTerm<P::Expr>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for stmt in &block.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(exc_edge) = &block.exc_edge {
        visitor.visit_label(&exc_edge.target);
    }
    visitor.visit_term(&block.term);
}

#[cfg(test)]
pub(crate) fn walk_fragment<V, P>(
    visitor: &mut V,
    fragment: &BlockPyCfgFragment<StructuredInstr<P::Expr>, BlockPyTerm<P::Expr>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for stmt in &fragment.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(term) = &fragment.term {
        visitor.visit_term(term);
    }
}

#[cfg(test)]
pub(crate) fn walk_stmt<V, P>(visitor: &mut V, stmt: &StructuredInstr<P::Expr>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    match stmt {
        StructuredInstr::Expr(expr) => visitor.visit_expr(expr),
        StructuredInstr::If(if_stmt) => {
            visitor.visit_expr(&if_stmt.test);
            visitor.visit_fragment(&if_stmt.body);
            visitor.visit_fragment(&if_stmt.orelse);
        }
    }
}

#[cfg(test)]
pub(crate) fn walk_label<V, P>(visitor: &mut V, label: &BlockPyLabel)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    let _ = visitor;
    let _ = label;
}

#[cfg(test)]
pub(crate) fn walk_term<V, P>(visitor: &mut V, term: &BlockPyTerm<P::Expr>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
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
pub(crate) fn walk_expr<V, P>(visitor: &mut V, expr: &P::Expr)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    let _ = expr.clone().walk_map(&mut |child| {
        visitor.visit_expr(&child);
        child
    });
}

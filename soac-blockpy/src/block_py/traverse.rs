use super::*;

pub(crate) fn instr_any<I, F>(instr: &I, mut predicate: F) -> bool
where
    I: Instr + Walkable<I>,
    F: FnMut(&I) -> bool,
{
    fn instr_any_impl<I, F>(instr: &I, predicate: &mut F) -> bool
    where
        I: Instr + Walkable<I>,
        F: FnMut(&I) -> bool,
    {
        if predicate(instr) {
            return true;
        }

        let mut found = false;
        instr.walk(&mut |child| {
            if !found && instr_any_impl(child, predicate) {
                found = true;
            }
        });
        found
    }

    instr_any_impl(instr, &mut predicate)
}

pub(crate) fn map_module<PIn, POut, M>(
    map: &mut M,
    module: BlockPyModule<PIn>,
) -> BlockPyModule<POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    M: MapExpr<PIn::Expr, POut::Expr>,
{
    BlockPyModule {
        module_name_gen: module.module_name_gen,
        global_names: module.global_names,
        callable_defs: module
            .callable_defs
            .into_iter()
            .map(|function| map_fn(map, function))
            .collect(),
        module_constants: module.module_constants,
        counter_defs: module.counter_defs,
    }
}

pub(crate) fn map_fn<PIn, POut, M>(map: &mut M, func: BlockPyFunction<PIn>) -> BlockPyFunction<POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    M: MapExpr<PIn::Expr, POut::Expr>,
{
    BlockPyFunction {
        function_id: func.function_id,
        name_gen: func.name_gen,
        names: func.names,
        kind: func.kind,
        params: func.params,
        blocks: func
            .blocks
            .into_iter()
            .map(|block| map_block(map, block))
            .collect(),
        doc: func.doc,
        storage_layout: func.storage_layout,
        scope: func.scope,
    }
}

pub(crate) fn map_block<In, Out, M>(map: &mut M, block: Block<In>) -> Block<Out>
where
    In: Instr,
    Out: Instr,
    M: MapExpr<In, Out>,
{
    Block {
        label: block.label,
        body: block
            .body
            .into_iter()
            .map(|stmt| map.map_expr(stmt))
            .collect(),
        term: map_term(map, block.term),
        params: block.params,
        exc_edge: block.exc_edge,
    }
}

pub(crate) fn map_term<In, Out, M>(map: &mut M, term: BlockTerm<In>) -> BlockTerm<Out>
where
    In: Instr,
    Out: Instr,
    M: MapExpr<In, Out>,
{
    match term {
        BlockTerm::Jump(edge) => BlockTerm::Jump(BlockEdge {
            target: edge.target,
            args: edge.args,
        }),
        BlockTerm::IfTerm(if_term) => BlockTerm::IfTerm(TermIf {
            test: map.map_expr(if_term.test),
            then_label: if_term.then_label,
            else_label: if_term.else_label,
        }),
        BlockTerm::BranchTable(branch) => BlockTerm::BranchTable(TermBranchTable {
            index: map.map_expr(branch.index),
            targets: branch.targets,
            default_label: branch.default_label,
        }),
        BlockTerm::Raise(raise_stmt) => BlockTerm::Raise(TermRaise {
            exc: raise_stmt.exc.map(|exc| map.map_expr(exc)),
        }),
        BlockTerm::Return(value) => BlockTerm::Return(map.map_expr(value)),
    }
}

pub(crate) fn try_map_fn<PIn, POut, Error, M>(
    map: &mut M,
    func: BlockPyFunction<PIn>,
) -> Result<BlockPyFunction<POut>, Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    M: TryMapExpr<PIn::Expr, POut::Expr, Error>,
{
    Ok(BlockPyFunction {
        function_id: func.function_id,
        name_gen: func.name_gen,
        names: func.names,
        kind: func.kind,
        params: func.params,
        blocks: func
            .blocks
            .into_iter()
            .map(|block| try_map_block(map, block))
            .collect::<Result<_, _>>()?,
        doc: func.doc,
        storage_layout: func.storage_layout,
        scope: func.scope,
    })
}

pub(crate) fn try_map_block<In, Out, Error, M>(
    map: &mut M,
    block: Block<In>,
) -> Result<Block<Out>, Error>
where
    In: Instr,
    Out: Instr,
    M: TryMapExpr<In, Out, Error>,
{
    Ok(Block {
        label: block.label,
        body: block
            .body
            .into_iter()
            .map(|stmt| map.try_map_expr(stmt))
            .collect::<Result<_, _>>()?,
        term: try_map_term(map, block.term)?,
        params: block.params,
        exc_edge: block.exc_edge,
    })
}

pub(crate) fn try_map_term<In, Out, Error, M>(
    map: &mut M,
    term: BlockTerm<In>,
) -> Result<BlockTerm<Out>, Error>
where
    In: Instr,
    Out: Instr,
    M: TryMapExpr<In, Out, Error>,
{
    match term {
        BlockTerm::Jump(edge) => Ok(BlockTerm::Jump(BlockEdge {
            target: edge.target,
            args: edge.args,
        })),
        BlockTerm::IfTerm(if_term) => Ok(BlockTerm::IfTerm(TermIf {
            test: map.try_map_expr(if_term.test)?,
            then_label: if_term.then_label,
            else_label: if_term.else_label,
        })),
        BlockTerm::BranchTable(branch) => Ok(BlockTerm::BranchTable(TermBranchTable {
            index: map.try_map_expr(branch.index)?,
            targets: branch.targets,
            default_label: branch.default_label,
        })),
        BlockTerm::Raise(raise_stmt) => Ok(BlockTerm::Raise(TermRaise {
            exc: raise_stmt
                .exc
                .map(|exc| map.try_map_expr(exc))
                .transpose()?,
        })),
        BlockTerm::Return(value) => Ok(BlockTerm::Return(map.try_map_expr(value)?)),
    }
}

enum TermChildRef<'a, E> {
    Expr(&'a E),
    Label(&'a BlockLabel),
}

fn walk_term_children<I: Instr>(
    term: &BlockTerm<I>,
    visit_child: &mut impl FnMut(TermChildRef<'_, I>),
) {
    match term {
        BlockTerm::Jump(edge) => {
            visit_child(TermChildRef::Label(&edge.target));
        }
        BlockTerm::IfTerm(if_term) => {
            visit_child(TermChildRef::Expr(&if_term.test));
            visit_child(TermChildRef::Label(&if_term.then_label));
            visit_child(TermChildRef::Label(&if_term.else_label));
        }
        BlockTerm::BranchTable(branch) => {
            visit_child(TermChildRef::Expr(&branch.index));
            for target in &branch.targets {
                visit_child(TermChildRef::Label(target));
            }
            visit_child(TermChildRef::Label(&branch.default_label));
        }
        BlockTerm::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                visit_child(TermChildRef::Expr(exc));
            }
        }
        BlockTerm::Return(value) => visit_child(TermChildRef::Expr(value)),
    }
}

fn walk_expr_children<E>(expr: &E, visit_expr: &mut impl FnMut(&E))
where
    E: Walkable<E>,
{
    expr.walk(visit_expr);
}

pub(crate) trait BlockPyLinearModuleVisitor<P>
where
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    fn visit_fn(&mut self, func: &BlockPyFunction<P>) {
        walk_linear_fn(self, func);
    }

    fn visit_block(&mut self, block: &Block<P::Expr, P::Expr>) {
        walk_linear_block(self, block);
    }

    fn visit_stmt(&mut self, stmt: &P::Expr) {
        walk_linear_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockTerm<P::Expr>) {
        walk_linear_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockLabel) {
        walk_linear_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, expr: &P::Expr) {
        walk_linear_expr(self, expr);
    }
}

pub(crate) fn walk_linear_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

pub(crate) fn walk_linear_block<V, P>(visitor: &mut V, block: &Block<P::Expr, P::Expr>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
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

pub(crate) fn walk_linear_stmt<V, P>(visitor: &mut V, stmt: &P::Expr)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    visitor.visit_expr(stmt);
}

pub(crate) fn walk_linear_label<V, P>(visitor: &mut V, label: &BlockLabel)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    let _ = visitor;
    let _ = label;
}

pub(crate) fn walk_linear_term<V, P>(visitor: &mut V, term: &BlockTerm<P::Expr>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    walk_term_children(term, &mut |child| match child {
        TermChildRef::Expr(expr) => visitor.visit_expr(expr),
        TermChildRef::Label(label) => visitor.visit_label(label),
    });
}

pub(crate) fn walk_linear_expr<V, P>(visitor: &mut V, expr: &P::Expr)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    walk_expr_children(expr, &mut |child| visitor.visit_expr(child));
}

#[cfg(test)]
pub(crate) trait BlockPyModuleVisitor<P>
where
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    fn visit_fn(&mut self, func: &BlockPyFunction<P, StructuredInstr<P::Expr>>) {
        walk_fn(self, func);
    }

    fn visit_block(&mut self, block: &Block<StructuredInstr<P::Expr>, P::Expr>) {
        walk_block(self, block);
    }

    fn visit_fragment(
        &mut self,
        fragment: &BlockBuilder<StructuredInstr<P::Expr>, BlockTerm<P::Expr>>,
    ) {
        walk_fragment(self, fragment);
    }

    fn visit_stmt(&mut self, stmt: &StructuredInstr<P::Expr>) {
        walk_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockTerm<P::Expr>) {
        walk_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockLabel) {
        walk_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, expr: &P::Expr) {
        walk_expr(self, expr);
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
pub(crate) fn walk_block<V, P>(visitor: &mut V, block: &Block<StructuredInstr<P::Expr>, P::Expr>)
where
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
    fragment: &BlockBuilder<StructuredInstr<P::Expr>, BlockTerm<P::Expr>>,
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
pub(crate) fn walk_label<V, P>(visitor: &mut V, label: &BlockLabel)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    let _ = visitor;
    let _ = label;
}

#[cfg(test)]
pub(crate) fn walk_term<V, P>(visitor: &mut V, term: &BlockTerm<P::Expr>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    walk_term_children(term, &mut |child| match child {
        TermChildRef::Expr(expr) => visitor.visit_expr(expr),
        TermChildRef::Label(label) => visitor.visit_label(label),
    });
}

#[cfg(test)]
pub(crate) fn walk_expr<V, P>(visitor: &mut V, expr: &P::Expr)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    walk_expr_children(expr, &mut |child| visitor.visit_expr(child));
}

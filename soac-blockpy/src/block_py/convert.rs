use super::*;

pub(crate) fn instr_any<I, F>(instr: &I, mut predicate: F) -> bool
where
    I: Instr,
    F: FnMut(&I) -> bool,
{
    fn instr_any_impl<I, F>(instr: &I, predicate: &mut F) -> bool
    where
        I: Instr,
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

pub(crate) fn try_map_module<PIn, POut, Error, M>(
    map: &mut M,
    module: BlockPyModule<PIn>,
) -> Result<BlockPyModule<POut>, Error>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    M: TryMapExpr<PIn::Expr, POut::Expr, Error>,
{
    Ok(BlockPyModule {
        module_name_gen: module.module_name_gen,
        callable_defs: module
            .callable_defs
            .into_iter()
            .map(|function| try_map_fn(map, function))
            .collect::<Result<_, _>>()?,
        module_constants: module.module_constants,
        counter_defs: module.counter_defs,
    })
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

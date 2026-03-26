use super::{
    BlockArg, BlockPyAssign, BlockPyBranchTable, BlockPyCfgFragment, BlockPyDelete, BlockPyIf,
    BlockPyIfTerm, BlockPyNameLike, BlockPyRaise, BlockPyStmt, BlockPyTerm, CfgBlock,
    IntoBlockPyStmt,
};
use crate::passes::ast_symbol_analysis::{collect_assigned_names, load_names_in_expr};
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::Expr;
use std::collections::{HashMap, HashSet};

pub(crate) fn compute_block_params_blockpy<S, E, N>(
    blocks: &[CfgBlock<S, BlockPyTerm<E>>],
    state_order: &[String],
    extra_successors: &HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>>
where
    S: IntoBlockPyStmt<E, N>,
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let label_to_index: HashMap<&str, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.as_str(), idx))
        .collect();
    let analyses: Vec<(HashSet<String>, HashSet<String>)> =
        blocks.iter().map(analyze_blockpy_use_def).collect();
    let mut live_in: Vec<HashSet<String>> = vec![HashSet::new(); blocks.len()];
    let mut live_out: Vec<HashSet<String>> = vec![HashSet::new(); blocks.len()];

    let mut changed = true;
    while changed {
        changed = false;
        for (idx, block) in blocks.iter().enumerate().rev() {
            let mut out = HashSet::new();
            match &block.term {
                BlockPyTerm::Jump(target) => {
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        target.as_str(),
                        &target.args,
                    );
                }
                BlockPyTerm::IfTerm(BlockPyIfTerm {
                    then_label,
                    else_label,
                    ..
                }) => {
                    let no_args = &[] as &[BlockArg];
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        then_label.as_str(),
                        no_args,
                    );
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        else_label.as_str(),
                        no_args,
                    );
                }
                BlockPyTerm::BranchTable(BlockPyBranchTable {
                    targets,
                    default_label,
                    ..
                }) => {
                    let no_args = &[] as &[BlockArg];
                    for target in targets {
                        extend_successor_live_in(
                            &mut out,
                            blocks,
                            &label_to_index,
                            &live_in,
                            target.as_str(),
                            no_args,
                        );
                    }
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        default_label.as_str(),
                        no_args,
                    );
                }
                BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => {}
            }
            if let Some(extra) = extra_successors.get(block.label.as_str()) {
                for succ in extra {
                    if let Some(succ_idx) = label_to_index.get(succ.as_str()) {
                        out.extend(live_in[*succ_idx].iter().cloned());
                    }
                }
            }
            let (uses, defs) = &analyses[idx];
            let mut incoming = uses.clone();
            for name in &out {
                if !defs.contains(name) {
                    incoming.insert(name.clone());
                }
            }
            if incoming != live_in[idx] || out != live_out[idx] {
                changed = true;
                live_in[idx] = incoming;
                live_out[idx] = out;
            }
        }
    }

    let mut params = HashMap::new();
    for (idx, block) in blocks.iter().enumerate() {
        let ordered = state_order
            .iter()
            .filter(|name| live_in[idx].contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        params.insert(block.label.as_str().to_string(), ordered);
    }
    params
}

pub(crate) fn merge_declared_block_params<S, T>(
    blocks: &[CfgBlock<S, T>],
    block_params: &mut HashMap<String, Vec<String>>,
) {
    for block in blocks {
        let params = block_params
            .entry(block.label.as_str().to_string())
            .or_default();
        for param_name in block
            .exception_param()
            .into_iter()
            .chain(block.param_names())
        {
            if !params.iter().any(|existing| existing == param_name) {
                params.push(param_name.to_string());
            }
        }
    }
}

pub(crate) fn extend_state_order_with_declared_block_params<S, T>(
    blocks: &[CfgBlock<S, T>],
    state_order: &mut Vec<String>,
) {
    for block in blocks {
        for param_name in block
            .exception_param()
            .into_iter()
            .chain(block.param_names())
        {
            if !state_order.iter().any(|existing| existing == param_name) {
                state_order.push(param_name.to_string());
            }
        }
    }
}

pub(crate) fn analyze_blockpy_use_def<S, E, N>(
    block: &CfgBlock<S, BlockPyTerm<E>>,
) -> (HashSet<String>, HashSet<String>)
where
    S: IntoBlockPyStmt<E, N>,
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();

    if let Some(exc_param) = block.exception_param() {
        uses.insert(exc_param.to_string());
    }

    for stmt in &block.body {
        let stmt = stmt.clone().into_stmt();
        for name in load_names_in_blockpy_stmt(&stmt) {
            if !defs.contains(name.as_str()) {
                uses.insert(name);
            }
        }
        for name in assigned_names_in_blockpy_stmt(&stmt) {
            defs.insert(name);
        }
    }
    for name in load_names_in_blockpy_term(&block.term) {
        if !defs.contains(name.as_str()) {
            uses.insert(name);
        }
    }
    for name in assigned_names_in_blockpy_term(&block.term) {
        defs.insert(name);
    }

    (uses, defs)
}

pub(crate) fn loaded_names_in_blockpy_block<S, E, N>(
    block: &CfgBlock<S, BlockPyTerm<E>>,
) -> HashSet<String>
where
    S: IntoBlockPyStmt<E, N>,
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut names = HashSet::new();
    if let Some(exc_param) = block.exception_param() {
        names.insert(exc_param.to_string());
    }
    for stmt in &block.body {
        let stmt = stmt.clone().into_stmt();
        names.extend(load_names_in_blockpy_stmt(&stmt));
    }
    names.extend(load_names_in_blockpy_term(&block.term));
    names
}

fn extend_successor_live_in<S, T>(
    out: &mut HashSet<String>,
    blocks: &[CfgBlock<S, T>],
    label_to_index: &HashMap<&str, usize>,
    live_in: &[HashSet<String>],
    target_label: &str,
    edge_args: &[BlockArg],
) {
    let Some(succ_idx) = label_to_index.get(target_label).copied() else {
        return;
    };
    let succ_block = &blocks[succ_idx];
    let declared_param_names = succ_block
        .exception_param()
        .into_iter()
        .chain(succ_block.param_names())
        .collect::<Vec<_>>();
    let explicit_start = declared_param_names.len().saturating_sub(edge_args.len());
    let explicit_param_names = declared_param_names[explicit_start..]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    for name in &live_in[succ_idx] {
        if !explicit_param_names.contains(name.as_str()) {
            out.insert(name.clone());
        }
    }
    out.extend(load_names_in_blockpy_edge_args(edge_args));
}

fn load_names_in_blockpy_edge_args(args: &[BlockArg]) -> HashSet<String> {
    let mut names = HashSet::new();
    for arg in args {
        match arg {
            BlockArg::Name(name) => {
                names.insert(name.clone());
            }
            BlockArg::None | BlockArg::CurrentException | BlockArg::AbruptKind(_) => {}
        }
    }
    names
}

fn load_names_in_blockpy_expr<E>(expr: &E) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    load_names_in_expr(&expr.clone().into())
}

fn load_names_in_blockpy_stmt<E, N>(stmt: &BlockPyStmt<E, N>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    match stmt {
        BlockPyStmt::Assign(BlockPyAssign { value, .. }) => load_names_in_blockpy_expr(value),
        BlockPyStmt::Expr(expr) => load_names_in_blockpy_expr(expr),
        BlockPyStmt::Delete(BlockPyDelete { target }) => {
            load_names_in_expr(&Expr::Name(target.clone().into()))
        }
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            let mut names = load_names_in_blockpy_expr(test);
            names.extend(load_names_in_blockpy_stmt_fragment(body));
            names.extend(load_names_in_blockpy_stmt_fragment(orelse));
            names
        }
    }
}

fn load_names_in_blockpy_term<E>(term: &BlockPyTerm<E>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    match term {
        BlockPyTerm::Jump(target) => load_names_in_blockpy_edge_args(&target.args),
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => load_names_in_blockpy_expr(test),
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            load_names_in_blockpy_expr(index)
        }
        BlockPyTerm::Return(value) => load_names_in_blockpy_expr(value),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => exc
            .as_ref()
            .map(load_names_in_blockpy_expr)
            .unwrap_or_default(),
    }
}

pub(super) fn assigned_names_in_blockpy_stmt<E, N>(stmt: &BlockPyStmt<E, N>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    match stmt {
        BlockPyStmt::Assign(BlockPyAssign { target, value }) => {
            let mut names = HashSet::from([target.id_str().to_string()]);
            collect_named_expr_target_names_in_blockpy_expr(value, &mut names);
            names
        }
        BlockPyStmt::Expr(expr) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(expr, &mut names);
            names
        }
        BlockPyStmt::Delete(_) => HashSet::new(),
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(test, &mut names);
            names.extend(assigned_names_in_blockpy_fragment(body));
            names.extend(assigned_names_in_blockpy_fragment(orelse));
            names
        }
    }
}

pub(super) fn assigned_names_in_blockpy_stmts<E, N>(stmts: &[BlockPyStmt<E, N>]) -> HashSet<String>
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut out = HashSet::new();
    for stmt in stmts {
        out.extend(assigned_names_in_blockpy_stmt(stmt));
    }
    out
}

pub(super) fn assigned_names_in_blockpy_term<E>(term: &BlockPyTerm<E>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    match term {
        BlockPyTerm::Jump(_) => HashSet::new(),
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(test, &mut names);
            names
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(index, &mut names);
            names
        }
        BlockPyTerm::Return(value) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(value, &mut names);
            names
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            let mut names = HashSet::new();
            if let Some(exc) = exc {
                collect_named_expr_target_names_in_blockpy_expr(exc, &mut names);
            }
            names
        }
    }
}

pub(super) fn assigned_names_in_blockpy_fragment<E, N>(
    fragment: &BlockPyCfgFragment<BlockPyStmt<E, N>, BlockPyTerm<E>>,
) -> HashSet<String>
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut out = assigned_names_in_blockpy_stmts(&fragment.body);
    if let Some(term) = &fragment.term {
        out.extend(assigned_names_in_blockpy_term(term));
    }
    out
}

fn load_names_in_blockpy_stmt_list<E, N>(stmts: &[BlockPyStmt<E, N>]) -> HashSet<String>
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut out = HashSet::new();
    for stmt in stmts {
        out.extend(load_names_in_blockpy_stmt(stmt));
    }
    out
}

fn load_names_in_blockpy_stmt_fragment<E, N>(
    fragment: &BlockPyCfgFragment<BlockPyStmt<E, N>, BlockPyTerm<E>>,
) -> HashSet<String>
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut out = load_names_in_blockpy_stmt_list(&fragment.body);
    if let Some(term) = &fragment.term {
        out.extend(load_names_in_blockpy_term(term));
    }
    out
}

fn collect_named_expr_target_names_in_blockpy_expr<E>(expr: &E, names: &mut HashSet<String>)
where
    E: Clone + Into<Expr>,
{
    collect_named_expr_target_names_in_expr(&expr.clone().into(), names);
}

fn collect_named_expr_target_names_in_expr(expr: &Expr, names: &mut HashSet<String>) {
    #[derive(Default)]
    struct NamedExprTargetCollector {
        names: HashSet<String>,
    }

    impl Transformer for NamedExprTargetCollector {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if let Expr::Named(named) = expr {
                collect_assigned_names(named.target.as_ref(), &mut self.names);
                self.visit_expr(named.value.as_mut());
                return;
            }
            walk_expr(self, expr);
        }
    }

    let mut expr = expr.clone();
    let mut collector = NamedExprTargetCollector::default();
    collector.visit_expr(&mut expr);
    names.extend(collector.names);
}

#[cfg(test)]
mod test;

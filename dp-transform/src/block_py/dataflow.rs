use super::{
    BlockArg, BlockPyAssign, BlockPyBlock, BlockPyBranchTable, BlockPyCfgFragment, BlockPyDelete,
    BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmt, BlockPyTerm,
};
use crate::passes::ast_symbol_analysis::{collect_assigned_names, load_names_in_expr};
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::Expr;
use std::collections::{HashMap, HashSet};

pub(crate) fn compute_block_params_blockpy<E>(
    blocks: &[BlockPyBlock<E>],
    state_order: &[String],
    extra_successors: &HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>>
where
    E: Clone + Into<Expr>,
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
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        then_label.as_str(),
                        &[],
                    );
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        else_label.as_str(),
                        &[],
                    );
                }
                BlockPyTerm::BranchTable(BlockPyBranchTable {
                    targets,
                    default_label,
                    ..
                }) => {
                    for target in targets {
                        extend_successor_live_in(
                            &mut out,
                            blocks,
                            &label_to_index,
                            &live_in,
                            target.as_str(),
                            &[],
                        );
                    }
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        default_label.as_str(),
                        &[],
                    );
                }
                BlockPyTerm::TryJump(try_jump) => {
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        try_jump.body_label.as_str(),
                        &[],
                    );
                    extend_successor_live_in(
                        &mut out,
                        blocks,
                        &label_to_index,
                        &live_in,
                        try_jump.except_label.as_str(),
                        &[],
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

pub(crate) fn merge_declared_block_params<E>(
    blocks: &[BlockPyBlock<E>],
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

pub(crate) fn extend_state_order_with_declared_block_params<E>(
    blocks: &[BlockPyBlock<E>],
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

pub(crate) fn analyze_blockpy_use_def<E>(
    block: &BlockPyBlock<E>,
) -> (HashSet<String>, HashSet<String>)
where
    E: Clone + Into<Expr>,
{
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();

    if let Some(exc_param) = block.exception_param() {
        uses.insert(exc_param.to_string());
    }

    for stmt in &block.body {
        for name in load_names_in_blockpy_stmt(stmt) {
            if !defs.contains(name.as_str()) {
                uses.insert(name);
            }
        }
        for name in assigned_names_in_blockpy_stmt(stmt) {
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

fn extend_successor_live_in<E>(
    out: &mut HashSet<String>,
    blocks: &[BlockPyBlock<E>],
    label_to_index: &HashMap<&str, usize>,
    live_in: &[HashSet<String>],
    target_label: &str,
    edge_args: &[BlockArg<E>],
) where
    E: Clone + Into<Expr>,
{
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

fn load_names_in_blockpy_edge_args<E>(args: &[BlockArg<E>]) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    let mut names = HashSet::new();
    for arg in args {
        match arg {
            BlockArg::Name(name) => {
                names.insert(name.clone());
            }
            BlockArg::Expr(expr) => {
                names.extend(load_names_in_blockpy_expr(expr));
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

fn load_names_in_blockpy_stmt<E>(stmt: &BlockPyStmt<E>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    match stmt {
        BlockPyStmt::Assign(BlockPyAssign { value, .. }) => load_names_in_blockpy_expr(value),
        BlockPyStmt::Expr(expr) => load_names_in_blockpy_expr(expr),
        BlockPyStmt::Delete(BlockPyDelete { target }) => {
            load_names_in_expr(&Expr::Name(target.clone()))
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
        BlockPyTerm::TryJump(_) => HashSet::new(),
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => load_names_in_blockpy_expr(test),
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            load_names_in_blockpy_expr(index)
        }
        BlockPyTerm::Return(value) => value
            .as_ref()
            .map(load_names_in_blockpy_expr)
            .unwrap_or_default(),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => exc
            .as_ref()
            .map(load_names_in_blockpy_expr)
            .unwrap_or_default(),
    }
}

fn assigned_names_in_blockpy_stmt<E>(stmt: &BlockPyStmt<E>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    match stmt {
        BlockPyStmt::Assign(BlockPyAssign { target, value }) => {
            let mut names = HashSet::from([target.id.to_string()]);
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
            names.extend(assigned_names_in_blockpy_stmt_fragment(body));
            names.extend(assigned_names_in_blockpy_stmt_fragment(orelse));
            names
        }
    }
}

fn assigned_names_in_blockpy_term<E>(term: &BlockPyTerm<E>) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => HashSet::new(),
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
            if let Some(value) = value {
                collect_named_expr_target_names_in_blockpy_expr(value, &mut names);
            }
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

fn load_names_in_blockpy_stmt_list<E>(stmts: &[BlockPyStmt<E>]) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    let mut out = HashSet::new();
    for stmt in stmts {
        out.extend(load_names_in_blockpy_stmt(stmt));
    }
    out
}

fn load_names_in_blockpy_stmt_fragment<E>(
    fragment: &BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>,
) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    let mut out = load_names_in_blockpy_stmt_list(&fragment.body);
    if let Some(term) = &fragment.term {
        out.extend(load_names_in_blockpy_term(term));
    }
    out
}

fn assigned_names_in_blockpy_stmt_fragment<E>(
    fragment: &BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>,
) -> HashSet<String>
where
    E: Clone + Into<Expr>,
{
    let mut out = HashSet::new();
    for stmt in &fragment.body {
        out.extend(assigned_names_in_blockpy_stmt(stmt));
    }
    if let Some(term) = &fragment.term {
        out.extend(assigned_names_in_blockpy_term(term));
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

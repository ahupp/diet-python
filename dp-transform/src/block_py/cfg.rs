use super::{
    BlockParam, BlockParamRole, BlockPyBlock, BlockPyCfgFragment, BlockPyIfTerm, BlockPyLabel,
    BlockPyStmt, BlockPyTerm,
};
use crate::passes::ast_symbol_analysis::collect_assigned_names;
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::Expr;
use std::collections::{HashMap, HashSet};

struct LabelNameRenamer<'a> {
    rename: &'a HashMap<String, String>,
}

impl Transformer for LabelNameRenamer<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(name) = expr {
            if let Some(rewritten) = self.rename.get(name.id.as_str()) {
                name.id = rewritten.as_str().into();
            }
        }
        walk_expr(self, expr);
    }
}

fn rename_blockpy_stmt(
    stmt: &mut BlockPyStmt<Expr>,
    body_renamer: &mut LabelNameRenamer<'_>,
    rename: &HashMap<String, String>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            if let Some(rewritten) = rename.get(assign.target.id.as_str()) {
                assign.target.id = rewritten.as_str().into();
            }
            body_renamer.visit_expr(&mut assign.value);
        }
        BlockPyStmt::Expr(expr) => body_renamer.visit_expr(expr),
        BlockPyStmt::Delete(delete) => {
            if let Some(rewritten) = rename.get(delete.target.id.as_str()) {
                delete.target.id = rewritten.as_str().into();
            }
        }
        BlockPyStmt::If(if_stmt) => {
            body_renamer.visit_expr(&mut if_stmt.test);
            rename_blockpy_stmt_fragment(&mut if_stmt.body, body_renamer, rename);
            rename_blockpy_stmt_fragment(&mut if_stmt.orelse, body_renamer, rename);
        }
    }
}

fn rename_blockpy_stmt_fragment(
    fragment: &mut BlockPyCfgFragment<BlockPyStmt<Expr>, BlockPyTerm<Expr>>,
    body_renamer: &mut LabelNameRenamer<'_>,
    rename: &HashMap<String, String>,
) {
    for stmt in &mut fragment.body {
        rename_blockpy_stmt(stmt, body_renamer, rename);
    }
    if let Some(term) = &mut fragment.term {
        rename_blockpy_term(term, body_renamer, rename);
    }
}

fn rename_blockpy_term(
    term: &mut BlockPyTerm<Expr>,
    body_renamer: &mut LabelNameRenamer<'_>,
    rename: &HashMap<String, String>,
) {
    fn rename_target_label(label: &mut BlockPyLabel, rename: &HashMap<String, String>) {
        if let Some(rewritten) = rename.get(label.as_str()) {
            *label = BlockPyLabel::from(rewritten.clone());
        }
    }

    match term {
        BlockPyTerm::Jump(target) => rename_target_label(&mut target.target, rename),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => {
            body_renamer.visit_expr(test);
            rename_target_label(then_label, rename);
            rename_target_label(else_label, rename);
        }
        BlockPyTerm::BranchTable(branch) => {
            body_renamer.visit_expr(&mut branch.index);
            for target in &mut branch.targets {
                rename_target_label(target, rename);
            }
            rename_target_label(&mut branch.default_label, rename);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                body_renamer.visit_expr(exc);
            }
        }
        BlockPyTerm::TryJump(try_jump) => {
            rename_target_label(&mut try_jump.body_label, rename);
            rename_target_label(&mut try_jump.except_label, rename);
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value {
                body_renamer.visit_expr(value);
            }
        }
    }
}

fn rename_blockpy_block(
    block: &mut BlockPyBlock<Expr>,
    body_renamer: &mut LabelNameRenamer<'_>,
    rename: &HashMap<String, String>,
) {
    let new_label = rename
        .get(block.label.as_str())
        .cloned()
        .unwrap_or_else(|| block.label.as_str().to_string());
    block.label = BlockPyLabel::from(new_label);
    for stmt in &mut block.body {
        rename_blockpy_stmt(stmt, body_renamer, rename);
    }
    rename_blockpy_term(&mut block.term, body_renamer, rename);
}

fn blockpy_successors<E>(block: &BlockPyBlock<E>) -> Vec<String> {
    match &block.term {
        BlockPyTerm::Jump(target) => vec![target.as_str().to_string()],
        BlockPyTerm::IfTerm(if_term) => vec![
            if_term.then_label.as_str().to_string(),
            if_term.else_label.as_str().to_string(),
        ],
        BlockPyTerm::BranchTable(branch) => {
            let mut out = branch
                .targets
                .iter()
                .map(|label| label.as_str().to_string())
                .collect::<Vec<_>>();
            out.push(branch.default_label.as_str().to_string());
            out
        }
        BlockPyTerm::TryJump(try_jump) => vec![
            try_jump.body_label.as_str().to_string(),
            try_jump.except_label.as_str().to_string(),
        ],
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => Vec::new(),
    }
}

pub(crate) fn rename_blockpy_labels(
    rename: &HashMap<String, String>,
    blocks: &mut [BlockPyBlock<Expr>],
) {
    for block in blocks.iter_mut() {
        let mut body_renamer = LabelNameRenamer { rename };
        rename_blockpy_block(block, &mut body_renamer, rename);
    }
}

fn apply_label_rename_blockpy(
    entry_label: &str,
    rename: &HashMap<String, String>,
    blocks: &mut [BlockPyBlock<Expr>],
) -> String {
    rename_blockpy_labels(rename, blocks);
    rename
        .get(entry_label)
        .cloned()
        .unwrap_or_else(|| entry_label.to_string())
}

fn fresh_linearized_if_label(
    base: &BlockPyLabel,
    counter: &mut usize,
    suffix: &str,
) -> BlockPyLabel {
    let label = BlockPyLabel::from(format!("{}_{}_{}", base.as_str(), suffix, *counter));
    *counter += 1;
    label
}

fn collect_named_expr_targets(expr: &Expr, names: &mut HashSet<String>) {
    #[derive(Default)]
    struct NamedExprTargetCollector {
        names: HashSet<String>,
    }

    impl Transformer for NamedExprTargetCollector {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if let Expr::Named(ruff_python_ast::ExprNamed { target, value, .. }) = expr {
                collect_assigned_names(target.as_ref(), &mut self.names);
                self.visit_expr(value.as_mut());
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

fn collect_assigned_names_in_blockpy_expr<E>(expr: &E, names: &mut HashSet<String>)
where
    E: Clone + Into<Expr>,
{
    collect_named_expr_targets(&expr.clone().into(), names);
}

fn collect_assigned_names_in_blockpy_term<E>(term: &BlockPyTerm<E>, names: &mut HashSet<String>)
where
    E: Clone + Into<Expr>,
{
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
        BlockPyTerm::IfTerm(if_term) => {
            collect_assigned_names_in_blockpy_expr(&if_term.test, names)
        }
        BlockPyTerm::BranchTable(branch_table) => {
            collect_assigned_names_in_blockpy_expr(&branch_table.index, names);
        }
        BlockPyTerm::Raise(raise) => {
            if let Some(exc) = &raise.exc {
                collect_assigned_names_in_blockpy_expr(exc, names);
            }
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value {
                collect_assigned_names_in_blockpy_expr(value, names);
            }
        }
    }
}

fn collect_assigned_names_in_blockpy_fragment<E>(
    fragment: &BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>,
    names: &mut HashSet<String>,
) where
    E: Clone + Into<Expr>,
{
    for stmt in &fragment.body {
        collect_assigned_names_in_blockpy_stmt(stmt, names);
    }
    if let Some(term) = &fragment.term {
        collect_assigned_names_in_blockpy_term(term, names);
    }
}

fn collect_assigned_names_in_blockpy_stmt<E>(stmt: &BlockPyStmt<E>, names: &mut HashSet<String>)
where
    E: Clone + Into<Expr>,
{
    match stmt {
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::Assign(assign) => {
            names.insert(assign.target.id.to_string());
            collect_assigned_names_in_blockpy_expr(&assign.value, names);
        }
        BlockPyStmt::Expr(expr) => collect_assigned_names_in_blockpy_expr(expr, names),
        BlockPyStmt::If(if_stmt) => {
            collect_assigned_names_in_blockpy_expr(&if_stmt.test, names);
            collect_assigned_names_in_blockpy_fragment(&if_stmt.body, names);
            collect_assigned_names_in_blockpy_fragment(&if_stmt.orelse, names);
        }
    }
}

fn extend_ordered_state(base: &[String], assigned: HashSet<String>) -> Vec<String> {
    let mut out = base.to_vec();
    let mut assigned = assigned.into_iter().collect::<Vec<_>>();
    assigned.sort();
    for name in assigned {
        if !out.iter().any(|existing| existing == &name) {
            out.push(name);
        }
    }
    out
}

fn conservative_state_after_prefix<E>(base: &[String], body: &[BlockPyStmt<E>]) -> Vec<String>
where
    E: Clone + Into<Expr>,
{
    let mut assigned = HashSet::new();
    for stmt in body {
        collect_assigned_names_in_blockpy_stmt(stmt, &mut assigned);
    }
    extend_ordered_state(base, assigned)
}

fn conservative_state_after_if_branches<E>(
    base: &[String],
    if_stmt: &super::BlockPyStructuredIf<E>,
) -> Vec<String>
where
    E: Clone + Into<Expr>,
{
    let mut assigned = HashSet::new();
    collect_assigned_names_in_blockpy_fragment(&if_stmt.body, &mut assigned);
    collect_assigned_names_in_blockpy_fragment(&if_stmt.orelse, &mut assigned);
    extend_ordered_state(base, assigned)
}

fn params_for_linearized_names(
    param_names: &[String],
    declared_params: &[BlockParam],
) -> Vec<BlockParam> {
    param_names
        .iter()
        .map(|name| BlockParam {
            name: name.clone(),
            role: declared_params
                .iter()
                .find(|param| param.name == *name)
                .map(|param| param.role)
                .unwrap_or(BlockParamRole::Local),
        })
        .collect()
}

fn linearize_blockpy_if_sequence<E: Clone + Into<Expr>>(
    label: BlockPyLabel,
    body: Vec<BlockPyStmt<E>>,
    final_term: BlockPyTerm<E>,
    meta: super::BlockPyBlockMeta,
    block_params: Vec<String>,
    declared_params: Vec<BlockParam>,
    exc_target: Option<String>,
    next_label_id: &mut usize,
    out_blocks: &mut Vec<BlockPyBlock<E>>,
    out_block_params: &mut HashMap<String, Vec<String>>,
    out_exception_edges: &mut HashMap<String, Option<String>>,
) {
    let Some(if_index) = body
        .iter()
        .position(|stmt| matches!(stmt, BlockPyStmt::If(_)))
    else {
        out_block_params.insert(label.as_str().to_string(), block_params.clone());
        out_exception_edges.insert(label.as_str().to_string(), exc_target);
        out_blocks.push(BlockPyBlock {
            label,
            body,
            term: final_term,
            params: params_for_linearized_names(&block_params, &declared_params),
            meta,
        });
        return;
    };

    let mut body = body;
    let rest = body.split_off(if_index + 1);
    let if_stmt = match body.pop() {
        Some(BlockPyStmt::If(if_stmt)) => if_stmt,
        _ => unreachable!("expected structured BlockPy if at split point"),
    };
    let available_before_if = conservative_state_after_prefix(&block_params, &body);
    let join_block_params = conservative_state_after_if_branches(&available_before_if, &if_stmt);

    let then_label = fresh_linearized_if_label(&label, next_label_id, "if_then");
    let else_label = fresh_linearized_if_label(&label, next_label_id, "if_else");
    let join_label = if rest.is_empty() {
        None
    } else {
        Some(fresh_linearized_if_label(&label, next_label_id, "if_join"))
    };

    out_block_params.insert(label.as_str().to_string(), block_params);
    out_exception_edges.insert(label.as_str().to_string(), exc_target.clone());
    out_blocks.push(BlockPyBlock {
        label: label.clone(),
        body,
        term: BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: if_stmt.test.clone(),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        }),
        params: params_for_linearized_names(
            out_block_params.get(label.as_str()).unwrap(),
            &declared_params,
        ),
        meta: meta.clone(),
    });

    let branch_fallthrough = join_label
        .clone()
        .map(|label| BlockPyTerm::Jump(label.into()))
        .unwrap_or_else(|| final_term.clone());
    linearize_blockpy_fragment(
        then_label,
        if_stmt.body,
        branch_fallthrough.clone(),
        meta.clone(),
        available_before_if.clone(),
        declared_params.clone(),
        exc_target.clone(),
        next_label_id,
        out_blocks,
        out_block_params,
        out_exception_edges,
    );
    linearize_blockpy_fragment(
        else_label,
        if_stmt.orelse,
        branch_fallthrough,
        meta.clone(),
        available_before_if.clone(),
        declared_params.clone(),
        exc_target.clone(),
        next_label_id,
        out_blocks,
        out_block_params,
        out_exception_edges,
    );

    if let Some(join_label) = join_label {
        linearize_blockpy_if_sequence(
            join_label,
            rest,
            final_term,
            meta,
            join_block_params,
            declared_params,
            exc_target,
            next_label_id,
            out_blocks,
            out_block_params,
            out_exception_edges,
        );
    }
}

fn linearize_blockpy_fragment<E: Clone + Into<Expr>>(
    label: BlockPyLabel,
    fragment: BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>,
    fallthrough_term: BlockPyTerm<E>,
    meta: super::BlockPyBlockMeta,
    block_params: Vec<String>,
    declared_params: Vec<BlockParam>,
    exc_target: Option<String>,
    next_label_id: &mut usize,
    out_blocks: &mut Vec<BlockPyBlock<E>>,
    out_block_params: &mut HashMap<String, Vec<String>>,
    out_exception_edges: &mut HashMap<String, Option<String>>,
) {
    linearize_blockpy_if_sequence(
        label,
        fragment.body,
        fragment.term.unwrap_or(fallthrough_term),
        meta,
        block_params,
        declared_params,
        exc_target,
        next_label_id,
        out_blocks,
        out_block_params,
        out_exception_edges,
    );
}

pub(crate) fn linearize_structured_ifs<E: Clone + Into<Expr>>(
    blocks: &[BlockPyBlock<E>],
    block_params: &HashMap<String, Vec<String>>,
    exception_edges: &HashMap<String, Option<String>>,
) -> (
    Vec<BlockPyBlock<E>>,
    HashMap<String, Vec<String>>,
    HashMap<String, Option<String>>,
) {
    let mut out_blocks = Vec::new();
    let mut out_block_params = HashMap::new();
    let mut out_exception_edges = HashMap::new();
    let mut next_label_id = 0usize;
    for block in blocks {
        let mut params = block_params
            .get(block.label.as_str())
            .cloned()
            .unwrap_or_default();
        for name in block.bb_param_names() {
            if !params.iter().any(|existing| existing == name) {
                params.push(name.to_string());
            }
        }
        let exc_target = exception_edges
            .get(block.label.as_str())
            .cloned()
            .unwrap_or(None);
        linearize_blockpy_if_sequence(
            block.label.clone(),
            block.body.clone(),
            block.term.clone(),
            block.meta.clone(),
            params,
            block.params.clone(),
            exc_target,
            &mut next_label_id,
            &mut out_blocks,
            &mut out_block_params,
            &mut out_exception_edges,
        );
    }
    (out_blocks, out_block_params, out_exception_edges)
}

pub(crate) fn relabel_blockpy_blocks(
    prefix: &str,
    entry_label: &str,
    blocks: &mut [BlockPyBlock<Expr>],
) -> (String, HashMap<String, String>) {
    let mut rename = HashMap::new();
    rename.insert(entry_label.to_string(), format!("{prefix}_start"));

    let mut next_id = 0usize;
    for block in blocks.iter() {
        if rename.contains_key(block.label.as_str()) {
            continue;
        }
        rename.insert(
            block.label.as_str().to_string(),
            format!("{prefix}_{next_id}"),
        );
        next_id += 1;
    }

    let rewritten_entry = apply_label_rename_blockpy(entry_label, &rename, blocks);
    (rewritten_entry, rename)
}

pub(crate) fn fold_jumps_to_trivial_none_return_blockpy<E>(blocks: &mut [BlockPyBlock<E>]) {
    let trivial_ret_none_labels: std::collections::HashSet<String> = blocks
        .iter()
        .filter(|block| block.body.is_empty() && matches!(block.term, BlockPyTerm::Return(None)))
        .map(|block| block.label.as_str().to_string())
        .collect();

    for block in blocks.iter_mut() {
        let jump_target = match &block.term {
            BlockPyTerm::Jump(target) => Some(target.as_str().to_string()),
            _ => None,
        };
        if let Some(target) = jump_target {
            if trivial_ret_none_labels.contains(target.as_str()) {
                block.term = BlockPyTerm::Return(None);
            }
        }
    }
}

pub(crate) fn fold_constant_brif_blockpy(blocks: &mut [BlockPyBlock<Expr>]) {
    for block in blocks.iter_mut() {
        let jump_target = match &block.term {
            BlockPyTerm::IfTerm(BlockPyIfTerm {
                test,
                then_label,
                else_label,
            }) => match test {
                Expr::BooleanLiteral(boolean) => {
                    if boolean.value {
                        Some(then_label.as_str().to_string())
                    } else {
                        Some(else_label.as_str().to_string())
                    }
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(target) = jump_target {
            block.term = BlockPyTerm::Jump(BlockPyLabel::from(target).into());
        }
    }
}

pub(crate) fn prune_unreachable_blockpy_blocks<E>(
    entry_label: &str,
    extra_roots: &[String],
    blocks: &mut Vec<BlockPyBlock<E>>,
) {
    let index_by_label: HashMap<String, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.as_str().to_string(), idx))
        .collect();

    let mut worklist = vec![entry_label.to_string()];
    worklist.extend(extra_roots.iter().cloned());
    let mut reachable = HashSet::new();
    while let Some(label) = worklist.pop() {
        if !reachable.insert(label.clone()) {
            continue;
        }
        let Some(idx) = index_by_label.get(label.as_str()) else {
            continue;
        };
        for succ in blockpy_successors(&blocks[*idx]) {
            worklist.push(succ);
        }
    }

    blocks.retain(|block| reachable.contains(block.label.as_str()));
}

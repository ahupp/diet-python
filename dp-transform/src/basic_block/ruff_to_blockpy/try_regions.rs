use super::compat::set_region_exc_param;
use super::*;
use crate::basic_block::ast_to_ast::body::{suite_ref, Suite};
use crate::basic_block::block_py::{
    BlockPyBlock, BlockPyIfTerm, BlockPyLabel, BlockPyStmt, BlockPyTerm,
};

#[derive(Debug, Clone)]
pub(crate) struct TryPlan {
    pub except_exc_name: String,
    pub finally_reason_name: Option<String>,
    pub finally_return_value_name: Option<String>,
    pub finally_dispatch_label: Option<String>,
    pub finally_return_label: Option<String>,
    pub finally_exc_name: Option<String>,
}

pub(crate) fn build_try_plan(
    fn_name: &str,
    has_finally: bool,
    needs_finally_return_flow: bool,
    next_id: &mut usize,
) -> TryPlan {
    let except_exc_name = compat_next_temp("try_exc", next_id);
    let finally_reason_name = if has_finally && needs_finally_return_flow {
        Some(compat_next_temp("try_reason", next_id))
    } else {
        None
    };
    let finally_return_value_name = if has_finally && needs_finally_return_flow {
        Some(compat_next_temp("try_value", next_id))
    } else {
        None
    };
    let finally_dispatch_label = if has_finally && needs_finally_return_flow {
        Some(compat_next_label(fn_name, next_id))
    } else {
        None
    };
    let finally_return_label = if has_finally && needs_finally_return_flow {
        Some(compat_next_label(fn_name, next_id))
    } else {
        None
    };
    let finally_exc_name = if has_finally {
        Some(compat_next_temp("try_exc", next_id))
    } else {
        None
    };

    TryPlan {
        except_exc_name,
        finally_reason_name,
        finally_return_value_name,
        finally_dispatch_label,
        finally_return_label,
        finally_exc_name,
    }
}

impl TryPlan {
    pub(crate) fn finally_cont_label(&self, rest_entry: &str) -> String {
        self.finally_dispatch_label
            .clone()
            .unwrap_or_else(|| rest_entry.to_string())
    }
}

pub(crate) fn prepare_finally_body(finalbody: &Suite, finally_exc_name: Option<&str>) -> Vec<Stmt> {
    let mut finally_body = flatten_stmt_boxes(finalbody);
    if let Some(finally_exc_name) = finally_exc_name {
        finally_body.insert(
            0,
            py_stmt!(
                "{exc:id} = __dp_current_exception()",
                exc = finally_exc_name,
            ),
        );
        finally_body.push(py_stmt!(
            "if __dp_is_not({exc:id}, None):\n    raise {exc:id}",
            exc = finally_exc_name,
        ));
    }
    finally_body
}

pub(crate) fn prepare_except_body(handlers: &[ast::ExceptHandler]) -> Vec<Stmt> {
    handlers
        .first()
        .map(|handler| {
            let ast::ExceptHandler::ExceptHandler(handler) = handler;
            flatten_stmt_boxes(suite_ref(&handler.body))
        })
        .unwrap_or_else(|| vec![py_stmt!("raise")])
}

pub(crate) struct LoweredTryRegions {
    pub body_label: String,
    pub except_label: String,
    pub body_region_range: std::ops::Range<usize>,
    pub else_region_range: std::ops::Range<usize>,
    pub except_region_range: Option<std::ops::Range<usize>>,
    pub finally_region_range: Option<std::ops::Range<usize>>,
    pub finally_label: Option<String>,
    pub finally_normal_entry: Option<String>,
}

pub(crate) fn lower_try_regions<F>(
    blocks: &mut Vec<BlockPyBlock>,
    try_plan: &TryPlan,
    rest_entry: &str,
    finally_body: Option<Vec<Stmt>>,
    else_body: Vec<Stmt>,
    try_body: Vec<Stmt>,
    except_body: Option<Vec<Stmt>>,
    lower_region: &mut F,
) -> LoweredTryRegions
where
    F: FnMut(&[Stmt], String, &mut Vec<BlockPyBlock>) -> String,
{
    let finally_label = if let Some(finally_body) = finally_body {
        let finally_region_start = blocks.len();
        let finally_label = lower_region(
            &finally_body,
            try_plan.finally_cont_label(rest_entry),
            blocks,
        );
        let finally_region_end = blocks.len();
        let finally_normal_entry = try_plan.finally_exc_name.as_ref().map(|finally_exc_name| {
            let normal_label = format!("{finally_label}__normal");
            blocks.push(compat_block_from_blockpy(
                normal_label.clone(),
                vec![py_stmt!("{exc:id} = None", exc = finally_exc_name.as_str(),)],
                BlockPyTerm::Jump(BlockPyLabel::from(finally_label.clone())),
            ));
            normal_label
        });
        if let (
            Some(finally_return_label),
            Some(finally_dispatch_label),
            Some(return_name),
            Some(reason_name),
        ) = (
            try_plan.finally_return_label.clone(),
            try_plan.finally_dispatch_label.clone(),
            try_plan.finally_return_value_name.as_ref(),
            try_plan.finally_reason_name.as_ref(),
        ) {
            emit_finally_return_dispatch_blocks(
                blocks,
                finally_return_label,
                finally_dispatch_label,
                return_name,
                reason_name,
                rest_entry.to_string(),
            );
        }
        Some((
            finally_label,
            finally_region_start..finally_region_end,
            finally_normal_entry,
        ))
    } else {
        None
    };

    let cleanup_target = finally_label
        .as_ref()
        .map(|(label, _, normal_entry)| normal_entry.clone().unwrap_or_else(|| label.clone()))
        .unwrap_or_else(|| rest_entry.to_string());

    let else_region_start = blocks.len();
    let else_entry = if else_body.is_empty() {
        cleanup_target.clone()
    } else {
        lower_region(&else_body, cleanup_target.clone(), blocks)
    };
    let else_region_end = blocks.len();

    let except_region_range;
    let except_label = if let Some(except_body) = except_body {
        let except_region_start = blocks.len();
        let except_label = lower_region(&except_body, cleanup_target, blocks);
        let except_region_end = blocks.len();
        except_region_range = Some(except_region_start..except_region_end);
        except_label
    } else if let Some((finally_label, _, _)) = finally_label.clone() {
        except_region_range = None;
        finally_label
    } else {
        panic!("expected except body or finally body when lowering try");
    };

    let body_region_start = blocks.len();
    let body_label = lower_region(&try_body, else_entry, blocks);
    let body_region_end = blocks.len();

    LoweredTryRegions {
        body_label,
        except_label,
        body_region_range: body_region_start..body_region_end,
        else_region_range: else_region_start..else_region_end,
        except_region_range,
        finally_region_range: finally_label.as_ref().map(|(_, range, _)| range.clone()),
        finally_normal_entry: finally_label
            .as_ref()
            .and_then(|(_, _, normal_entry)| normal_entry.clone()),
        finally_label: finally_label.map(|(label, _, _)| label),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_try_regions(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    body_label: String,
    except_label: String,
    try_plan: TryPlan,
    body_region_range: std::ops::Range<usize>,
    else_region_range: std::ops::Range<usize>,
    except_region_range: Option<std::ops::Range<usize>>,
    finally_region_range: Option<std::ops::Range<usize>>,
    finally_label: Option<String>,
    finally_normal_entry: Option<String>,
) -> (String, TryRegionPlan) {
    if let (Some(reason_name), Some(return_name), Some(finally_target)) = (
        try_plan.finally_reason_name.as_ref(),
        try_plan.finally_return_value_name.as_ref(),
        finally_normal_entry.as_ref().or(finally_label.as_ref()),
    ) {
        rewrite_region_returns_to_finally_blockpy(
            &mut blocks[body_region_range.clone()],
            reason_name.as_str(),
            return_name.as_str(),
            finally_target.as_str(),
            None,
        );
        rewrite_region_returns_to_finally_blockpy(
            &mut blocks[else_region_range.clone()],
            reason_name.as_str(),
            return_name.as_str(),
            finally_target.as_str(),
            None,
        );
        if let Some(except_region_range) = except_region_range.as_ref() {
            rewrite_region_returns_to_finally_blockpy(
                &mut blocks[except_region_range.clone()],
                reason_name.as_str(),
                return_name.as_str(),
                finally_target.as_str(),
                None,
            );
        }
    }

    if let Some(except_region_range) = except_region_range.as_ref() {
        set_region_exc_param(
            blocks,
            except_region_range,
            try_plan.except_exc_name.as_str(),
        );
    }
    if let (Some(finally_region_range), Some(finally_exc_name)) = (
        finally_region_range.as_ref(),
        try_plan.finally_exc_name.as_ref(),
    ) {
        set_region_exc_param(blocks, finally_region_range, finally_exc_name.as_str());
    }

    let cleanup_region_labels = if finally_label.is_some() {
        let mut labels = collect_region_label_names(&blocks[else_region_range.clone()]);
        if let Some(except_region_range) = except_region_range.as_ref() {
            labels.extend(collect_region_label_names(
                &blocks[except_region_range.clone()],
            ));
        }
        labels
    } else {
        Vec::new()
    };
    let try_region = TryRegionPlan {
        body_region_labels: collect_region_label_names(&blocks[body_region_range]),
        body_exception_target: except_label.clone(),
        cleanup_region_labels,
        cleanup_exception_target: finally_label.clone(),
    };

    let label = emit_try_jump_entry(blocks, label, linear, body_label, except_label);
    (label, try_region)
}

pub(crate) fn emit_finally_return_dispatch_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    finally_return_label: String,
    finally_dispatch_label: String,
    return_name: &str,
    reason_name: &str,
    rest_entry: String,
) {
    blocks.push(compat_block_from_blockpy(
        finally_return_label.clone(),
        Vec::new(),
        BlockPyTerm::Return(Some(py_expr!("{name:id}", name = return_name).into())),
    ));
    blocks.push(compat_block_from_blockpy(
        finally_dispatch_label.clone(),
        Vec::new(),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: py_expr!("__dp_eq({reason:id}, 'return')", reason = reason_name,).into(),
            then_label: BlockPyLabel::from(finally_return_label),
            else_label: BlockPyLabel::from(rest_entry),
        }),
    ));
}

pub(crate) fn collect_region_label_names(blocks: &[BlockPyBlock]) -> Vec<String> {
    blocks
        .iter()
        .map(|block| block.label.as_str().to_string())
        .collect()
}

pub(crate) fn emit_try_jump_entry(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    body_label: String,
    except_label: String,
) -> String {
    blocks.push(compat_block_from_blockpy(
        label.clone(),
        linear,
        BlockPyTerm::TryJump(BlockPyTryJump {
            body_label: BlockPyLabel::from(body_label),
            except_label: BlockPyLabel::from(except_label),
        }),
    ));
    label
}

pub(crate) fn block_references_label(block: &BlockPyBlock, label: &str) -> bool {
    fn stmt_references_label(stmt: &BlockPyStmt, label: &str) -> bool {
        match stmt {
            BlockPyStmt::If(if_stmt) => {
                stmt_fragment_references_label(&if_stmt.body, label)
                    || stmt_fragment_references_label(&if_stmt.orelse, label)
            }
            _ => false,
        }
    }

    fn stmt_list_references_label(stmts: &[BlockPyStmt], label: &str) -> bool {
        stmts.iter().any(|stmt| stmt_references_label(stmt, label))
    }

    fn stmt_fragment_references_label(
        fragment: &crate::basic_block::block_py::BlockPyCfgFragment<BlockPyStmt, BlockPyTerm>,
        label: &str,
    ) -> bool {
        stmt_list_references_label(&fragment.body, label)
            || fragment
                .term
                .as_ref()
                .is_some_and(|term| term_references_label(term, label))
    }

    fn term_references_label(term: &BlockPyTerm, label: &str) -> bool {
        match term {
            BlockPyTerm::Jump(target) => target.as_str() == label,
            BlockPyTerm::IfTerm(if_term) => {
                if_term.then_label.as_str() == label || if_term.else_label.as_str() == label
            }
            BlockPyTerm::BranchTable(branch) => {
                branch.default_label.as_str() == label
                    || branch.targets.iter().any(|target| target.as_str() == label)
            }
            BlockPyTerm::TryJump(try_jump) => {
                try_jump.body_label.as_str() == label || try_jump.except_label.as_str() == label
            }
            BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => false,
        }
    }

    block
        .body
        .iter()
        .any(|stmt| stmt_references_label(stmt, label))
        || term_references_label(&block.term, label)
}

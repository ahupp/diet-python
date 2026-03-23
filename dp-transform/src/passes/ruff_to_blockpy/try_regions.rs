use super::compat::set_region_exc_param;
use super::*;
use crate::block_py::{
    AbruptKind, BlockArg, BlockParamRole, BlockPyBranchTable, BlockPyCfgBlockBuilder, BlockPyEdge,
    BlockPyLabel, BlockPyRaise, BlockPyStmt, BlockPyTerm,
};
use crate::passes::ast_to_ast::body::{suite_ref, Suite};

#[derive(Debug, Clone)]
pub(crate) struct TryPlan {
    pub except_exc_name: String,
    pub finally_abrupt_kind_name: Option<String>,
    pub finally_abrupt_payload_name: Option<String>,
    pub finally_dispatch_label: Option<BlockPyLabel>,
    pub finally_return_label: Option<BlockPyLabel>,
    pub finally_raise_label: Option<BlockPyLabel>,
    pub finally_exc_name: Option<String>,
}

pub(crate) fn build_try_plan(
    name_gen: &NameGen,
    has_finally: bool,
    _needs_finally_return_flow: bool,
) -> TryPlan {
    let except_exc_name = name_gen.next_tmp_name("try_exc").to_string();
    let finally_abrupt_kind_name = if has_finally {
        Some(name_gen.next_tmp_name("try_abrupt_kind").to_string())
    } else {
        None
    };
    let finally_abrupt_payload_name = if has_finally {
        Some(name_gen.next_tmp_name("try_abrupt_payload").to_string())
    } else {
        None
    };
    let finally_dispatch_label = if has_finally {
        Some(name_gen.next_block_name())
    } else {
        None
    };
    let finally_return_label = if has_finally {
        Some(name_gen.next_block_name())
    } else {
        None
    };
    let finally_raise_label = if has_finally {
        Some(name_gen.next_block_name())
    } else {
        None
    };
    let finally_exc_name = if has_finally {
        Some(name_gen.next_tmp_name("try_exc").to_string())
    } else {
        None
    };

    TryPlan {
        except_exc_name,
        finally_abrupt_kind_name,
        finally_abrupt_payload_name,
        finally_dispatch_label,
        finally_return_label,
        finally_raise_label,
        finally_exc_name,
    }
}

impl TryPlan {
    pub(crate) fn finally_cont_label(&self, rest_entry: &str) -> String {
        self.finally_dispatch_label
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| rest_entry.to_string())
    }
}

pub(crate) fn prepare_finally_body(finalbody: &Suite) -> Vec<Stmt> {
    finalbody.to_vec()
}

pub(crate) fn prepare_except_body(handlers: &[ast::ExceptHandler]) -> Vec<Stmt> {
    handlers
        .first()
        .map(|handler| {
            let ast::ExceptHandler::ExceptHandler(handler) = handler;
            suite_ref(&handler.body).to_vec()
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
    pub finally_exception_entry: Option<String>,
}

pub(crate) fn lower_try_regions<F>(
    blocks: &mut Vec<BlockPyBlock>,
    try_plan: &TryPlan,
    rest_entry: &str,
    finally_body: Option<Vec<Stmt>>,
    else_body: Vec<Stmt>,
    try_body: Vec<Stmt>,
    except_body: Option<Vec<Stmt>>,
    active_exc_target: Option<String>,
    lower_region: &mut F,
) -> LoweredTryRegions
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let finally_label = if let Some(finally_body) = finally_body {
        let finally_region_start = blocks.len();
        let finally_label = lower_region(
            &finally_body,
            try_plan.finally_cont_label(rest_entry),
            active_exc_target.clone(),
            blocks,
        );
        let finally_region_end = blocks.len();
        if let Some(finally_entry) = blocks
            .iter_mut()
            .find(|block| block.label.as_str() == finally_label)
        {
            if let Some(kind_name) = try_plan.finally_abrupt_kind_name.as_ref() {
                finally_entry.ensure_param(kind_name.clone(), BlockParamRole::AbruptKind);
            }
            if let Some(payload_name) = try_plan.finally_abrupt_payload_name.as_ref() {
                finally_entry.ensure_param(payload_name.clone(), BlockParamRole::AbruptPayload);
            }
        }
        let finally_normal_entry = try_plan.finally_abrupt_kind_name.as_ref().map(|_| {
            let normal_label = format!("{finally_label}__normal");
            let mut block = BlockPyCfgBlockBuilder::<BlockPyStmt, BlockPyTerm>::new(
                BlockPyLabel::from(normal_label.clone()),
            );
            let mut args = Vec::new();
            args.push(BlockArg::AbruptKind(AbruptKind::Fallthrough));
            args.push(BlockArg::None);
            block.set_term(BlockPyTerm::Jump(BlockPyEdge::with_args(
                BlockPyLabel::from(finally_label.clone()),
                args,
            )));
            let block = block.finish(None);
            let block = crate::block_py::CfgBlock {
                label: block.label,
                body: block.body,
                term: block.term,
                params: block.params,
                exc_edge: active_exc_target
                    .as_ref()
                    .map(|target| BlockPyEdge::new(BlockPyLabel::from(target.clone()))),
            };
            blocks.push(block);
            normal_label
        });
        let finally_exception_entry = try_plan.finally_exc_name.as_ref().map(|finally_exc_name| {
            let exception_label = format!("{finally_label}__exception");
            let mut block = BlockPyCfgBlockBuilder::<BlockPyStmt, BlockPyTerm>::new(
                BlockPyLabel::from(exception_label.clone()),
            )
            .with_exc_param(Some(finally_exc_name.clone()));
            let args = vec![
                BlockArg::AbruptKind(AbruptKind::Exception),
                BlockArg::Name(finally_exc_name.clone()),
            ];
            block.set_term(BlockPyTerm::Jump(BlockPyEdge::with_args(
                BlockPyLabel::from(finally_label.clone()),
                args,
            )));
            let block = block.finish(None);
            let block = crate::block_py::CfgBlock {
                label: block.label,
                body: block.body,
                term: block.term,
                params: block.params,
                exc_edge: active_exc_target
                    .as_ref()
                    .map(|target| BlockPyEdge::new(BlockPyLabel::from(target.clone()))),
            };
            blocks.push(block);
            exception_label
        });
        if let (
            Some(finally_return_label),
            Some(finally_dispatch_label),
            Some(finally_raise_label),
            Some(payload_name),
            Some(kind_name),
        ) = (
            try_plan.finally_return_label.clone(),
            try_plan.finally_dispatch_label.clone(),
            try_plan.finally_raise_label.clone(),
            try_plan.finally_abrupt_payload_name.as_ref(),
            try_plan.finally_abrupt_kind_name.as_ref(),
        ) {
            emit_finally_abrupt_dispatch_blocks(
                blocks,
                finally_return_label,
                finally_raise_label,
                finally_dispatch_label,
                payload_name,
                kind_name,
                rest_entry.to_string(),
                active_exc_target.clone(),
            );
        }
        Some((
            finally_label,
            finally_region_start..finally_region_end,
            finally_normal_entry,
            finally_exception_entry,
        ))
    } else {
        None
    };

    let cleanup_target = finally_label
        .as_ref()
        .map(|(label, _, normal_entry, _)| normal_entry.clone().unwrap_or_else(|| label.clone()))
        .unwrap_or_else(|| rest_entry.to_string());
    let cleanup_exc_target = finally_label
        .as_ref()
        .map(|(label, _, _, finally_exception_entry)| {
            finally_exception_entry
                .clone()
                .unwrap_or_else(|| label.clone())
        })
        .or(active_exc_target.clone());

    let else_region_start = blocks.len();
    let else_entry = if else_body.is_empty() {
        cleanup_target.clone()
    } else {
        lower_region(
            &else_body,
            cleanup_target.clone(),
            cleanup_exc_target.clone(),
            blocks,
        )
    };
    let else_region_end = blocks.len();

    let except_region_range;
    let except_label = if let Some(except_body) = except_body {
        let except_region_start = blocks.len();
        let except_label = lower_region(&except_body, cleanup_target, cleanup_exc_target, blocks);
        let except_region_end = blocks.len();
        except_region_range = Some(except_region_start..except_region_end);
        except_label
    } else if let Some((finally_label, _, _, finally_exception_entry)) = finally_label.clone() {
        except_region_range = None;
        finally_exception_entry.unwrap_or(finally_label)
    } else {
        panic!("expected except body or finally body when lowering try");
    };

    let body_region_start = blocks.len();
    let body_label = lower_region(&try_body, else_entry, Some(except_label.clone()), blocks);
    let body_region_end = blocks.len();

    LoweredTryRegions {
        body_label,
        except_label,
        body_region_range: body_region_start..body_region_end,
        else_region_range: else_region_start..else_region_end,
        except_region_range,
        finally_region_range: finally_label.as_ref().map(|(_, range, _, _)| range.clone()),
        finally_normal_entry: finally_label
            .as_ref()
            .and_then(|(_, _, normal_entry, _)| normal_entry.clone()),
        finally_exception_entry: finally_label
            .as_ref()
            .and_then(|(_, _, _, exception_entry)| exception_entry.clone()),
        finally_label: finally_label.map(|(label, _, _, _)| label),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_try_regions(
    blocks: &mut Vec<BlockPyBlock>,
    label: BlockPyLabel,
    linear: Vec<Stmt>,
    body_label: String,
    except_label: String,
    try_plan: TryPlan,
    body_region_range: std::ops::Range<usize>,
    else_region_range: std::ops::Range<usize>,
    except_region_range: Option<std::ops::Range<usize>>,
    _finally_region_range: Option<std::ops::Range<usize>>,
    finally_label: Option<String>,
    _finally_normal_entry: Option<String>,
    _finally_exception_entry: Option<String>,
    active_exc_target: Option<String>,
) -> String {
    if let Some(finally_target) = finally_label.as_ref() {
        let payload_name = try_plan
            .finally_abrupt_payload_name
            .as_deref()
            .expect("finally region must have abrupt payload slot");
        rewrite_region_returns_to_finally_blockpy(
            &mut blocks[body_region_range.clone()],
            finally_target.as_str(),
            payload_name,
        );
        rewrite_region_returns_to_finally_blockpy(
            &mut blocks[else_region_range.clone()],
            finally_target.as_str(),
            payload_name,
        );
        if let Some(except_region_range) = except_region_range.as_ref() {
            rewrite_region_returns_to_finally_blockpy(
                &mut blocks[except_region_range.clone()],
                finally_target.as_str(),
                payload_name,
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
        _finally_region_range.as_ref(),
        try_plan.finally_exc_name.as_ref(),
    ) {
        set_region_exc_param(blocks, finally_region_range, finally_exc_name.as_str());
    }
    emit_try_jump_entry(
        blocks,
        label,
        linear,
        body_label,
        except_label,
        active_exc_target,
    )
}

pub(crate) fn emit_finally_abrupt_dispatch_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    finally_return_label: BlockPyLabel,
    finally_raise_label: BlockPyLabel,
    finally_dispatch_label: BlockPyLabel,
    payload_name: &str,
    kind_name: &str,
    rest_entry: String,
    active_exc_target: Option<String>,
) {
    blocks.push(compat_block_from_blockpy_with_exc_target(
        finally_return_label.clone(),
        Vec::new(),
        BlockPyTerm::Return(py_expr!("{name:id}", name = payload_name).into()),
        active_exc_target.as_deref(),
    ));
    blocks.push(compat_block_from_blockpy_with_exc_target(
        finally_raise_label.clone(),
        Vec::new(),
        BlockPyTerm::Raise(BlockPyRaise {
            exc: Some(py_expr!("{name:id}", name = payload_name).into()),
        }),
        active_exc_target.as_deref(),
    ));
    blocks.push(compat_block_from_blockpy_with_exc_target(
        finally_dispatch_label.clone(),
        Vec::new(),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: py_expr!("{name:id}", name = kind_name).into(),
            targets: vec![
                BlockPyLabel::from(rest_entry.clone()),
                finally_return_label,
                finally_raise_label,
            ],
            default_label: BlockPyLabel::from(rest_entry),
        }),
        active_exc_target.as_deref(),
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
    label: BlockPyLabel,
    linear: Vec<Stmt>,
    body_label: String,
    _except_label: String,
    active_exc_target: Option<String>,
) -> String {
    blocks.push(compat_block_from_blockpy_with_exc_target(
        label.clone(),
        linear,
        BlockPyTerm::Jump(BlockPyLabel::from(body_label).into()),
        active_exc_target.as_deref(),
    ));
    label.to_string()
}

pub(crate) fn block_references_label(
    block: &crate::block_py::CfgBlock<BlockPyStmt, BlockPyTerm>,
    label: &str,
) -> bool {
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
        fragment: &crate::block_py::BlockPyCfgFragment<BlockPyStmt, BlockPyTerm>,
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
            BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => false,
        }
    }

    block
        .body
        .iter()
        .any(|stmt| stmt_references_label(stmt, label))
        || block
            .exc_edge
            .as_ref()
            .is_some_and(|edge| edge.target.as_str() == label)
        || term_references_label(&block.term, label)
}

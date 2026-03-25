use super::{build_blockpy_closure_layout, resume_closure_bindings};
use crate::block_py::{
    BlockPyBindingKind, BlockPyBindingPurpose, BlockPyCallableScopeKind,
    BlockPyCallableSemanticInfo, BlockPyCellBindingKind, BlockPyCfgBlockBuilder, BlockPyTerm,
    ClosureInit, ClosureLayout, ClosureSlot, FunctionId, FunctionName,
};
use crate::passes::ast_to_ast::scope_helpers::is_internal_symbol;
use crate::passes::ruff_to_blockpy::lower_stmts_to_blockpy_stmts;
use crate::{py_expr, py_stmt};
use ruff_python_ast::Expr;
use std::collections::HashSet;

fn blockpy_make_dp_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
        panic!("expected call expression for __dp_tuple");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

fn closure_backed_generator_init_expr(slot: &ClosureSlot) -> Expr {
    match slot.init {
        ClosureInit::InheritedCapture => {
            panic!("inherited captures do not allocate new cells in outer factories")
        }
        ClosureInit::Parameter => {
            py_expr!("{name:id}", name = slot.logical_name.as_str())
        }
        ClosureInit::DeletedSentinel => py_expr!("__dp_DELETED"),
        ClosureInit::RuntimePcUnstarted => py_expr!("1"),
        ClosureInit::RuntimeNone => py_expr!("None"),
        ClosureInit::Deferred => py_expr!("None"),
    }
}

fn build_closure_backed_generator_factory_block(
    factory_label: &str,
    visible_function_id: FunctionId,
    resume_function_id: FunctionId,
    resume_state_order: &[String],
    layout: &ClosureLayout,
    is_coroutine: bool,
    is_async_generator: bool,
) -> crate::block_py::BlockPyBlock<Expr> {
    let mut body = Vec::new();

    for slot in layout.cellvars.iter().chain(layout.runtime_cells.iter()) {
        let stmt = py_stmt!(
            "{cell:id} = __dp_make_cell({init:expr})",
            cell = slot.storage_name.as_str(),
            init = closure_backed_generator_init_expr(slot),
        );
        let lowered = lower_stmts_to_blockpy_stmts::<Expr>(&[stmt])
            .unwrap_or_else(|err| panic!("failed to lower generator factory cell init: {err}"));
        assert!(lowered.term.is_none());
        body.extend(lowered.body);
    }

    let closure_bindings = resume_closure_bindings(layout, resume_state_order);
    let closure_names = closure_bindings
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let closure_values = blockpy_make_dp_tuple(
        closure_bindings
            .iter()
            .map(|(_, value_name)| py_expr!("{name:id}", name = value_name.as_str()))
            .collect(),
    );

    let resume_entry = py_expr!(
            "__dp_def_hidden_resume_fn({function_id:literal}, {closure_names:expr}, {closure_values:expr}, __dp_globals(), async_gen={async_gen:expr})",
            function_id = resume_function_id.0,
            closure_names = blockpy_make_dp_tuple(
                closure_names
                    .iter()
                    .map(|state_name| py_expr!("{value:literal}", value = state_name.as_str()))
                    .collect(),
            ),
            closure_values = closure_values,
            async_gen = if is_async_generator {
                py_expr!("True")
            } else {
                py_expr!("False")
            },
        );

    let generator_expr = if is_async_generator {
        py_expr!(
                "__dp_make_closure_async_generator({function_id:literal}, {resume:expr}, __dp_globals())",
                function_id = visible_function_id.0,
                resume = resume_entry,
            )
    } else {
        py_expr!(
            "__dp_make_closure_generator({function_id:literal}, {resume:expr}, __dp_globals())",
            function_id = visible_function_id.0,
            resume = resume_entry,
        )
    };

    let return_value = if is_coroutine {
        py_expr!(
            "__dp_make_coroutine_from_generator({gen:expr})",
            gen = generator_expr
        )
    } else {
        generator_expr
    };

    let mut block = BlockPyCfgBlockBuilder::new(factory_label.into());
    block.extend(body);
    block.set_term(BlockPyTerm::Return(return_value.into()));
    block.finish(None)
}

#[test]
fn build_blockpy_closure_layout_classifies_capture_local_and_runtime_cells() {
    let layout = build_blockpy_closure_layout(
        &["arg".to_string()],
        &[
            "_dp_self".to_string(),
            "arg".to_string(),
            "captured".to_string(),
            "_dp_yieldfrom".to_string(),
            "_dp_pc".to_string(),
            "_dp_try_exc_0".to_string(),
        ],
        &["captured".to_string()],
        &HashSet::from(["_dp_try_exc_0".to_string()]),
    );

    assert_eq!(
        layout
            .freevars
            .iter()
            .map(|slot| (slot.logical_name.as_str(), slot.storage_name.as_str()))
            .collect::<Vec<_>>(),
        vec![("captured", "_dp_cell_captured")]
    );
    assert_eq!(
        layout
            .cellvars
            .iter()
            .map(|slot| (
                slot.logical_name.as_str(),
                slot.storage_name.as_str(),
                &slot.init
            ))
            .collect::<Vec<_>>(),
        vec![
            ("arg", "_dp_cell_arg", &ClosureInit::Parameter),
            (
                "_dp_try_exc_0",
                "_dp_cell__dp_try_exc_0",
                &ClosureInit::DeletedSentinel
            ),
        ]
    );
    assert_eq!(
        layout
            .runtime_cells
            .iter()
            .map(|slot| (
                slot.logical_name.as_str(),
                slot.storage_name.as_str(),
                &slot.init
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "_dp_yieldfrom",
                "_dp_cell__dp_yieldfrom",
                &ClosureInit::RuntimeNone
            ),
            (
                "_dp_pc",
                "_dp_cell__dp_pc",
                &ClosureInit::RuntimePcUnstarted
            ),
        ]
    );
}

#[test]
fn builds_closure_backed_generator_factory_block() {
    let layout = ClosureLayout {
        freevars: vec![ClosureSlot {
            logical_name: "captured".to_string(),
            storage_name: "_dp_cell_captured".to_string(),
            init: ClosureInit::InheritedCapture,
        }],
        cellvars: vec![ClosureSlot {
            logical_name: "x".to_string(),
            storage_name: "_dp_cell_x".to_string(),
            init: ClosureInit::Parameter,
        }],
        runtime_cells: vec![ClosureSlot {
            logical_name: "_dp_pc".to_string(),
            storage_name: "_dp_cell__dp_pc".to_string(),
            init: ClosureInit::RuntimePcUnstarted,
        }],
    };

    let block = build_closure_backed_generator_factory_block(
        "_dp_bb_demo_factory",
        FunctionId(1),
        FunctionId(0),
        &[
            "_dp_self".to_string(),
            "_dp_send_value".to_string(),
            "_dp_resume_exc".to_string(),
            "_dp_cell_captured".to_string(),
            "_dp_cell_x".to_string(),
            "_dp_cell__dp_pc".to_string(),
        ],
        &layout,
        false,
        false,
    );

    assert_eq!(block.label.as_str(), "_dp_bb_demo_factory");
    assert!(matches!(block.term, BlockPyTerm::Return(_)));
}

#[test]
fn resume_closure_bindings_include_storage_aliases_for_cell_backed_state() {
    let layout = ClosureLayout {
        freevars: vec![ClosureSlot {
            logical_name: "captured".to_string(),
            storage_name: "_dp_cell_captured".to_string(),
            init: ClosureInit::InheritedCapture,
        }],
        cellvars: vec![ClosureSlot {
            logical_name: "total".to_string(),
            storage_name: "_dp_cell_total".to_string(),
            init: ClosureInit::Deferred,
        }],
        runtime_cells: vec![ClosureSlot {
            logical_name: "_dp_pc".to_string(),
            storage_name: "_dp_cell__dp_pc".to_string(),
            init: ClosureInit::RuntimePcUnstarted,
        }],
    };

    assert_eq!(
        resume_closure_bindings(
            &layout,
            &[
                "_dp_self".to_string(),
                "_dp_send_value".to_string(),
                "_dp_resume_exc".to_string(),
                "_dp_cell_captured".to_string(),
                "total".to_string(),
                "_dp_pc".to_string(),
            ],
        ),
        vec![
            (
                "_dp_cell_captured".to_string(),
                "_dp_cell_captured".to_string()
            ),
            ("total".to_string(), "_dp_cell_total".to_string()),
            ("_dp_pc".to_string(), "_dp_cell__dp_pc".to_string()),
            ("_dp_cell_total".to_string(), "_dp_cell_total".to_string()),
            ("_dp_cell__dp_pc".to_string(), "_dp_cell__dp_pc".to_string()),
        ]
    );
}

#[test]
fn resume_semantic_marks_generator_state_as_cell_captures() {
    let layout = ClosureLayout {
        freevars: vec![ClosureSlot {
            logical_name: "captured".to_string(),
            storage_name: "_dp_cell_captured".to_string(),
            init: ClosureInit::InheritedCapture,
        }],
        cellvars: vec![ClosureSlot {
            logical_name: "total".to_string(),
            storage_name: "_dp_cell_total".to_string(),
            init: ClosureInit::Deferred,
        }],
        runtime_cells: vec![ClosureSlot {
            logical_name: "_dp_pc".to_string(),
            storage_name: "_dp_cell__dp_pc".to_string(),
            init: ClosureInit::RuntimePcUnstarted,
        }],
    };
    let mut semantic = BlockPyCallableSemanticInfo {
        names: FunctionName::new("gen_resume", "_dp_resume", "gen", "gen"),
        scope_kind: BlockPyCallableScopeKind::Function,
        ..Default::default()
    };
    for slot in layout
        .freevars
        .iter()
        .chain(layout.cellvars.iter())
        .chain(layout.runtime_cells.iter())
    {
        semantic.insert_binding(
            slot.logical_name.clone(),
            BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture),
            is_internal_symbol(slot.logical_name.as_str()),
        );
    }

    assert_eq!(semantic.names.bind_name, "gen_resume");
    assert_eq!(
        semantic.binding_kind("captured"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.binding_kind("total"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.binding_kind("_dp_pc"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("_dp_pc"),
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
    );
    assert_eq!(
        semantic.effective_binding("_dp_pc", BlockPyBindingPurpose::Load),
        Some(crate::block_py::BlockPyEffectiveBinding::Cell(
            BlockPyCellBindingKind::Capture
        ))
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("_dp_self"),
        BlockPyBindingKind::Local
    );
}

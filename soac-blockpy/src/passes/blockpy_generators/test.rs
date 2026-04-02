use super::{
    augment_resume_semantic_for_standard_name_binding, build_blockpy_storage_layout,
    persistent_generator_state_order, resume_closure_bindings,
};
use crate::block_py::{
    BlockPyBindingKind, BlockPyBindingPurpose, BlockPyCallableScopeKind,
    BlockPyCallableSemanticInfo, BlockPyCellBindingKind, BlockPyCfgBlockBuilder, BlockPyLabel,
    BlockPyTerm, ClosureInit, ClosureSlot, FunctionId, FunctionName, StorageLayout,
};
use crate::passes::ast_to_ast::scope_helpers::is_internal_symbol;
use crate::py_expr;
use ruff_python_ast::Expr;
use std::collections::HashSet;

fn generator_test_semantic() -> BlockPyCallableSemanticInfo {
    BlockPyCallableSemanticInfo {
        names: FunctionName::new("gen", "gen", "gen", "gen"),
        scope_kind: BlockPyCallableScopeKind::Function,
        ..Default::default()
    }
}

fn generator_resume_source_semantic(layout: &StorageLayout) -> BlockPyCallableSemanticInfo {
    let mut semantic = generator_test_semantic();
    for slot in &layout.freevars {
        semantic.insert_binding_with_cell_names(
            slot.logical_name.clone(),
            BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture),
            is_internal_symbol(slot.logical_name.as_str()),
            Some(slot.logical_name.clone()),
            Some(slot.storage_name.clone()),
        );
    }
    for slot in &layout.cellvars {
        semantic.insert_binding_with_cell_names(
            slot.logical_name.clone(),
            BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner),
            is_internal_symbol(slot.logical_name.as_str()),
            Some(slot.storage_name.clone()),
            Some(slot.storage_name.clone()),
        );
    }
    semantic
}

fn blockpy_make_dp_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
        panic!("expected call expression for __dp_tuple");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

fn build_closure_backed_generator_factory_block(
    _factory_label: &str,
    visible_names: &FunctionName,
    resume_function_id: FunctionId,
    _resume_state_order: &[String],
    _layout: &StorageLayout,
    is_coroutine: bool,
    is_async_generator: bool,
) -> crate::block_py::BlockPyBlock<Expr> {
    let resume_entry = py_expr!(
        "__dp_make_function({function_id:literal}, \"function\", __dp_tuple(), __dp_tuple(), None)",
        function_id = resume_function_id.0,
    );

    let generator_expr = if is_async_generator {
        py_expr!(
            "runtime.ClosureAsyncGenerator(resume={resume:expr}, name={name:literal}, qualname={qualname:literal}, code=runtime.code_template_async_gen.__code__.replace(co_name={name:literal}, co_qualname={qualname:literal}), yieldfrom_cell=__dp_cell_ref(\"_dp_yieldfrom\"), throw_context_cell=__dp_cell_ref(\"_dp_throw_context\"))",
            resume = resume_entry,
            name = visible_names.display_name.as_str(),
            qualname = visible_names.qualname.as_str(),
        )
    } else {
        py_expr!(
            "runtime.ClosureGenerator(resume={resume:expr}, name={name:literal}, qualname={qualname:literal}, code=runtime.code_template_gen.__code__.replace(co_name={name:literal}, co_qualname={qualname:literal}), yieldfrom_cell=__dp_cell_ref(\"_dp_yieldfrom\"), throw_context_cell=__dp_cell_ref(\"_dp_throw_context\"))",
            resume = resume_entry,
            name = visible_names.display_name.as_str(),
            qualname = visible_names.qualname.as_str(),
        )
    };

    let return_value = if is_coroutine {
        py_expr!("runtime.Coroutine({gen:expr})", gen = generator_expr)
    } else {
        generator_expr
    };

    let mut block = BlockPyCfgBlockBuilder::new(BlockPyLabel::from(0u32));
    block.set_term(BlockPyTerm::Return(return_value.into()));
    block.finish(None)
}

#[test]
fn resume_closure_bindings_keep_internal_eval_state_on_runtime_binding_path() {
    let layout = StorageLayout {
        freevars: vec![ClosureSlot {
            logical_name: "captured".to_string(),
            storage_name: "_dp_cell_captured".to_string(),
            init: ClosureInit::InheritedCapture,
        }],
        cellvars: vec![
            ClosureSlot {
                logical_name: "total".to_string(),
                storage_name: "_dp_cell_total".to_string(),
                init: ClosureInit::Deferred,
            },
            ClosureSlot {
                logical_name: "_dp_eval_1".to_string(),
                storage_name: "_dp_cell__dp_eval_1".to_string(),
                init: ClosureInit::Deferred,
            },
            ClosureSlot {
                logical_name: "_dp_eval_2".to_string(),
                storage_name: "_dp_cell__dp_eval_2".to_string(),
                init: ClosureInit::Deferred,
            },
            ClosureSlot {
                logical_name: "_dp_try_exc_0".to_string(),
                storage_name: "_dp_cell__dp_try_exc_0".to_string(),
                init: ClosureInit::DeletedSentinel,
            },
        ],
        runtime_cells: vec![
            ClosureSlot {
                logical_name: "_dp_yieldfrom".to_string(),
                storage_name: "_dp_cell__dp_yieldfrom".to_string(),
                init: ClosureInit::RuntimeNone,
            },
            ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            },
        ],
        stack_slots: Vec::new(),
    };

    let semantic = generator_resume_source_semantic(&layout);
    let closure_bindings = resume_closure_bindings(
        &semantic,
        &[
            "captured".to_string(),
            "total".to_string(),
            "_dp_eval_1".to_string(),
            "_dp_eval_2".to_string(),
            "_dp_yieldfrom".to_string(),
            "_dp_pc".to_string(),
            "_dp_try_exc_0".to_string(),
        ],
    );

    assert_eq!(
        closure_bindings.runtime_state_bindings,
        vec![
            ("captured".to_string(), "_dp_cell_captured".to_string()),
            ("total".to_string(), "_dp_cell_total".to_string()),
            ("_dp_eval_1".to_string(), "_dp_cell__dp_eval_1".to_string()),
            ("_dp_eval_2".to_string(), "_dp_cell__dp_eval_2".to_string()),
            (
                "_dp_yieldfrom".to_string(),
                "_dp_cell__dp_yieldfrom".to_string()
            ),
            ("_dp_pc".to_string(), "_dp_cell__dp_pc".to_string()),
            (
                "_dp_try_exc_0".to_string(),
                "_dp_cell__dp_try_exc_0".to_string()
            ),
        ]
    );
}

#[test]
fn persistent_generator_state_order_omits_resume_abi_params() {
    let layout = StorageLayout {
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
        stack_slots: Vec::new(),
    };

    assert_eq!(
        persistent_generator_state_order(&layout),
        vec![
            "captured".to_string(),
            "total".to_string(),
            "_dp_pc".to_string(),
        ]
    );
}

#[test]
fn build_blockpy_storage_layout_classifies_capture_local_and_runtime_cells() {
    let semantic = generator_test_semantic();
    let layout = build_blockpy_storage_layout(
        &semantic,
        &["arg".to_string()],
        &[
            "arg".to_string(),
            "captured".to_string(),
            "_dp_yieldfrom".to_string(),
            "_dp_pc".to_string(),
            "_dp_throw_context".to_string(),
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
            (
                "_dp_throw_context",
                "_dp_cell__dp_throw_context",
                &ClosureInit::RuntimeNone
            ),
        ]
    );
}

#[test]
fn build_blockpy_storage_layout_uses_semantic_classcell_storage_mapping() {
    let mut semantic = generator_test_semantic();
    semantic.insert_binding(
        "__class__",
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner),
        false,
        Some("_dp_classcell".to_string()),
    );

    let layout = build_blockpy_storage_layout(
        &semantic,
        &[],
        &["__class__".to_string()],
        &[],
        &HashSet::new(),
    );

    assert_eq!(
        layout
            .cellvars
            .iter()
            .map(|slot| (slot.logical_name.as_str(), slot.storage_name.as_str()))
            .collect::<Vec<_>>(),
        vec![("__class__", "_dp_classcell")]
    );
}

#[test]
fn builds_closure_backed_generator_factory_block() {
    let layout = StorageLayout {
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
        runtime_cells: vec![
            ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            },
            ClosureSlot {
                logical_name: "_dp_throw_context".to_string(),
                storage_name: "_dp_cell__dp_throw_context".to_string(),
                init: ClosureInit::RuntimeNone,
            },
        ],
        stack_slots: Vec::new(),
    };

    let block = build_closure_backed_generator_factory_block(
        "_dp_bb_demo_factory",
        &FunctionName::new("gen", "gen", "gen", "gen"),
        FunctionId(0),
        &[
            "_dp_cell_captured".to_string(),
            "_dp_cell_x".to_string(),
            "_dp_cell__dp_pc".to_string(),
        ],
        &layout,
        false,
        false,
    );

    assert_eq!(block.label, BlockPyLabel::from(0u32));
    assert!(block.body.is_empty(), "{block:?}");
    assert!(matches!(block.term, BlockPyTerm::Return(_)));
}

#[test]
fn resume_closure_bindings_use_semantic_capture_sources_for_cell_backed_state() {
    let layout = StorageLayout {
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
        runtime_cells: vec![
            ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            },
            ClosureSlot {
                logical_name: "_dp_throw_context".to_string(),
                storage_name: "_dp_cell__dp_throw_context".to_string(),
                init: ClosureInit::RuntimeNone,
            },
        ],
        stack_slots: Vec::new(),
    };

    let semantic = generator_resume_source_semantic(&layout);
    let closure_bindings = resume_closure_bindings(
        &semantic,
        &[
            "captured".to_string(),
            "total".to_string(),
            "_dp_pc".to_string(),
            "_dp_throw_context".to_string(),
        ],
    );

    assert_eq!(
        closure_bindings.runtime_state_bindings,
        vec![
            ("captured".to_string(), "_dp_cell_captured".to_string()),
            ("total".to_string(), "_dp_cell_total".to_string()),
            ("_dp_pc".to_string(), "_dp_cell__dp_pc".to_string()),
            (
                "_dp_throw_context".to_string(),
                "_dp_cell__dp_throw_context".to_string()
            ),
        ]
    );
}

#[test]
fn resume_closure_bindings_use_logical_names_for_shared_storage() {
    let layout = StorageLayout {
        freevars: vec![ClosureSlot {
            logical_name: "j".to_string(),
            storage_name: "_dp_cell_j".to_string(),
            init: ClosureInit::InheritedCapture,
        }],
        cellvars: vec![],
        runtime_cells: vec![
            ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            },
            ClosureSlot {
                logical_name: "_dp_throw_context".to_string(),
                storage_name: "_dp_cell__dp_throw_context".to_string(),
                init: ClosureInit::RuntimeNone,
            },
        ],
        stack_slots: Vec::new(),
    };

    let semantic = generator_resume_source_semantic(&layout);
    let closure_bindings = resume_closure_bindings(
        &semantic,
        &[
            "j".to_string(),
            "_dp_pc".to_string(),
            "_dp_throw_context".to_string(),
        ],
    );

    assert_eq!(
        closure_bindings.runtime_state_bindings,
        vec![
            ("j".to_string(), "_dp_cell_j".to_string()),
            ("_dp_pc".to_string(), "_dp_cell__dp_pc".to_string()),
            (
                "_dp_throw_context".to_string(),
                "_dp_cell__dp_throw_context".to_string()
            ),
        ]
    );
}

#[test]
fn resume_semantic_marks_generator_state_as_cell_captures() {
    let layout = StorageLayout {
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
        runtime_cells: vec![
            ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            },
            ClosureSlot {
                logical_name: "_dp_throw_context".to_string(),
                storage_name: "_dp_cell__dp_throw_context".to_string(),
                init: ClosureInit::RuntimeNone,
            },
        ],
        stack_slots: Vec::new(),
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
            None,
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
        semantic.binding_kind("_dp_throw_context"),
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

#[test]
fn resume_semantic_overlay_marks_runtime_and_logical_state_for_standard_name_binding() {
    let layout = StorageLayout {
        freevars: vec![ClosureSlot {
            logical_name: "captured".to_string(),
            storage_name: "_dp_cell_captured".to_string(),
            init: ClosureInit::InheritedCapture,
        }],
        cellvars: vec![
            ClosureSlot {
                logical_name: "total".to_string(),
                storage_name: "_dp_cell_total".to_string(),
                init: ClosureInit::Deferred,
            },
            ClosureSlot {
                logical_name: "_dp_eval_1".to_string(),
                storage_name: "_dp_cell__dp_eval_1".to_string(),
                init: ClosureInit::Deferred,
            },
            ClosureSlot {
                logical_name: "_dp_eval_2".to_string(),
                storage_name: "_dp_cell__dp_eval_2".to_string(),
                init: ClosureInit::Deferred,
            },
            ClosureSlot {
                logical_name: "_dp_try_exc_0".to_string(),
                storage_name: "_dp_cell__dp_try_exc_0".to_string(),
                init: ClosureInit::DeletedSentinel,
            },
        ],
        runtime_cells: vec![
            ClosureSlot {
                logical_name: "_dp_yieldfrom".to_string(),
                storage_name: "_dp_cell__dp_yieldfrom".to_string(),
                init: ClosureInit::RuntimeNone,
            },
            ClosureSlot {
                logical_name: "_dp_throw_context".to_string(),
                storage_name: "_dp_cell__dp_throw_context".to_string(),
                init: ClosureInit::RuntimeNone,
            },
            ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            },
        ],
        stack_slots: Vec::new(),
    };
    let semantic_for_bindings = generator_resume_source_semantic(&layout);
    let closure_bindings = resume_closure_bindings(
        &semantic_for_bindings,
        &[
            "captured".to_string(),
            "total".to_string(),
            "_dp_eval_1".to_string(),
            "_dp_eval_2".to_string(),
            "_dp_yieldfrom".to_string(),
            "_dp_throw_context".to_string(),
            "_dp_pc".to_string(),
            "_dp_try_exc_0".to_string(),
        ],
    );
    let mut semantic = BlockPyCallableSemanticInfo {
        names: FunctionName::new("gen_resume", "_dp_resume", "gen", "gen"),
        scope_kind: BlockPyCallableScopeKind::Function,
        ..Default::default()
    };

    augment_resume_semantic_for_standard_name_binding(&mut semantic, &closure_bindings);

    assert_eq!(
        semantic.binding_kind("total"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.binding_kind("_dp_pc"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("total"),
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
    );
    assert_eq!(semantic.cell_storage_name("total"), "total");
    assert_eq!(semantic.cell_capture_source_name("total"), "_dp_cell_total");
    assert_eq!(
        semantic.binding_kind("_dp_eval_1"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("_dp_eval_1"),
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
    );
    assert_eq!(semantic.cell_storage_name("_dp_eval_1"), "_dp_eval_1");
    assert_eq!(
        semantic.cell_capture_source_name("_dp_eval_1"),
        "_dp_cell__dp_eval_1"
    );
    assert_eq!(
        semantic.binding_kind("_dp_eval_2"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("_dp_eval_2"),
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
    );
    assert_eq!(semantic.cell_storage_name("_dp_eval_2"), "_dp_eval_2");
    assert_eq!(
        semantic.cell_capture_source_name("_dp_eval_2"),
        "_dp_cell__dp_eval_2"
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("_dp_pc"),
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
    );
    assert_eq!(semantic.cell_storage_name("_dp_pc"), "_dp_pc");
    assert_eq!(
        semantic.cell_capture_source_name("_dp_pc"),
        "_dp_cell__dp_pc"
    );
    assert_eq!(
        semantic.binding_kind("_dp_yieldfrom"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("_dp_yieldfrom"),
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
    );
    assert_eq!(semantic.cell_storage_name("_dp_yieldfrom"), "_dp_yieldfrom");
    assert_eq!(
        semantic.cell_capture_source_name("_dp_yieldfrom"),
        "_dp_cell__dp_yieldfrom"
    );
    assert_eq!(
        semantic.binding_kind("_dp_throw_context"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("_dp_throw_context"),
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
    );
    assert_eq!(
        semantic.cell_storage_name("_dp_throw_context"),
        "_dp_throw_context"
    );
    assert_eq!(
        semantic.cell_capture_source_name("_dp_throw_context"),
        "_dp_cell__dp_throw_context"
    );
    assert_eq!(
        semantic.binding_kind("_dp_try_exc_0"),
        Some(BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture))
    );
    assert_eq!(
        semantic.resolved_load_binding_kind("_dp_try_exc_0"),
        BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
    );
    assert_eq!(semantic.cell_storage_name("_dp_try_exc_0"), "_dp_try_exc_0");
    assert_eq!(
        semantic.cell_capture_source_name("_dp_try_exc_0"),
        "_dp_cell__dp_try_exc_0"
    );
}

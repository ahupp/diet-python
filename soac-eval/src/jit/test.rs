use super::*;

mod tests {
    use super::*;
    use dp_transform::block_py::{
        BlockPyRaise, BlockPyTerm, LocatedCoreBlockPyExpr, LocatedName, NameLocation,
    };
    use ruff_python_ast as ast;

    fn test_name(name: &str) -> LocatedName {
        LocatedName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: Default::default(),
            node_index: Default::default(),
            location: NameLocation::Local { slot: 0 },
        }
    }

    fn test_global_name(name: &str) -> LocatedName {
        LocatedName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: Default::default(),
            node_index: Default::default(),
            location: NameLocation::Global,
        }
    }

    fn test_closure_cell_name(name: &str, slot: u32) -> LocatedName {
        LocatedName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: Default::default(),
            node_index: Default::default(),
            location: NameLocation::ClosureCell { slot },
        }
    }

    fn test_captured_cell_source_name(name: &str, slot: u32) -> LocatedName {
        LocatedName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: Default::default(),
            node_index: Default::default(),
            location: NameLocation::CapturedCellSource { slot },
        }
    }

    fn test_term() -> BlockPyTerm<LocatedCoreBlockPyExpr> {
        BlockPyTerm::Raise(BlockPyRaise { exc: None })
    }

    #[test]
    fn render_specialized_jit_clif_smoke() {
        let blocks = [1usize as ObjPtr, 2usize as ObjPtr, 3usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec![],
            blocks: vec![
                ClifBlockPlan {
                    label: "b0".into(),
                    param_names: vec![],
                    runtime_param_names: vec![],
                    term: test_term(),
                    exc_target: None,
                    exc_dispatch: None,
                    fast_path: BlockFastPath::None,
                },
                ClifBlockPlan {
                    label: "b1".into(),
                    param_names: vec![],
                    runtime_param_names: vec![],
                    term: test_term(),
                    exc_target: None,
                    exc_dispatch: None,
                    fast_path: BlockFastPath::None,
                },
                ClifBlockPlan {
                    label: "b2".into(),
                    param_names: vec![],
                    runtime_param_names: vec![],
                    term: test_term(),
                    exc_target: None,
                    exc_dispatch: None,
                    fast_path: BlockFastPath::None,
                },
            ],
        };
        let err = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect_err("specialized JIT CLIF render should reject slow-path blocks");
        assert!(
            err.contains("fully lowered fastpath blocks"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn render_specialized_jit_operator_calls_use_python_capi() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec![],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Intrinsic {
                            intrinsic: &blockpy_intrinsics::ADD_INTRINSIC,
                            parts: vec![
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(1)),
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(2)),
                            ],
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call PyNumber_Add"),
            "operator lowering should use PyNumber_Add in rendered CLIF:\n{rendered}"
        );
        assert!(
            !rendered.contains("call dp_jit_py_call_positional_three"),
            "direct operator lowering should avoid generic Python helper calls:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_compare_calls_use_richcompare() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec![],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Intrinsic {
                            intrinsic: &blockpy_intrinsics::LT_INTRINSIC,
                            parts: vec![
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(1)),
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(2)),
                            ],
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call PyObject_RichCompare"),
            "comparison lowering should use PyObject_RichCompare in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_allocates_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec!["x".into(), "y".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Int(7),
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.matches("explicit_slot 8").count() >= 2,
            "slot-backed JIT plans should allocate explicit stack slots:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_assignments_sync_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec!["x".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleBlock {
                    plan: DirectSimpleBlockPlan {
                        params: vec![],
                        ops: vec![DirectSimpleOpPlan::Assign(DirectSimpleAssignPlan {
                            target: test_name("x"),
                            value: DirectSimpleExprPlan::Int(7),
                        })],
                        term: DirectSimpleTermPlan::Ret {
                            value: DirectSimpleExprPlan::Name(test_name("x")),
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("store.i64") || rendered.contains("stack_store"),
            "assignment-backed JIT plans should update mirrored function-state slots:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_global_names_use_global_lookup_hook() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec![],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Name(test_global_name("x")),
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_function_globals")
                && rendered.contains("call dp_jit_load_name"),
            "global located names should use callable-rooted globals lookup:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_load_global_intrinsic_uses_direct_helper() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec![],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Intrinsic {
                            intrinsic: &blockpy_intrinsics::LOAD_GLOBAL_INTRINSIC,
                            parts: vec![
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(1)),
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(2)),
                            ],
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_load_global_obj"),
            "load_global intrinsic should use the direct JIT helper:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_store_global_intrinsic_uses_direct_helper() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec![],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Intrinsic {
                            intrinsic: &blockpy_intrinsics::STORE_GLOBAL_INTRINSIC,
                            parts: vec![
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(1)),
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(2)),
                                DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(3)),
                            ],
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_store_global"),
            "store_global intrinsic should use the direct JIT helper:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_closure_names_use_function_closure_cells() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec!["x".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Name(test_closure_cell_name("x", 2)),
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_function_closure_cell")
                && rendered.contains("call dp_jit_load_cell"),
            "closure located names should load through callable-rooted closure cells:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_cell_ref_intrinsic_uses_function_closure_cells() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec!["x".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Intrinsic {
                            intrinsic: &blockpy_intrinsics::CELL_REF_INTRINSIC,
                            parts: vec![DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Name(
                                test_closure_cell_name("x", 2),
                            ))],
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_function_closure_cell"),
            "cell_ref intrinsic should use callable-rooted closure cells:\n{rendered}"
        );
        assert!(
            !rendered.contains("call dp_jit_load_cell"),
            "cell_ref intrinsic should return the cell object, not its contents:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_cell_ref_on_captured_source_unwraps_wrapper_cell_once() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec!["_dp_classcell".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Intrinsic {
                            intrinsic: &blockpy_intrinsics::CELL_REF_INTRINSIC,
                            parts: vec![DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Name(
                                test_captured_cell_source_name("_dp_classcell", 2),
                            ))],
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_function_closure_cell"),
            "captured cell sources should resolve through the callable closure:\n{rendered}"
        );
        assert!(
            !rendered.contains("call dp_jit_load_cell"),
            "__dp_cell_ref on a captured cell source should still return the raw cell object:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_delete_intrinsics_use_direct_helpers() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![],
            entry_param_names: vec![],
            entry_param_default_sources: vec![],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec!["cell".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleBlock {
                    plan: DirectSimpleBlockPlan {
                        params: vec![],
                        ops: vec![
                            DirectSimpleOpPlan::Expr(DirectSimpleExprPlan::Intrinsic {
                                intrinsic: &blockpy_intrinsics::DELITEM_INTRINSIC,
                                parts: vec![
                                    DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(1)),
                                    DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(2)),
                                ],
                            }),
                            DirectSimpleOpPlan::Expr(DirectSimpleExprPlan::Intrinsic {
                                intrinsic: &blockpy_intrinsics::DEL_QUIETLY_INTRINSIC,
                                parts: vec![
                                    DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(3)),
                                    DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Int(4)),
                                ],
                            }),
                            DirectSimpleOpPlan::Expr(DirectSimpleExprPlan::Intrinsic {
                                intrinsic: &blockpy_intrinsics::DEL_DEREF_INTRINSIC,
                                parts: vec![DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Name(
                                    test_name("cell"),
                                ))],
                            }),
                            DirectSimpleOpPlan::Expr(DirectSimpleExprPlan::Intrinsic {
                                intrinsic: &blockpy_intrinsics::DEL_DEREF_QUIETLY_INTRINSIC,
                                parts: vec![DirectSimpleCallPart::Pos(DirectSimpleExprPlan::Name(
                                    test_name("cell"),
                                ))],
                            }),
                        ],
                        term: DirectSimpleTermPlan::Ret {
                            value: DirectSimpleExprPlan::Int(0),
                        },
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_pyobject_delitem"),
            "delitem intrinsic should use the direct JIT helper:\n{rendered}"
        );
        assert!(
            rendered.contains("call dp_jit_del_quietly"),
            "del_quietly intrinsic should use the direct JIT helper:\n{rendered}"
        );
        assert!(
            rendered.contains("call dp_jit_del_deref"),
            "del_deref intrinsic should use the direct JIT helper:\n{rendered}"
        );
        assert!(
            rendered.contains("call dp_jit_del_deref_quietly"),
            "del_deref_quietly intrinsic should use the direct JIT helper:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_direct_entry_uses_live_positional_defaults() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![
                ClifBindingParam {
                    name: "x".into(),
                    kind: ClifBindingParamKind::PositionalOrKeyword,
                    has_default: false,
                },
                ClifBindingParam {
                    name: "y".into(),
                    kind: ClifBindingParamKind::PositionalOrKeyword,
                    has_default: true,
                },
            ],
            entry_param_names: vec!["x".into(), "y".into()],
            entry_param_default_sources: vec![
                None,
                Some(ClifEntryParamDefaultSource::Positional(0)),
            ],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec!["x".into(), "y".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Name(test_name("y")),
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_function_positional_default"),
            "direct entry lowering should source omitted positional defaults from the callable:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_direct_entry_uses_live_kwonly_defaults() {
        let blocks = [1usize as ObjPtr];
        let plan = ClifPlan {
            entry_params: vec![ClifBindingParam {
                name: "x".into(),
                kind: ClifBindingParamKind::KeywordOnly,
                has_default: true,
            }],
            entry_param_names: vec!["x".into()],
            entry_param_default_sources: vec![Some(ClifEntryParamDefaultSource::KeywordOnly(
                "x".into(),
            ))],
            ambient_param_names: vec![],
            owned_cell_slot_names: vec![],
            slot_names: vec!["x".into()],
            blocks: vec![ClifBlockPlan {
                label: "b0".into(),
                param_names: vec![],
                runtime_param_names: vec![],
                term: test_term(),
                exc_target: None,
                exc_dispatch: None,
                fast_path: BlockFastPath::DirectSimpleRet {
                    plan: DirectSimpleRetPlan {
                        params: vec![],
                        assigns: vec![],
                        ret: DirectSimpleExprPlan::Name(test_name("x")),
                    },
                },
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_with_cfg(
                &blocks,
                &plan,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("call dp_jit_function_kwonly_default"),
            "direct entry lowering should source omitted kwonly defaults from the callable:\n{rendered}"
        );
    }
}

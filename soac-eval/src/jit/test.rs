use super::*;

mod tests {
    use super::*;
    use dp_transform::block_py::{BlockPyRaise, BlockPyTerm, LocatedCoreBlockPyExpr};

    fn test_term() -> BlockPyTerm<LocatedCoreBlockPyExpr> {
        BlockPyTerm::Raise(BlockPyRaise { exc: None })
    }

    #[test]
    fn render_specialized_jit_clif_smoke() {
        let blocks = [1usize as ObjPtr, 2usize as ObjPtr, 3usize as ObjPtr];
        let plan = ClifPlan {
            ambient_param_names: vec![],
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
            ambient_param_names: vec![],
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
            ambient_param_names: vec![],
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
            ambient_param_names: vec![],
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
            ambient_param_names: vec![],
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
                            target: "x".into(),
                            value: DirectSimpleExprPlan::Int(7),
                        })],
                        term: DirectSimpleTermPlan::Ret {
                            value: DirectSimpleExprPlan::Name("x".into()),
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
}

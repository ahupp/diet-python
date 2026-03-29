use super::*;
use dp_transform::block_py::{
    BinOp, BinOpKind, BlockPyAssign, BlockPyDelete, BlockPyFunction, BlockPyStmt, BlockPyTerm,
    ClosureInit, ClosureLayout, ClosureSlot, CodegenBlockPyExpr, CodegenBlockPyLiteral,
    CoreBytesLiteral, CoreNumberLiteral, CoreNumberLiteralValue, DelDeref, DelDerefQuietly,
    DelItem, DelQuietly, FunctionName, LoadGlobal, LocatedCodegenBlockPyExpr, LocatedName,
    MakeString, ModuleNameGen, NameLocation, Operation, Param, ParamKind, ParamSpec, StoreGlobal,
    TernaryOp, TernaryOpKind,
};
use dp_transform::passes::CodegenBlockPyPass;
mod tests {
    use super::*;
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

    fn int_expr(value: i64) -> LocatedCodegenBlockPyExpr {
        let value_str = value.to_string();
        CodegenBlockPyExpr::Literal(CodegenBlockPyLiteral::NumberLiteral(CoreNumberLiteral {
            node_index: Default::default(),
            range: Default::default(),
            value: CoreNumberLiteralValue::Int(
                ast::Int::from_str_radix(value_str.as_str(), 10, value_str.as_str())
                    .expect("test integer literal should parse"),
            ),
        }))
    }

    fn bytes_expr(value: &[u8]) -> LocatedCodegenBlockPyExpr {
        CodegenBlockPyExpr::Literal(CodegenBlockPyLiteral::BytesLiteral(CoreBytesLiteral {
            node_index: Default::default(),
            range: Default::default(),
            value: value.to_vec(),
        }))
    }

    fn name_expr(name: LocatedName) -> LocatedCodegenBlockPyExpr {
        CodegenBlockPyExpr::Name(name)
    }

    fn op_expr(operation: Operation<LocatedCodegenBlockPyExpr>) -> LocatedCodegenBlockPyExpr {
        CodegenBlockPyExpr::Op(Box::new(operation))
    }

    fn expr_stmt(
        expr: LocatedCodegenBlockPyExpr,
    ) -> BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName> {
        BlockPyStmt::Expr(expr)
    }

    fn assign_stmt(
        target: LocatedName,
        value: LocatedCodegenBlockPyExpr,
    ) -> BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName> {
        BlockPyStmt::Assign(BlockPyAssign { target, value })
    }

    fn delete_stmt(target: LocatedName) -> BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName> {
        BlockPyStmt::Delete(BlockPyDelete { target })
    }

    fn ret_term(value: LocatedCodegenBlockPyExpr) -> BlockPyTerm<LocatedCodegenBlockPyExpr> {
        BlockPyTerm::Return(value)
    }

    fn raise_term() -> BlockPyTerm<LocatedCodegenBlockPyExpr> {
        BlockPyTerm::Raise(dp_transform::block_py::BlockPyRaise { exc: None })
    }

    fn test_block(
        label: &str,
        ops: Vec<BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName>>,
        term: BlockPyTerm<LocatedCodegenBlockPyExpr>,
    ) -> SpecializedJitBlockData {
        SpecializedJitBlockData {
            label: label.into(),
            full_param_names: vec![],
            runtime_param_names: vec![],
            exc_dispatch: None,
            ops,
            term,
        }
    }

    fn test_function() -> BlockPyFunction<CodegenBlockPyPass> {
        let mut module_name_gen = ModuleNameGen::new(0);
        let name_gen = module_name_gen.next_function_name_gen();
        BlockPyFunction {
            function_id: name_gen.function_id(),
            name_gen,
            names: FunctionName::new("test", "test", "test", "test"),
            kind: dp_transform::block_py::BlockPyFunctionKind::Function,
            params: ParamSpec::default(),
            blocks: vec![],
            doc: None,
            closure_layout: None,
            semantic: Default::default(),
        }
    }

    fn test_jit_data(blocks: Vec<SpecializedJitBlockData>) -> SpecializedJitData {
        SpecializedJitData {
            function: test_function(),
            function_state_slot_names: vec![],
            blocks,
        }
    }

    fn test_single_block_data(
        ops: Vec<BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName>>,
        term: BlockPyTerm<LocatedCodegenBlockPyExpr>,
    ) -> SpecializedJitData {
        test_jit_data(vec![test_block("b0", ops, term)])
    }

    fn render_test_jit_data(jit_data: &SpecializedJitData, blocks: &[ObjPtr]) -> String {
        unsafe {
            render_cranelift_run_bb_specialized_data_with_cfg(
                blocks,
                jit_data,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif
    }

    #[test]
    fn render_specialized_jit_clif_smoke() {
        let blocks = [1usize as ObjPtr, 2usize as ObjPtr, 3usize as ObjPtr];
        let jit_data = test_jit_data(vec![
            test_block("b0", vec![], raise_term()),
            test_block("b1", vec![], raise_term()),
            test_block("b2", vec![], raise_term()),
        ]);
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("function"),
            "specialized JIT CLIF render should produce function text:\n{}",
            rendered
        );
    }

    #[test]
    fn render_specialized_jit_clif_annotates_block_headers_with_named_typed_params() {
        let blocks = [1usize as ObjPtr];
        let jit_data = SpecializedJitData {
            function: test_function(),
            function_state_slot_names: vec!["current".into(), "acc".into()],
            blocks: vec![SpecializedJitBlockData {
                label: "loop_body".into(),
                full_param_names: vec!["current".into(), "acc".into()],
                runtime_param_names: vec!["current".into(), "acc".into()],
                ops: vec![],
                term: ret_term(int_expr(7)),
                exc_dispatch: None,
            }],
        };
        let rendered = unsafe {
            render_cranelift_run_bb_specialized_data_with_cfg(
                &blocks,
                &jit_data,
                11usize as ObjPtr,
                12usize as ObjPtr,
                13usize as ObjPtr,
                14usize as ObjPtr,
            )
        }
        .expect("specialized JIT CLIF render should succeed")
        .clif;
        assert!(
            rendered.contains("; block jit_entry(callable: i64)"),
            "rendered CLIF should include named typed params on surviving post-opt block headers:\n{rendered}"
        );
        assert!(
            rendered.contains("; block loop_body()"),
            "rendered CLIF should still surface the semantic name for optimized blocks:\n{rendered}"
        );
        assert!(
            rendered.contains("block0(v0: i64):"),
            "rendered CLIF should keep the real Cranelift block header for round-tripping:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_operator_calls_use_python_capi() {
        let blocks = [1usize as ObjPtr];
        let jit_data = test_single_block_data(
            vec![],
            ret_term(op_expr(Operation::BinOp(BinOp {
                node_index: Default::default(),
                range: Default::default(),
                kind: BinOpKind::Add,
                arg0: int_expr(1),
                arg1: int_expr(2),
            }))),
        );
        let rendered = render_test_jit_data(&jit_data, &blocks);
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
        let jit_data = test_single_block_data(
            vec![],
            ret_term(op_expr(Operation::BinOp(BinOp {
                node_index: Default::default(),
                range: Default::default(),
                kind: BinOpKind::Lt,
                arg0: int_expr(1),
                arg1: int_expr(2),
            }))),
        );
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call PyObject_RichCompare"),
            "comparison lowering should use PyObject_RichCompare in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_make_string_uses_decode_helper_directly() {
        let blocks = [1usize as ObjPtr];
        let jit_data = test_single_block_data(
            vec![],
            ret_term(op_expr(Operation::MakeString(MakeString {
                node_index: Default::default(),
                range: Default::default(),
                arg0: bytes_expr(b"hello"),
            }))),
        );
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call dp_jit_decode_literal_bytes"),
            "MakeString lowering should use the direct decode helper:\n{rendered}"
        );
        assert!(
            !rendered.contains("call dp_jit_py_call_positional_one"),
            "MakeString lowering should avoid generic Python helper calls:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_pow_calls_use_pynumber_power() {
        let blocks = [1usize as ObjPtr];
        let jit_data = test_single_block_data(
            vec![],
            ret_term(op_expr(Operation::TernaryOp(TernaryOp {
                node_index: Default::default(),
                range: Default::default(),
                kind: TernaryOpKind::Pow,
                arg0: int_expr(2),
                arg1: int_expr(3),
                arg2: name_expr(test_global_name("__dp_NONE")),
            }))),
        );
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call PyNumber_Power"),
            "power lowering should use PyNumber_Power in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_allocates_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut jit_data = test_single_block_data(vec![], ret_term(int_expr(7)));
        jit_data.function_state_slot_names = vec!["x".into(), "y".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.matches("explicit_slot 8").count() >= 2,
            "slot-backed JIT plans should allocate explicit stack slots:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_assignments_sync_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut jit_data = test_single_block_data(
            vec![assign_stmt(test_name("x"), int_expr(7))],
            ret_term(name_expr(test_name("x"))),
        );
        jit_data.function_state_slot_names = vec!["x".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("store.i64") || rendered.contains("stack_store"),
            "assignment-backed JIT plans should update mirrored function-state slots:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_global_names_use_global_lookup_hook() {
        let blocks = [1usize as ObjPtr];
        let jit_data = test_single_block_data(vec![], ret_term(name_expr(test_global_name("x"))));
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call dp_jit_function_globals")
                && rendered.contains("call dp_jit_load_name"),
            "global located names should use callable-rooted globals lookup:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_load_global_intrinsic_uses_direct_helper() {
        let blocks = [1usize as ObjPtr];
        let jit_data = test_single_block_data(
            vec![],
            ret_term(op_expr(Operation::LoadGlobal(LoadGlobal {
                node_index: Default::default(),
                range: Default::default(),
                arg0: int_expr(1),
                arg1: int_expr(2),
            }))),
        );
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call dp_jit_load_global_obj"),
            "load_global intrinsic should use the direct JIT helper:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_store_global_intrinsic_uses_direct_helper() {
        let blocks = [1usize as ObjPtr];
        let jit_data = test_single_block_data(
            vec![],
            ret_term(op_expr(Operation::StoreGlobal(StoreGlobal {
                node_index: Default::default(),
                range: Default::default(),
                arg0: int_expr(1),
                arg1: int_expr(2),
                arg2: int_expr(3),
            }))),
        );
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call dp_jit_store_global"),
            "store_global intrinsic should use the direct JIT helper:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_closure_names_use_function_closure_cells() {
        let blocks = [1usize as ObjPtr];
        let mut jit_data =
            test_single_block_data(vec![], ret_term(name_expr(test_closure_cell_name("x", 2))));
        jit_data.function_state_slot_names = vec!["x".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call dp_jit_function_closure_cell")
                && rendered.contains("call dp_jit_load_cell"),
            "closure located names should load through callable-rooted closure cells:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_cell_ref_intrinsic_uses_function_closure_cells() {
        let blocks = [1usize as ObjPtr];
        let mut jit_data = test_single_block_data(
            vec![],
            ret_term(op_expr(Operation::CellRef(
                dp_transform::block_py::CellRef {
                    node_index: Default::default(),
                    range: Default::default(),
                    arg0: name_expr(test_closure_cell_name("x", 2)),
                },
            ))),
        );
        jit_data.function_state_slot_names = vec!["x".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
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
        let mut jit_data = test_single_block_data(
            vec![],
            ret_term(op_expr(Operation::CellRef(
                dp_transform::block_py::CellRef {
                    node_index: Default::default(),
                    range: Default::default(),
                    arg0: name_expr(test_captured_cell_source_name("_dp_classcell", 2)),
                },
            ))),
        );
        jit_data.function.closure_layout = Some(ClosureLayout {
            freevars: vec![
                ClosureSlot {
                    logical_name: "_dp_classcell".into(),
                    storage_name: "_dp_classcell".into(),
                    init: ClosureInit::InheritedCapture,
                },
                ClosureSlot {
                    logical_name: "__unused".into(),
                    storage_name: "__unused".into(),
                    init: ClosureInit::InheritedCapture,
                },
                ClosureSlot {
                    logical_name: "_dp_classcell".into(),
                    storage_name: "_dp_classcell".into(),
                    init: ClosureInit::InheritedCapture,
                },
            ],
            cellvars: vec![],
            runtime_cells: vec![],
        });
        jit_data.function_state_slot_names = vec!["_dp_classcell".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
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
        let mut jit_data = test_single_block_data(
            vec![
                expr_stmt(op_expr(Operation::DelItem(DelItem {
                    node_index: Default::default(),
                    range: Default::default(),
                    arg0: int_expr(1),
                    arg1: int_expr(2),
                }))),
                expr_stmt(op_expr(Operation::DelQuietly(DelQuietly {
                    node_index: Default::default(),
                    range: Default::default(),
                    arg0: int_expr(3),
                    arg1: int_expr(4),
                }))),
                expr_stmt(op_expr(Operation::DelDeref(DelDeref {
                    node_index: Default::default(),
                    range: Default::default(),
                    arg0: name_expr(test_name("cell")),
                }))),
                expr_stmt(op_expr(Operation::DelDerefQuietly(DelDerefQuietly {
                    node_index: Default::default(),
                    range: Default::default(),
                    arg0: name_expr(test_name("cell")),
                }))),
            ],
            ret_term(int_expr(0)),
        );
        jit_data.function_state_slot_names = vec!["cell".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
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
        let mut jit_data = test_single_block_data(vec![], ret_term(name_expr(test_name("y"))));
        jit_data.function.params = ParamSpec {
            params: vec![
                Param {
                    name: "x".into(),
                    kind: ParamKind::Any,
                    has_default: false,
                },
                Param {
                    name: "y".into(),
                    kind: ParamKind::Any,
                    has_default: true,
                },
            ],
        };
        jit_data.function_state_slot_names = vec!["x".into(), "y".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call dp_jit_function_positional_default"),
            "direct entry lowering should source omitted positional defaults from the callable:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_direct_entry_uses_live_kwonly_defaults() {
        let blocks = [1usize as ObjPtr];
        let mut jit_data = test_single_block_data(vec![], ret_term(name_expr(test_name("x"))));
        jit_data.function.params = ParamSpec {
            params: vec![Param {
                name: "x".into(),
                kind: ParamKind::KwOnly,
                has_default: true,
            }],
        };
        jit_data.function_state_slot_names = vec!["x".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("call dp_jit_function_kwonly_default"),
            "direct entry lowering should source omitted kwonly defaults from the callable:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_delete_stmt_updates_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut jit_data =
            test_single_block_data(vec![delete_stmt(test_name("x"))], ret_term(int_expr(0)));
        jit_data.function_state_slot_names = vec!["x".into()];
        let rendered = render_test_jit_data(&jit_data, &blocks);
        assert!(
            rendered.contains("store.i64")
                || rendered.contains("stack_store")
                || rendered.contains("store notrap"),
            "delete-backed JIT plans should update mirrored function-state slots:\n{rendered}"
        );
    }
}

use super::*;
use soac_blockpy::block_py::{
    BinOp, BinOpKind, BlockParamRole, BlockPyAssign, BlockPyDelete, BlockPyFunction, BlockPyStmt,
    BlockPyTerm, CellLocation, ClosureInit, ClosureSlot, CodegenBlock, CodegenBlockPyExpr,
    CodegenBlockPyLiteral, CoreBytesLiteral, CoreNumberLiteral, CoreNumberLiteralValue, DelDeref,
    DelDerefQuietly, DelItem, DelQuietly, FunctionName, LoadGlobal, LocatedCodegenBlockPyExpr,
    LocatedName, MakeString, Meta, ModuleNameGen, NameLocation, Operation, Param, ParamKind,
    ParamSpec, StorageLayout, StoreGlobal, TernaryOp, TernaryOpKind, WithMeta,
};
use soac_blockpy::passes::CodegenBlockPyPass;
mod tests {
    use super::*;
    use ruff_python_ast as ast;

    fn test_name(name: &str) -> LocatedName {
        LocatedName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: Default::default(),
            node_index: Default::default(),
            location: NameLocation::local(0),
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
            location: NameLocation::closure_cell(slot),
        }
    }

    fn test_captured_cell_source_name(name: &str, slot: u32) -> LocatedName {
        LocatedName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: Default::default(),
            node_index: Default::default(),
            location: NameLocation::captured_source_cell(slot),
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
        CodegenBlockPyExpr::Op(operation)
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
        BlockPyTerm::Raise(soac_blockpy::block_py::BlockPyRaise { exc: None })
    }

    fn test_source_block(
        function: &BlockPyFunction<CodegenBlockPyPass>,
        ops: Vec<BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName>>,
        term: BlockPyTerm<LocatedCodegenBlockPyExpr>,
    ) -> CodegenBlock {
        CodegenBlock {
            label: function.name_gen.next_block_name(),
            body: ops,
            term,
            params: vec![],
            exc_edge: None,
        }
    }

    fn test_function() -> BlockPyFunction<CodegenBlockPyPass> {
        let mut module_name_gen = ModuleNameGen::new(0);
        let name_gen = module_name_gen.next_function_name_gen();
        BlockPyFunction {
            function_id: name_gen.function_id(),
            name_gen,
            names: FunctionName::new("test", "test", "test", "test"),
            kind: soac_blockpy::block_py::BlockPyFunctionKind::Function,
            params: ParamSpec::default(),
            blocks: vec![],
            doc: None,
            storage_layout: None,
            semantic: Default::default(),
        }
    }

    fn with_test_blocks(
        mut function: BlockPyFunction<CodegenBlockPyPass>,
        blocks: Vec<CodegenBlock>,
    ) -> BlockPyFunction<CodegenBlockPyPass> {
        function.blocks = blocks;
        function
    }

    fn set_stack_slots(function: &mut BlockPyFunction<CodegenBlockPyPass>, names: &[&str]) {
        function
            .storage_layout
            .get_or_insert_with(StorageLayout::default)
            .set_stack_slots(names.iter().map(|name| (*name).to_string()).collect());
    }

    fn with_single_test_block(
        function: BlockPyFunction<CodegenBlockPyPass>,
        ops: Vec<BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName>>,
        term: BlockPyTerm<LocatedCodegenBlockPyExpr>,
    ) -> BlockPyFunction<CodegenBlockPyPass> {
        let block = test_source_block(&function, ops, term);
        with_test_blocks(function, vec![block])
    }

    fn render_test_jit_function(
        function: &BlockPyFunction<CodegenBlockPyPass>,
        blocks: &[ObjPtr],
    ) -> String {
        unsafe {
            let mut builder = new_jit_builder().expect("test jit builder should construct");
            register_specialized_jit_symbols(&mut builder);
            let mut jit_module = JITModule::new(builder);
            let module_constants =
                crate::module_constants::ModuleCodegenConstants::collect_from_functions([function]);
            let built = build_cranelift_run_bb_specialized_function(
                &mut jit_module,
                blocks,
                function,
                &module_constants,
            )
            .expect("specialized JIT build should succeed");
            let (clif, _cfg_dot, _vcode_disasm) = render_compiled_clif_and_vcode_disasm(
                &mut jit_module,
                built.ctx,
                &built.import_id_to_symbol,
                &built.block_annotations,
            )
            .expect("specialized JIT CLIF render should succeed");
            clif
        }
    }

    #[test]
    fn render_specialized_jit_clif_smoke() {
        let blocks = [1usize as ObjPtr, 2usize as ObjPtr, 3usize as ObjPtr];
        let function = test_function();
        let function = with_test_blocks(
            function.clone(),
            vec![
                test_source_block(&function, vec![], raise_term()),
                test_source_block(&function, vec![], raise_term()),
                test_source_block(&function, vec![], raise_term()),
            ],
        );
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("function"),
            "specialized JIT CLIF render should produce function text:\n{}",
            rendered
        );
    }

    #[test]
    fn render_specialized_jit_clif_annotates_block_headers_with_named_typed_params() {
        let blocks = [1usize as ObjPtr];
        let mut function = test_function();
        set_stack_slots(&mut function, &["current", "acc"]);
        let mut source = test_source_block(&function, vec![], ret_term(int_expr(7)));
        source.ensure_param("current", BlockParamRole::AbruptKind);
        source.ensure_param("acc", BlockParamRole::AbruptPayload);
        let function = with_test_blocks(function, vec![source]);
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("; block jit_entry(vmctx: i64, callable: i64)"),
            "rendered CLIF should include named typed params on surviving post-opt block headers:\n{rendered}"
        );
        assert!(
            rendered.contains("; block bb0()"),
            "rendered CLIF should still surface the semantic name for optimized blocks:\n{rendered}"
        );
        assert!(
            rendered.contains("block0(v0: i64, v1: i64):"),
            "rendered CLIF should keep the real Cranelift block header for round-tripping:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_operator_calls_use_python_capi() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Operation::new(BinOp::new(BinOpKind::Add, int_expr(1), int_expr(2)))
                    .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function(&function, &blocks);
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
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Operation::new(BinOp::new(BinOpKind::Lt, int_expr(1), int_expr(2)))
                    .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("call PyObject_RichCompare"),
            "comparison lowering should use PyObject_RichCompare in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_make_string_uses_module_constant_loader() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Operation::new(MakeString::new(b"hello".to_vec())).with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("call dp_jit_load_module_constant"),
            "MakeString lowering should load a module constant:\n{rendered}"
        );
        assert!(
            !rendered.contains("call dp_jit_decode_literal_bytes"),
            "MakeString lowering should not decode literal bytes directly anymore:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_pow_calls_use_pynumber_power() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Operation::new(TernaryOp::new(
                    TernaryOpKind::Pow,
                    int_expr(2),
                    int_expr(3),
                    name_expr(test_global_name("__dp_NONE")),
                ))
                .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("call PyNumber_Power"),
            "power lowering should use PyNumber_Power in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_allocates_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut function = with_single_test_block(test_function(), vec![], ret_term(int_expr(7)));
        set_stack_slots(&mut function, &["x", "y"]);
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.matches("explicit_slot 8").count() >= 2,
            "slot-backed JIT plans should allocate explicit stack slots:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_assignments_sync_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut function = with_single_test_block(
            test_function(),
            vec![assign_stmt(test_name("x"), int_expr(7))],
            ret_term(name_expr(test_name("x"))),
        );
        set_stack_slots(&mut function, &["x"]);
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("store.i64") || rendered.contains("stack_store"),
            "assignment-backed JIT plans should update mirrored function-state slots:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_global_names_load_from_vmctx_globals() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(name_expr(test_global_name("x"))),
        );
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            !rendered.contains("call dp_jit_function_globals")
                && rendered.contains("call dp_jit_load_global_obj")
                && rendered.contains("call dp_jit_load_module_constant"),
            "global located names should load through vmctx-backed globals without a callable globals helper:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_load_global_intrinsic_uses_direct_helper() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Operation::new(LoadGlobal::new(int_expr(1), "x".to_string()))
                    .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("call dp_jit_load_global_obj"),
            "load_global intrinsic should use the direct JIT helper:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_store_global_intrinsic_uses_direct_helper() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Operation::new(StoreGlobal::new(int_expr(1), "x".to_string(), int_expr(3)))
                    .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("call dp_jit_store_global"),
            "store_global intrinsic should use the direct JIT helper:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_closure_names_use_function_closure_cells() {
        let blocks = [1usize as ObjPtr];
        let mut function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(name_expr(test_closure_cell_name("x", 2))),
        );
        set_stack_slots(&mut function, &["x"]);
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("call dp_jit_function_closure_cell")
                && rendered.contains("call dp_jit_load_cell"),
            "closure located names should load through callable-rooted closure cells:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_cell_ref_intrinsic_uses_function_closure_cells() {
        let blocks = [1usize as ObjPtr];
        let mut function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Operation::new(soac_blockpy::block_py::CellRef::new(CellLocation::Closure(
                    2,
                )))
                .with_meta(Meta::synthetic()),
            )),
        );
        set_stack_slots(&mut function, &["x"]);
        let rendered = render_test_jit_function(&function, &blocks);
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
        let mut function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Operation::new(soac_blockpy::block_py::CellRef::new(
                    CellLocation::CapturedSource(2),
                ))
                .with_meta(Meta::synthetic()),
            )),
        );
        function.storage_layout = Some(StorageLayout {
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
            stack_slots: Vec::new(),
        });
        set_stack_slots(&mut function, &["_dp_classcell"]);
        let rendered = render_test_jit_function(&function, &blocks);
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
        let mut function = with_single_test_block(
            test_function(),
            vec![
                expr_stmt(op_expr(
                    Operation::new(DelItem::new(int_expr(1), int_expr(2)))
                        .with_meta(Meta::synthetic()),
                )),
                expr_stmt(op_expr(
                    Operation::new(DelQuietly::new(int_expr(3), "x".to_string()))
                        .with_meta(Meta::synthetic()),
                )),
                expr_stmt(op_expr(
                    Operation::new(DelDeref::new(CellLocation::Closure(2)))
                        .with_meta(Meta::synthetic()),
                )),
                expr_stmt(op_expr(
                    Operation::new(DelDerefQuietly::new(CellLocation::Closure(2)))
                        .with_meta(Meta::synthetic()),
                )),
            ],
            ret_term(int_expr(0)),
        );
        set_stack_slots(&mut function, &["cell"]);
        let rendered = render_test_jit_function(&function, &blocks);
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
        let mut function =
            with_single_test_block(test_function(), vec![], ret_term(name_expr(test_name("y"))));
        function.params = ParamSpec {
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
        set_stack_slots(&mut function, &["x", "y"]);
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("call dp_jit_function_positional_default_obj"),
            "direct entry lowering should source omitted positional defaults from the callable:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_direct_entry_uses_live_kwonly_defaults() {
        let blocks = [1usize as ObjPtr];
        let mut function =
            with_single_test_block(test_function(), vec![], ret_term(name_expr(test_name("x"))));
        function.params = ParamSpec {
            params: vec![Param {
                name: "x".into(),
                kind: ParamKind::KwOnly,
                has_default: true,
            }],
        };
        set_stack_slots(&mut function, &["x"]);
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("call dp_jit_function_kwonly_default_obj"),
            "direct entry lowering should source omitted kwonly defaults from the callable:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_delete_stmt_updates_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut function = with_single_test_block(
            test_function(),
            vec![delete_stmt(test_name("x"))],
            ret_term(int_expr(0)),
        );
        set_stack_slots(&mut function, &["x"]);
        let rendered = render_test_jit_function(&function, &blocks);
        assert!(
            rendered.contains("store.i64")
                || rendered.contains("stack_store")
                || rendered.contains("store notrap"),
            "delete-backed JIT plans should update mirrored function-state slots:\n{rendered}"
        );
    }
}

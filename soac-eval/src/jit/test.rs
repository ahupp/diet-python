use super::*;
use soac_blockpy::block_py::{
    BinOp, BinOpKind, BlockParamRole, BlockPyFunction, BlockPyLiteral, BlockPyModule, BlockPyStmt,
    BlockPyTerm, Call, CellLocation, ClosureInit, ClosureSlot, CodegenBlock, CodegenBlockPyExpr,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBytesLiteral, CoreNumberLiteral,
    CoreNumberLiteralValue, CoreStringLiteral, Del, DelItem, FunctionName, HasMeta, InstrExprNode,
    Load, LocatedCodegenBlockPyExpr, LocatedCoreBlockPyExpr, LocatedName, Meta, ModuleNameGen,
    NameLocation, Param, ParamKind, ParamSpec, StorageLayout, Store, WithMeta,
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

    fn test_runtime_name(name: &str) -> LocatedName {
        LocatedName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: Default::default(),
            node_index: Default::default(),
            location: NameLocation::RuntimeName,
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

    fn test_constant_name(index: u32) -> LocatedName {
        LocatedName {
            id: "__dp_constant".into(),
            ctx: ast::ExprContext::Load,
            range: Default::default(),
            node_index: Default::default(),
            location: NameLocation::Constant(index),
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

    fn int_literal(value: i64) -> LocatedCoreBlockPyExpr {
        let value_str = value.to_string();
        CoreBlockPyExpr::Literal(BlockPyLiteral::NumberLiteral(CoreNumberLiteral {
            node_index: Default::default(),
            range: Default::default(),
            value: CoreNumberLiteralValue::Int(
                ast::Int::from_str_radix(value_str.as_str(), 10, value_str.as_str())
                    .expect("test integer literal should parse"),
            ),
        }))
    }

    fn bytes_literal(value: &[u8]) -> LocatedCoreBlockPyExpr {
        CoreBlockPyExpr::Literal(BlockPyLiteral::BytesLiteral(CoreBytesLiteral {
            node_index: Default::default(),
            range: Default::default(),
            value: value.to_vec(),
        }))
    }

    fn string_literal(value: &str) -> LocatedCoreBlockPyExpr {
        CoreBlockPyExpr::Literal(BlockPyLiteral::StringLiteral(CoreStringLiteral {
            node_index: Default::default(),
            range: Default::default(),
            value: value.to_string(),
        }))
    }

    #[derive(Default)]
    struct TestConstantPool {
        module_constants: Vec<LocatedCoreBlockPyExpr>,
    }

    impl TestConstantPool {
        fn push_literal(&mut self, literal: LocatedCoreBlockPyExpr) -> LocatedCodegenBlockPyExpr {
            let meta = literal.meta();
            let index = u32::try_from(self.module_constants.len())
                .expect("test module constant count should fit in u32");
            self.module_constants.push(literal);
            Load::new(test_constant_name(index)).with_meta(meta).into()
        }

        fn int_expr(&mut self, value: i64) -> LocatedCodegenBlockPyExpr {
            self.push_literal(int_literal(value))
        }

        fn bytes_expr(&mut self, value: &[u8]) -> LocatedCodegenBlockPyExpr {
            self.push_literal(bytes_literal(value))
        }

        fn string_expr(&mut self, value: &str) -> LocatedCodegenBlockPyExpr {
            self.push_literal(string_literal(value))
        }
    }

    fn name_expr(name: LocatedName) -> LocatedCodegenBlockPyExpr {
        Load::new(name).with_meta(Meta::synthetic()).into()
    }

    fn op_expr(operation: impl Into<LocatedCodegenBlockPyExpr>) -> LocatedCodegenBlockPyExpr {
        operation.into()
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
        expr_stmt(op_expr(
            Store::new(target, value).with_meta(Meta::synthetic()),
        ))
    }

    fn delete_stmt(target: LocatedName) -> BlockPyStmt<LocatedCodegenBlockPyExpr, LocatedName> {
        expr_stmt(op_expr(
            Del::new(target, false).with_meta(Meta::synthetic()),
        ))
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

    #[derive(Default)]
    struct TestLiteralExtractor {
        module_constants: Vec<LocatedCodegenBlockPyExpr>,
    }

    impl TestLiteralExtractor {
        fn extract_function(&mut self, function: &mut BlockPyFunction<CodegenBlockPyPass>) {
            for block in &mut function.blocks {
                for stmt in &mut block.body {
                    match stmt {
                        BlockPyStmt::Expr(expr) => self.extract_expr(expr),
                        BlockPyStmt::_Marker(_) => unreachable!("marker stmt should not appear"),
                    }
                }
                match &mut block.term {
                    BlockPyTerm::Jump(_) => {}
                    BlockPyTerm::IfTerm(if_term) => self.extract_expr(&mut if_term.test),
                    BlockPyTerm::BranchTable(branch_table) => {
                        self.extract_expr(&mut branch_table.index)
                    }
                    BlockPyTerm::Raise(raise_stmt) => {
                        if let Some(exc) = &mut raise_stmt.exc {
                            self.extract_expr(exc);
                        }
                    }
                    BlockPyTerm::Return(value) => self.extract_expr(value),
                }
            }
        }

        fn extract_expr(&mut self, expr: &mut LocatedCodegenBlockPyExpr) {
            if matches!(expr, CodegenBlockPyExpr::Literal(_)) {
                let meta = expr.meta();
                let index = u32::try_from(self.module_constants.len())
                    .expect("test module constant count should fit in u32");
                let literal = std::mem::replace(
                    expr,
                    Load::new(test_constant_name(index)).with_meta(meta).into(),
                );
                self.module_constants.push(literal);
                return;
            }
            match expr {
                CodegenBlockPyExpr::Literal(_) => {}
                CodegenBlockPyExpr::BinOp(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::UnaryOp(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::Call(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::GetAttr(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::SetAttr(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::GetItem(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::SetItem(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::DelItem(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::Load(_) => {}
                CodegenBlockPyExpr::Store(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::Del(_) => {}
                CodegenBlockPyExpr::MakeCell(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
                CodegenBlockPyExpr::CellRefForName(_) => {}
                CodegenBlockPyExpr::CellRef(_) => {}
                CodegenBlockPyExpr::MakeFunction(op) => {
                    op.visit_exprs_mut(&mut |child| self.extract_expr(child))
                }
            }
        }
    }

    fn render_test_jit_function(
        function: &BlockPyFunction<CodegenBlockPyPass>,
        blocks: &[ObjPtr],
    ) -> String {
        render_test_jit_function_with_module_constants(function, blocks, Vec::new())
    }

    fn render_test_jit_function_with_module_constants(
        function: &BlockPyFunction<CodegenBlockPyPass>,
        blocks: &[ObjPtr],
        module_constants: Vec<LocatedCoreBlockPyExpr>,
    ) -> String {
        let module = BlockPyModule {
            callable_defs: vec![function.clone()],
            module_constants,
        };
        let module_constants =
            crate::module_constants::ModuleCodegenConstants::collect_from_module(&module);
        render_test_jit_function_with_constants(&function, blocks, &module_constants)
    }

    fn render_test_jit_function_with_constants(
        function: &BlockPyFunction<CodegenBlockPyPass>,
        blocks: &[ObjPtr],
        module_constants: &crate::module_constants::ModuleCodegenConstants,
    ) -> String {
        unsafe {
            let mut builder = new_jit_builder().expect("test jit builder should construct");
            register_specialized_jit_symbols(&mut builder);
            let mut jit_module = JITModule::new(builder);
            let built = build_cranelift_run_bb_specialized_function(
                &mut jit_module,
                blocks,
                function,
                module_constants,
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
        let mut constants = TestConstantPool::default();
        let mut function = test_function();
        set_stack_slots(&mut function, &["current", "acc"]);
        let mut source = test_source_block(&function, vec![], ret_term(constants.int_expr(7)));
        source.ensure_param("current", BlockParamRole::AbruptKind);
        source.ensure_param("acc", BlockParamRole::AbruptPayload);
        let function = with_test_blocks(function, vec![source]);
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
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
        let mut constants = TestConstantPool::default();
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                BinOp::new(BinOpKind::Add, constants.int_expr(1), constants.int_expr(2))
                    .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
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
        let mut constants = TestConstantPool::default();
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                BinOp::new(BinOpKind::Lt, constants.int_expr(1), constants.int_expr(2))
                    .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
        assert!(
            rendered.contains("call PyObject_RichCompare"),
            "comparison lowering should use PyObject_RichCompare in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_string_literals_use_module_constant_loader() {
        let blocks = [1usize as ObjPtr];
        let mut constants = TestConstantPool::default();
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(constants.string_expr("hello")),
        );
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
        assert!(
            rendered.contains("call dp_jit_load_module_constant"),
            "string literal lowering should load a module constant:\n{rendered}"
        );
        assert!(
            !rendered.contains("call dp_jit_decode_literal_bytes"),
            "string literal lowering should not decode literal bytes directly anymore:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_constant_locations_use_module_constant_loader() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Load::new(test_constant_name(0)).with_meta(Meta::synthetic()),
            )),
        );
        let module = BlockPyModule {
            callable_defs: vec![function.clone()],
            module_constants: vec![int_literal(7)],
        };
        let module_constants =
            crate::module_constants::ModuleCodegenConstants::collect_from_module(&module);
        let rendered =
            render_test_jit_function_with_constants(&function, &blocks, &module_constants);
        assert!(
            rendered.contains("call dp_jit_load_module_constant"),
            "constant slot lowering should load through the module constant table:\n{rendered}"
        );
    }

    fn render_specialized_jit_pow_calls_use_pynumber_power() {
        let blocks = [1usize as ObjPtr];
        let mut constants = TestConstantPool::default();
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                BinOp::new(BinOpKind::Pow, constants.int_expr(2), constants.int_expr(3))
                    .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
        assert!(
            rendered.contains("call PyNumber_Power"),
            "power lowering should use PyNumber_Power in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_inplace_pow_calls_use_pynumber_inplace_power() {
        let blocks = [1usize as ObjPtr];
        let mut constants = TestConstantPool::default();
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                BinOp::new(
                    BinOpKind::InplacePow,
                    constants.int_expr(2),
                    constants.int_expr(3),
                )
                .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
        assert!(
            rendered.contains("call PyNumber_InPlacePower"),
            "inplace power lowering should use PyNumber_InPlacePower in rendered CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_allocates_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut constants = TestConstantPool::default();
        let mut function =
            with_single_test_block(test_function(), vec![], ret_term(constants.int_expr(7)));
        set_stack_slots(&mut function, &["x", "y"]);
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
        assert!(
            rendered.matches("explicit_slot 8").count() >= 2,
            "slot-backed JIT plans should allocate explicit stack slots:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_assignments_sync_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut constants = TestConstantPool::default();
        let mut function = with_single_test_block(
            test_function(),
            vec![assign_stmt(test_name("x"), constants.int_expr(7))],
            ret_term(name_expr(test_name("x"))),
        );
        set_stack_slots(&mut function, &["x"]);
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
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
                Load::new(test_global_name("x")).with_meta(Meta::synthetic()),
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
        let mut constants = TestConstantPool::default();
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Store::new(test_global_name("x"), constants.int_expr(3))
                    .with_meta(Meta::synthetic()),
            )),
        );
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
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
                soac_blockpy::block_py::CellRef::new(CellLocation::Closure(2))
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
                soac_blockpy::block_py::CellRef::new(CellLocation::CapturedSource(2))
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
        let mut constants = TestConstantPool::default();
        let mut function = with_single_test_block(
            test_function(),
            vec![
                expr_stmt(op_expr(
                    DelItem::new(constants.int_expr(1), constants.int_expr(2))
                        .with_meta(Meta::synthetic()),
                )),
                expr_stmt(op_expr(
                    Del::new(test_global_name("x"), true).with_meta(Meta::synthetic()),
                )),
                expr_stmt(op_expr(
                    Del::new(test_closure_cell_name("cell", 2), false).with_meta(Meta::synthetic()),
                )),
                expr_stmt(op_expr(
                    Del::new(test_closure_cell_name("cell", 2), true).with_meta(Meta::synthetic()),
                )),
            ],
            ret_term(constants.int_expr(0)),
        );
        set_stack_slots(&mut function, &["cell"]);
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
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
    fn render_specialized_jit_deleted_name_checks_inline_the_sentinel_compare() {
        let blocks = [1usize as ObjPtr];
        let mut constants = TestConstantPool::default();
        let mut function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(
                Call::new(
                    name_expr(test_runtime_name("load_deleted_name")),
                    vec![
                        CoreBlockPyCallArg::Positional(constants.string_expr("x")),
                        CoreBlockPyCallArg::Positional(name_expr(test_name("x"))),
                    ],
                    vec![],
                )
                .with_meta(Meta::synthetic()),
            )),
        );
        set_stack_slots(&mut function, &["x"]);
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
        assert!(
            rendered.contains("call dp_jit_raise_deleted_name_error"),
            "deleted-name lowering should keep only the cold-path error helper:\n{rendered}"
        );
        assert!(
            !rendered.contains("call dp_jit_load_deleted_name_obj"),
            "deleted-name lowering should inline the DELETED sentinel check in CLIF:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_delete_stmt_updates_function_state_slots() {
        let blocks = [1usize as ObjPtr];
        let mut constants = TestConstantPool::default();
        let mut function = with_single_test_block(
            test_function(),
            vec![delete_stmt(test_name("x"))],
            ret_term(constants.int_expr(0)),
        );
        set_stack_slots(&mut function, &["x"]);
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            constants.module_constants,
        );
        assert!(
            rendered.contains("store.i64")
                || rendered.contains("stack_store")
                || rendered.contains("store notrap"),
            "delete-backed JIT plans should update mirrored function-state slots:\n{rendered}"
        );
    }
}

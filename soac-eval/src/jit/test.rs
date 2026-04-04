use super::*;
use soac_blockpy::block_py::{
    BinOp, BinOpKind, BlockParamRole, BlockPyFunction, BlockPyLiteral, BlockPyModule, BlockTerm,
    Call, CellLocation, ClosureInit, ClosureSlot, CodegenBlock, CodegenBlockPyExpr,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBytesLiteral, CoreNumberLiteral,
    CoreNumberLiteralValue, CoreStringLiteral, Del, DelItem, FunctionName, LiteralValue, Load,
    LocatedCoreBlockPyExpr, LocatedName, ModuleNameGen, NameLocation, Param, ParamKind, ParamSpec,
    StorageLayout, Store,
};
use soac_blockpy::passes::CodegenBlockPyPass;
mod tests {
    use super::*;
    use pyo3::{Python, ffi};
    use ruff_python_ast as ast;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    static CAPSULE_DESTROYED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn test_capsule_destructor(_capsule: *mut ffi::PyObject) {
        CAPSULE_DESTROYED.store(true, Ordering::SeqCst);
    }

    fn test_name(name: &str) -> LocatedName {
        LocatedName {
            id: name.into(),
            location: NameLocation::local(0),
        }
    }

    fn test_global_name(name: &str) -> LocatedName {
        LocatedName {
            id: name.into(),
            location: NameLocation::Global,
        }
    }

    fn test_runtime_name(name: &str) -> LocatedName {
        LocatedName {
            id: name.into(),
            location: NameLocation::RuntimeName,
        }
    }

    fn test_closure_cell_name(name: &str, slot: u32) -> LocatedName {
        LocatedName {
            id: name.into(),
            location: NameLocation::closure_cell(slot),
        }
    }

    fn test_constant_name(index: u32) -> LocatedName {
        LocatedName {
            id: "__dp_constant".into(),
            location: NameLocation::Constant(index),
        }
    }

    fn test_captured_cell_source_name(name: &str, slot: u32) -> LocatedName {
        LocatedName {
            id: name.into(),
            location: NameLocation::captured_source_cell(slot),
        }
    }

    fn int_literal(value: i64) -> LocatedCoreBlockPyExpr {
        let value_str = value.to_string();
        let literal = BlockPyLiteral::NumberLiteral(CoreNumberLiteral {
            value: CoreNumberLiteralValue::Int(
                ast::Int::from_str_radix(value_str.as_str(), 10, value_str.as_str())
                    .expect("test integer literal should parse"),
            ),
        });
        CoreBlockPyExpr::Literal(LiteralValue::new(literal))
    }

    fn bytes_literal(value: &[u8]) -> LocatedCoreBlockPyExpr {
        let literal = BlockPyLiteral::BytesLiteral(CoreBytesLiteral {
            value: value.to_vec(),
        });
        CoreBlockPyExpr::Literal(LiteralValue::new(literal))
    }

    fn string_literal(value: &str) -> LocatedCoreBlockPyExpr {
        let literal = BlockPyLiteral::StringLiteral(CoreStringLiteral {
            value: value.to_string(),
        });
        CoreBlockPyExpr::Literal(LiteralValue::new(literal))
    }

    #[derive(Default)]
    struct TestConstantPool {
        module_constants: Vec<LocatedCoreBlockPyExpr>,
    }

    impl TestConstantPool {
        fn push_literal(&mut self, literal: LocatedCoreBlockPyExpr) -> CodegenBlockPyExpr {
            let index = u32::try_from(self.module_constants.len())
                .expect("test module constant count should fit in u32");
            self.module_constants.push(literal);
            Load::new(test_constant_name(index)).into()
        }

        fn int_expr(&mut self, value: i64) -> CodegenBlockPyExpr {
            self.push_literal(int_literal(value))
        }

        fn bytes_expr(&mut self, value: &[u8]) -> CodegenBlockPyExpr {
            self.push_literal(bytes_literal(value))
        }

        fn string_expr(&mut self, value: &str) -> CodegenBlockPyExpr {
            self.push_literal(string_literal(value))
        }
    }

    fn name_expr(name: LocatedName) -> CodegenBlockPyExpr {
        Load::new(name).into()
    }

    fn op_expr(operation: impl Into<CodegenBlockPyExpr>) -> CodegenBlockPyExpr {
        operation.into()
    }

    fn expr_stmt(expr: CodegenBlockPyExpr) -> CodegenBlockPyExpr {
        expr
    }

    fn assign_stmt(target: LocatedName, value: CodegenBlockPyExpr) -> CodegenBlockPyExpr {
        expr_stmt(op_expr(Store::new(target, value)))
    }

    fn delete_stmt(target: LocatedName) -> CodegenBlockPyExpr {
        expr_stmt(op_expr(Del::new(target, false)))
    }

    fn ret_term(value: CodegenBlockPyExpr) -> BlockTerm<CodegenBlockPyExpr> {
        BlockTerm::Return(value)
    }

    fn raise_term() -> BlockTerm<CodegenBlockPyExpr> {
        BlockTerm::Raise(soac_blockpy::block_py::TermRaise { exc: None })
    }

    fn test_source_block(
        function: &BlockPyFunction<CodegenBlockPyPass>,
        ops: Vec<CodegenBlockPyExpr>,
        term: BlockTerm<CodegenBlockPyExpr>,
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
        let module_name_gen = ModuleNameGen::new(0);
        let name_gen = module_name_gen.next_function_name_gen();
        BlockPyFunction {
            function_id: name_gen.function_id(),
            name_gen,
            names: FunctionName::new("test", "test", "test", "test"),
            kind: soac_blockpy::block_py::FunctionKind::Function,
            params: ParamSpec::default(),
            blocks: vec![],
            doc: None,
            storage_layout: None,
            scope: Default::default(),
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
        ops: Vec<CodegenBlockPyExpr>,
        term: BlockTerm<CodegenBlockPyExpr>,
    ) -> BlockPyFunction<CodegenBlockPyPass> {
        let block = test_source_block(&function, ops, term);
        with_test_blocks(function, vec![block])
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
            module_name_gen: ModuleNameGen::new(0),
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
            let mut jit_module = new_jit_module().expect("test jit module should construct");
            let module_constant_ptrs = placeholder_module_constant_ptrs(module_constants.len());
            let built = build_cranelift_run_bb_specialized_function(
                &mut jit_module,
                blocks,
                function,
                module_constants,
                &module_constant_ptrs,
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

    fn vendored_python_home() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace crate should have a repo-root parent")
            .join("vendor")
            .join("cpython")
    }

    unsafe extern "C" fn test_bind_direct_args_stub(
        _callable: ObjPtr,
        _args: *const ObjPtr,
        _nargsf: usize,
        _kwnames: ObjPtr,
        _data_ptr: ObjPtr,
        _out_args: *mut ObjPtr,
        _out_len: i64,
    ) -> i32 {
        1
    }

    fn count_direct_calls_to_runtime_helpers(
        function: &ir::Function,
        helpers: &[ir::UserExternalName],
    ) -> usize {
        let mut count = 0usize;
        for block in function.layout.blocks() {
            for inst in function.layout.block_insts(block) {
                let callee = match function.dfg.insts[inst] {
                    ir::InstructionData::Call { func_ref, .. }
                    | ir::InstructionData::TryCall { func_ref, .. } => Some(func_ref),
                    _ => None,
                };
                let Some(callee) = callee else {
                    continue;
                };
                let ext_func = &function.dfg.ext_funcs[callee];
                let ir::ExternalName::User(name_ref) = &ext_func.name else {
                    continue;
                };
                let user_name = &function.params.user_named_funcs()[*name_ref];
                if helpers.contains(user_name) {
                    count += 1;
                }
            }
        }
        count
    }

    unsafe fn build_runtime_refcount_smoke_context() -> (
        JITModule,
        cranelift_codegen::Context,
        FuncId,
        [ir::UserExternalName; 2],
    ) {
        let mut jit_module = new_jit_module().expect("test jit module should construct");
        let ptr_ty = jit_module.target_config().pointer_type();

        let mut refcount_signature = jit_module.make_signature();
        refcount_signature.params.push(ir::AbiParam::new(ptr_ty));

        let mut wrapper_signature = jit_module.make_signature();
        wrapper_signature.params.push(ir::AbiParam::new(ptr_ty));
        wrapper_signature.returns.push(ir::AbiParam::new(ptr_ty));

        let wrapper_id = declare_local_fn(
            &mut jit_module,
            "jit_runtime_support_smoke_wrapper",
            &wrapper_signature,
        )
        .expect("wrapper function should declare");
        let incref_id = declare_local_fn(
            &mut jit_module,
            SOAC_RUNTIME_INCREF_SYMBOL,
            &refcount_signature,
        )
        .expect("runtime incref support function should be available");
        let decref_id = declare_local_fn(
            &mut jit_module,
            SOAC_RUNTIME_DECREF_SYMBOL,
            &refcount_signature,
        )
        .expect("runtime decref support function should be available");

        let mut ctx = jit_module.make_context();
        ctx.func.name = ir::UserFuncName::user(0, wrapper_id.as_u32());
        ctx.func.signature = wrapper_signature;

        let mut builder_ctx = FunctionBuilderContext::new();
        {
            let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
            let entry = fb.create_block();
            fb.append_block_params_for_function_params(entry);
            fb.switch_to_block(entry);
            fb.seal_block(entry);

            let incref_ref = jit_module.declare_func_in_func(incref_id, &mut fb.func);
            let decref_ref = jit_module.declare_func_in_func(decref_id, &mut fb.func);
            let arg = fb.block_params(entry)[0];
            fb.ins().call(incref_ref, &[arg]);
            fb.ins().call(decref_ref, &[arg]);
            fb.ins().return_(&[arg]);
            fb.finalize();
        }

        (
            jit_module,
            ctx,
            wrapper_id,
            [
                ir::UserExternalName::new(0, incref_id.as_u32()),
                ir::UserExternalName::new(0, decref_id.as_u32()),
            ],
        )
    }

    unsafe fn build_runtime_refcount_smoke_wrapper()
    -> unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void {
        let (mut jit_module, mut ctx, wrapper_id, _) = build_runtime_refcount_smoke_context();

        define_function_with_incremental_cache(
            &mut jit_module,
            wrapper_id,
            &mut ctx,
            "test wrapper function should define",
        )
        .expect("wrapper function should compile");
        jit_module.clear_context(&mut ctx);
        jit_module
            .finalize_definitions()
            .expect("jit module should finalize");

        let code_ptr = jit_module.get_finalized_function(wrapper_id);
        let compiled: unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void =
            std::mem::transmute(code_ptr);
        Box::leak(Box::new(jit_module));
        compiled
    }

    unsafe fn build_runtime_decref_wrapper() -> unsafe extern "C" fn(*mut std::ffi::c_void) {
        let mut jit_module = new_jit_module().expect("test jit module should construct");
        let ptr_ty = jit_module.target_config().pointer_type();

        let mut refcount_signature = jit_module.make_signature();
        refcount_signature.params.push(ir::AbiParam::new(ptr_ty));

        let mut wrapper_signature = jit_module.make_signature();
        wrapper_signature.params.push(ir::AbiParam::new(ptr_ty));

        let wrapper_id = declare_local_fn(
            &mut jit_module,
            "jit_runtime_support_decref_wrapper",
            &wrapper_signature,
        )
        .expect("wrapper function should declare");
        let decref_id = declare_local_fn(
            &mut jit_module,
            SOAC_RUNTIME_DECREF_SYMBOL,
            &refcount_signature,
        )
        .expect("runtime decref support function should be available");

        let mut ctx = jit_module.make_context();
        ctx.func.name = ir::UserFuncName::user(0, wrapper_id.as_u32());
        ctx.func.signature = wrapper_signature;

        let mut builder_ctx = FunctionBuilderContext::new();
        {
            let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
            let entry = fb.create_block();
            fb.append_block_params_for_function_params(entry);
            fb.switch_to_block(entry);
            fb.seal_block(entry);

            let decref_ref = jit_module.declare_func_in_func(decref_id, &mut fb.func);
            let arg = fb.block_params(entry)[0];
            fb.ins().call(decref_ref, &[arg]);
            fb.ins().return_(&[]);
            fb.finalize();
        }

        define_function_with_incremental_cache(
            &mut jit_module,
            wrapper_id,
            &mut ctx,
            "test wrapper function should define",
        )
        .expect("wrapper function should compile");
        jit_module.clear_context(&mut ctx);
        jit_module
            .finalize_definitions()
            .expect("jit module should finalize");

        let code_ptr = jit_module.get_finalized_function(wrapper_id);
        let compiled: unsafe extern "C" fn(*mut std::ffi::c_void) = std::mem::transmute(code_ptr);
        Box::leak(Box::new(jit_module));
        compiled
    }

    #[test]
    fn jit_can_call_runtime_support_clif_function() {
        unsafe {
            let wrapper = build_runtime_refcount_smoke_wrapper();
            let result = wrapper(std::ptr::null_mut());
            assert!(
                result.is_null(),
                "runtime incref/decref smoke wrapper should preserve the null pointer"
            );
        }
    }

    #[test]
    fn jit_runtime_support_inliner_removes_direct_refcount_calls_from_caller() {
        let (mut jit_module, mut ctx, _wrapper_id, helper_names) =
            unsafe { build_runtime_refcount_smoke_context() };
        let before = count_direct_calls_to_runtime_helpers(&ctx.func, &helper_names);
        assert_eq!(
            before, 2,
            "test caller should start with direct incref/decref calls"
        );

        let inlined = inline_runtime_support_calls(
            &mut jit_module,
            &mut ctx,
            "test runtime support inliner should run",
        )
        .expect("runtime support inliner should succeed");
        let after = count_direct_calls_to_runtime_helpers(&ctx.func, &helper_names);

        assert!(
            inlined,
            "runtime support inliner should report at least one inlined call"
        );
        assert_eq!(
            after, 0,
            "runtime support inliner should remove direct incref/decref calls from the caller"
        );
    }

    #[test]
    fn jit_runtime_clif_refcount_roundtrip_preserves_py_long_refcount() {
        unsafe {
            let wrapper = build_runtime_refcount_smoke_wrapper();
            let python_home = vendored_python_home();
            std::env::set_var("PYTHONHOME", &python_home);
            std::env::set_var("PYTHONPATH", python_home.join("Lib"));
            Python::initialize();
            Python::attach(|_| {
                let obj = ffi::PyLong_FromLongLong(123);
                assert!(
                    !obj.is_null(),
                    "PyLong_FromLongLong should produce a test object"
                );
                let before = ffi::Py_REFCNT(obj);
                let result = wrapper(obj.cast());
                let after = ffi::Py_REFCNT(obj);
                assert_eq!(result, obj.cast(), "wrapper should return the same pointer");
                assert_eq!(
                    after, before,
                    "runtime CLIF incref/decref should preserve PyLong refcount"
                );
                ffi::Py_DECREF(obj);
            });
        }
    }

    #[test]
    fn jit_runtime_clif_decref_can_destroy_py_capsule() {
        unsafe {
            let wrapper = build_runtime_decref_wrapper();
            let python_home = vendored_python_home();
            std::env::set_var("PYTHONHOME", &python_home);
            std::env::set_var("PYTHONPATH", python_home.join("Lib"));
            Python::initialize();
            Python::attach(|_| {
                CAPSULE_DESTROYED.store(false, Ordering::SeqCst);
                let capsule = ffi::PyCapsule_New(
                    std::ptr::dangling_mut::<c_void>(),
                    c"soac.runtime.test".as_ptr(),
                    Some(test_capsule_destructor),
                );
                assert!(
                    !capsule.is_null(),
                    "PyCapsule_New should produce a test object"
                );
                assert_eq!(
                    ffi::Py_REFCNT(capsule),
                    1,
                    "capsule should start with a unique owned reference"
                );

                wrapper(capsule.cast());
                let after = ffi::Py_REFCNT(capsule);

                assert!(
                    CAPSULE_DESTROYED.load(Ordering::SeqCst),
                    "runtime CLIF decref should drive PyCapsule destruction through _Py_Dealloc; refcnt after wrapper = {after}"
                );
            });
        }
    }

    #[test]
    fn jit_vectorcall_trampoline_can_link_runtime_decref_clif() {
        unsafe {
            let compiled = Box::new(CompiledSpecializedRunner {
                _jit_module: new_jit_module().expect("compiled runner jit module should construct"),
                entry: Some(CompiledRunnerEntry::Direct {
                    code_ptr: std::ptr::null(),
                    param_count: 0,
                }),
            });
            let compiled_handle = Box::into_raw(compiled) as ObjPtr;
            let result = compile_cranelift_vectorcall_direct_trampoline(
                test_bind_direct_args_stub,
                1usize as ObjPtr,
                1usize as ObjPtr,
                compiled_handle,
                "jit_runtime_support_vectorcall_smoke",
            );

            match result {
                Ok((trampoline_handle, _entry)) => {
                    free_cranelift_vectorcall_trampoline(trampoline_handle);
                }
                Err(error) => {
                    free_cranelift_run_bb_specialized_cached(compiled_handle);
                    panic!(
                        "vectorcall trampoline should link runtime CLIF refcount helpers: {error}"
                    );
                }
            }

            free_cranelift_run_bb_specialized_cached(compiled_handle);
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
            "rendered CLIF should still surface the scope name for optimized blocks:\n{rendered}"
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
            ret_term(op_expr(BinOp::new(
                BinOpKind::Add,
                constants.int_expr(1),
                constants.int_expr(2),
            ))),
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
            ret_term(op_expr(BinOp::new(
                BinOpKind::Lt,
                constants.int_expr(1),
                constants.int_expr(2),
            ))),
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
            !rendered.contains("call dp_jit_load_module_constant"),
            "string literal lowering should not call the module constant hook anymore:\n{rendered}"
        );
        assert!(
            rendered.contains("iconst.i64 4096"),
            "string literal lowering should embed the immortal module constant pointer directly:\n{rendered}"
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
            ret_term(op_expr(Load::new(test_constant_name(0)))),
        );
        let module = BlockPyModule {
            module_name_gen: ModuleNameGen::new(0),
            callable_defs: vec![function.clone()],
            module_constants: vec![int_literal(7)],
        };
        let module_constants =
            crate::module_constants::ModuleCodegenConstants::collect_from_module(&module);
        let rendered =
            render_test_jit_function_with_constants(&function, &blocks, &module_constants);
        assert!(
            !rendered.contains("call dp_jit_load_module_constant"),
            "constant slot lowering should not call the module constant hook anymore:\n{rendered}"
        );
        assert!(
            rendered.contains("iconst.i64 4096"),
            "constant slot lowering should embed the immortal module constant pointer directly:\n{rendered}"
        );
    }

    fn render_specialized_jit_pow_calls_use_pynumber_power() {
        let blocks = [1usize as ObjPtr];
        let mut constants = TestConstantPool::default();
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(BinOp::new(
                BinOpKind::Pow,
                constants.int_expr(2),
                constants.int_expr(3),
            ))),
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
            ret_term(op_expr(BinOp::new(
                BinOpKind::InplacePow,
                constants.int_expr(2),
                constants.int_expr(3),
            ))),
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
            rendered.contains("store.i64")
                || rendered.contains("stack_store")
                || rendered.contains("store notrap"),
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
                && !rendered.contains("call dp_jit_load_module_constant"),
            "global located names should use vmctx-backed globals and pass the name object as an immediate constant:\n{rendered}"
        );
    }

    #[test]
    fn render_specialized_jit_load_global_intrinsic_uses_direct_helper() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(Load::new(test_global_name("x")))),
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
            ret_term(op_expr(Store::new(
                test_global_name("x"),
                constants.int_expr(3),
            ))),
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
            ret_term(op_expr(soac_blockpy::block_py::CellRef::new(
                CellLocation::Closure(2),
            ))),
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
            ret_term(op_expr(soac_blockpy::block_py::CellRef::new(
                CellLocation::CapturedSource(2),
            ))),
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
                expr_stmt(op_expr(DelItem::new(
                    constants.int_expr(1),
                    constants.int_expr(2),
                ))),
                expr_stmt(op_expr(Del::new(test_global_name("x"), true))),
                expr_stmt(op_expr(Del::new(test_closure_cell_name("cell", 2), false))),
                expr_stmt(op_expr(Del::new(test_closure_cell_name("cell", 2), true))),
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
            ret_term(op_expr(Call::new(
                name_expr(test_runtime_name("load_deleted_name")),
                vec![
                    CoreBlockPyCallArg::Positional(constants.string_expr("x")),
                    CoreBlockPyCallArg::Positional(name_expr(test_name("x"))),
                ],
                vec![],
            ))),
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
    fn render_specialized_jit_constant_runtime_helper_calls_still_specialize() {
        let blocks = [1usize as ObjPtr];
        let function = with_single_test_block(
            test_function(),
            vec![],
            ret_term(op_expr(Call::new(
                name_expr(test_constant_name(0)),
                vec![],
                vec![],
            ))),
        );
        let rendered = render_test_jit_function_with_module_constants(
            &function,
            &blocks,
            vec![CoreBlockPyExpr::Load(Load::new(test_runtime_name(
                "globals",
            )))],
        );
        assert!(
            !rendered.contains("call dp_jit_load_runtime_obj")
                && !rendered.contains("call dp_jit_py_call_object")
                && !rendered.contains("call dp_jit_py_call_with_kw"),
            "constant-backed runtime helpers should still specialize instead of reloading or generic-calling:\n{rendered}"
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

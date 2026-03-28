use super::{
    callable_semantic_info, capture_items_to_expr, closure_freevar_capture_items,
    rewrite_ast_to_lowered_blockpy_module_plan_with_module, BlockPyModuleRewriter,
    FunctionScopeFrame,
};
use crate::block_py::{
    BindingTarget, BlockPyBindingKind, BlockPyBindingPurpose, BlockPyClassBodyFallback,
    BlockPyEffectiveBinding, BlockPyModule, ClosureInit, ClosureLayout, ClosureSlot, ModuleNameGen,
};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::semantic::SemanticAstState;
use crate::passes::RuffBlockPyPass;
use crate::transform_str_to_ruff;
use ruff_python_ast::Stmt;
use ruff_python_parser::parse_module;

fn lower_test_module_plan(
    context: &Context,
    mut module: Vec<Stmt>,
) -> BlockPyModule<RuffBlockPyPass> {
    crate::passes::ast_to_ast::simplify::flatten(&mut module);
    let mut semantic_state = SemanticAstState::from_ruff(&mut module);
    if !module.iter().any(
            |stmt| matches!(stmt, Stmt::FunctionDef(func) if func.name.id.as_str() == "_dp_module_init"),
        ) {
            crate::driver::wrap_module_init(&mut semantic_state, &mut module);
        }
    rewrite_ast_to_lowered_blockpy_module_plan_with_module(context, &mut module, &semantic_state)
}

#[test]
fn capture_items_render_as_name_value_pairs() {
    let mut semantic = crate::block_py::BlockPyCallableSemanticInfo::default();
    semantic
        .cell_capture_source_names
        .insert("x".to_string(), "_dp_cell_x".to_string());
    semantic
        .cell_capture_source_names
        .insert("y".to_string(), "_dp_classcell".to_string());
    let captures = closure_freevar_capture_items(
        Some(&ClosureLayout {
            freevars: vec![
                ClosureSlot {
                    logical_name: "x".to_string(),
                    storage_name: "x".to_string(),
                    init: ClosureInit::InheritedCapture,
                },
                ClosureSlot {
                    logical_name: "y".to_string(),
                    storage_name: "y".to_string(),
                    init: ClosureInit::InheritedCapture,
                },
            ],
            cellvars: vec![],
            runtime_cells: vec![],
        }),
        &semantic,
    );
    let expr = capture_items_to_expr(&captures);
    assert_eq!(
        crate::ruff_ast_to_string(&expr).trim(),
        "__dp_tuple(__dp_tuple(\"x\", __dp_cell_ref(\"x\")), __dp_tuple(\"y\", __dp_cell_ref(\"y\")))"
    );
}

#[test]
fn callable_semantic_info_uses_logical_storage_for_cell_captures() {
    let source = concat!(
        "def outer():\n",
        "    x = 1\n",
        "    def inner():\n",
        "        return x\n",
        "    return inner\n",
    );
    let blockpy_module = transform_str_to_ruff(source)
        .unwrap()
        .get_pass::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
        .cloned()
        .expect("semantic_blockpy pass should be tracked");
    let inner = blockpy_module
        .callable_defs
        .iter()
        .find(|func| func.names.bind_name == "inner")
        .expect("missing inner callable");

    assert_eq!(
        inner.semantic.binding_kind("x"),
        Some(BlockPyBindingKind::Cell(
            crate::block_py::BlockPyCellBindingKind::Capture
        ))
    );
    assert_eq!(inner.semantic.cell_storage_name("x"), "x");
    assert_eq!(inner.semantic.cell_capture_source_name("x"), "_dp_cell_x");
    assert_eq!(
        inner
            .semantic
            .logical_name_for_cell_capture_source("_dp_cell_x"),
        Some("x".to_string())
    );
}

#[test]
fn callable_semantic_info_maps_classcell_capture_source_back_to_dunder_class() {
    let source = concat!(
        "class C:\n",
        "    def f(self):\n",
        "        def g():\n",
        "            return __class__\n",
        "        return g\n",
    );
    let blockpy_module = transform_str_to_ruff(source)
        .unwrap()
        .get_pass::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
        .cloned()
        .expect("semantic_blockpy pass should be tracked");
    let f = blockpy_module
        .callable_defs
        .iter()
        .find(|func| func.names.bind_name == "f")
        .expect("missing method callable");

    assert_eq!(
        f.semantic.binding_kind("__class__"),
        Some(BlockPyBindingKind::Cell(
            crate::block_py::BlockPyCellBindingKind::Capture
        ))
    );
    assert_eq!(f.semantic.cell_storage_name("__class__"), "__class__");
    assert_eq!(
        f.semantic.cell_capture_source_name("__class__"),
        "_dp_classcell"
    );
    assert_eq!(
        f.semantic
            .logical_name_for_cell_capture_source("_dp_classcell"),
        Some("__class__".to_string())
    );
}

#[test]
fn recursive_local_function_bindings_are_cell_owned_in_parent_scope() {
    let source = concat!(
        "def outer():\n",
        "    def recurse():\n",
        "        return recurse()\n",
        "    return recurse\n",
    );
    let context = Context::new(source);
    let mut module = parse_module(source).unwrap().into_syntax().body;
    let semantic_state = SemanticAstState::from_ruff(&mut module);
    let Stmt::FunctionDef(outer) = &mut module[0] else {
        panic!("expected outer function");
    };
    let outer_scope = semantic_state
        .function_scope(outer)
        .expect("missing outer scope");
    let mut rewriter = BlockPyModuleRewriter {
        context: &context,
        semantic_state: &semantic_state,
        module_name_gen: ModuleNameGen::new(0),
        function_scope_stack: vec![FunctionScopeFrame {
            scope: Some(outer_scope.clone()),
            callable_semantic: callable_semantic_info(
                &semantic_state,
                None,
                Some(&outer_scope),
                Some(outer),
                &outer.body,
            ),
            hoisted_to_parent: Vec::new(),
        }],
        callable_defs: Vec::new(),
    };
    let nested_stmt = &mut outer
        .body
        .iter_mut()
        .find(|stmt| matches!(stmt, Stmt::FunctionDef(_)))
        .expect("missing nested function");
    let Stmt::FunctionDef(nested_func) = nested_stmt else {
        panic!("expected nested function def");
    };
    let nested_state = rewriter.walk_function_def_with_scope(nested_func);
    assert_eq!(
        rewriter
            .function_scope_stack
            .last()
            .expect("missing outer function frame")
            .callable_semantic
            .binding_kind("recurse"),
        Some(crate::block_py::BlockPyBindingKind::Cell(
            crate::block_py::BlockPyCellBindingKind::Owner
        ))
    );
    let replacement = rewriter.rewrite_visited_function_def(nested_func, nested_state);
    let rendered = replacement
        .iter()
        .map(crate::ruff_ast_to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
        "{rendered}"
    );
}

#[test]
fn callable_semantic_info_tracks_bind_and_qualname_for_class_helper_override() {
    let source = "class Box:\n    value = 1\n";
    let blockpy_module = transform_str_to_ruff(source)
        .unwrap()
        .get_pass::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
        .cloned()
        .expect("semantic_blockpy pass should be tracked");
    let class_helper = blockpy_module
        .callable_defs
        .iter()
        .find(|func| func.names.bind_name == "_dp_class_ns_Box")
        .expect("missing class helper");
    assert_eq!(class_helper.semantic.names.bind_name, "_dp_class_ns_Box");
    assert_eq!(class_helper.semantic.names.display_name, "_dp_class_ns_Box");
    assert_eq!(class_helper.semantic.names.qualname, "_dp_class_ns_Box");
}

#[test]
fn callable_semantic_info_marks_class_helper_as_owning_classcell() {
    let source = "class Box:\n    pass\n";
    let blockpy_module = transform_str_to_ruff(source)
        .unwrap()
        .get_pass::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
        .cloned()
        .expect("semantic_blockpy pass should be tracked");
    let class_helper = blockpy_module
        .callable_defs
        .iter()
        .find(|func| func.names.bind_name == "_dp_class_ns_Box")
        .expect("missing class helper");

    assert_eq!(
        class_helper.semantic.binding_kind("__class__"),
        Some(BlockPyBindingKind::Cell(
            crate::block_py::BlockPyCellBindingKind::Owner
        ))
    );
    assert!(class_helper.semantic.has_local_def("__class__"));
    assert_eq!(
        class_helper.semantic.cell_storage_name("__class__"),
        "_dp_classcell"
    );
}

#[test]
fn callable_semantic_info_distinguishes_class_type_params_from_class_body_locals() {
    let source = "class Box[T]:\n    value = T\n";
    let blockpy_module = transform_str_to_ruff(source)
        .unwrap()
        .get_pass::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
        .cloned()
        .expect("semantic_blockpy pass should be tracked");
    let class_helper = blockpy_module
        .callable_defs
        .iter()
        .find(|func| func.names.bind_name == "_dp_class_ns_Box")
        .expect("missing class helper");

    assert!(class_helper.semantic.type_param_names.contains("T"));
    assert_eq!(
        class_helper
            .semantic
            .effective_binding("T", BlockPyBindingPurpose::Store),
        Some(BlockPyEffectiveBinding::Local),
    );
    assert_eq!(
        class_helper
            .semantic
            .binding_target_for_name("T", BlockPyBindingPurpose::Store),
        BindingTarget::Local,
    );
    assert_eq!(
        class_helper
            .semantic
            .effective_binding("T", BlockPyBindingPurpose::Load),
        Some(BlockPyEffectiveBinding::ClassBody(
            BlockPyClassBodyFallback::Global
        )),
    );
    assert_eq!(
        class_helper
            .semantic
            .binding_target_for_name("value", BlockPyBindingPurpose::Store),
        BindingTarget::ClassNamespace,
    );
    assert_eq!(
        class_helper
            .semantic
            .effective_binding("value", BlockPyBindingPurpose::Store),
        Some(BlockPyEffectiveBinding::ClassBody(
            BlockPyClassBodyFallback::Global
        ))
    );
}

#[test]
fn callable_semantic_info_keeps_class_attrs_out_of_cell_bindings() {
    let source = concat!(
        "def outer():\n",
        "    x = \"outer\"\n",
        "    class Inner:\n",
        "        x = \"class\"\n",
        "        def read():\n",
        "            return x\n",
        "    return Inner\n",
    );
    let blockpy_module = transform_str_to_ruff(source)
        .unwrap()
        .get_pass::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
        .cloned()
        .expect("semantic_blockpy pass should be tracked");
    let class_helper = blockpy_module
        .callable_defs
        .iter()
        .find(|func| func.names.bind_name == "_dp_class_ns_Inner")
        .expect("missing class helper");

    assert_eq!(
        class_helper.semantic.binding_kind("x"),
        Some(BlockPyBindingKind::Local),
    );
    assert_eq!(
        class_helper
            .semantic
            .binding_target_for_name("x", BlockPyBindingPurpose::Store),
        BindingTarget::ClassNamespace,
    );
}

#[test]
fn callable_semantic_info_records_class_cell_fallback_for_outer_reads() {
    let source = concat!(
        "def outer():\n",
        "    x = 1\n",
        "    class Box:\n",
        "        value = x\n",
        "    return Box\n",
    );
    let blockpy_module = transform_str_to_ruff(source)
        .unwrap()
        .get_pass::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
        .cloned()
        .expect("semantic_blockpy pass should be tracked");
    let class_helper = blockpy_module
        .callable_defs
        .iter()
        .find(|func| func.names.bind_name == "_dp_class_ns_Box")
        .expect("missing class helper");

    assert_eq!(
        class_helper
            .semantic
            .effective_binding("x", BlockPyBindingPurpose::Load),
        Some(BlockPyEffectiveBinding::ClassBody(
            BlockPyClassBodyFallback::Cell
        ))
    );
}

#[test]
fn callable_semantic_info_resolves_implicit_global_loads_in_body() {
    let source = concat!(
        "def outer(scale):\n",
        "    factor = scale\n",
        "    def inner(x):\n",
        "        try:\n",
        "            return x + factor\n",
        "        except Exception as exc:\n",
        "            return len(str(exc))\n",
        "    return inner\n",
    );
    let mut module = parse_module(source).unwrap().into_syntax().body;
    let semantic_state = SemanticAstState::from_ruff(&mut module);
    let Stmt::FunctionDef(outer) = &module[0] else {
        panic!("expected outer function");
    };
    let inner = &outer
        .body
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::FunctionDef(func) if func.name.id.as_str() == "inner" => Some(func),
            _ => None,
        })
        .expect("missing inner");
    let inner_scope = semantic_state
        .function_scope(inner)
        .expect("missing inner scope");
    let outer_scope = semantic_state
        .function_scope(outer)
        .expect("missing outer scope");
    let semantic = callable_semantic_info(
        &semantic_state,
        Some(&outer_scope),
        Some(&inner_scope),
        Some(inner),
        &inner.body,
    );

    assert_eq!(
        semantic.binding_kind("factor"),
        Some(crate::block_py::BlockPyBindingKind::Cell(
            crate::block_py::BlockPyCellBindingKind::Capture
        ))
    );
    assert_eq!(
        semantic.binding_kind("x"),
        Some(crate::block_py::BlockPyBindingKind::Local)
    );
    assert_eq!(
        semantic.binding_kind("Exception"),
        Some(crate::block_py::BlockPyBindingKind::Global)
    );
    assert_eq!(
        semantic.binding_kind("len"),
        Some(crate::block_py::BlockPyBindingKind::Global)
    );
    assert_eq!(
        semantic.binding_kind("str"),
        Some(crate::block_py::BlockPyBindingKind::Global)
    );
}

#[test]
fn lowering_recursive_local_function_with_finally_keeps_plain_binding_before_name_binding() {
    let source = concat!(
        "import sys\n",
        "def exercise():\n",
        "    original_limit = sys.getrecursionlimit()\n",
        "    sys.setrecursionlimit(50)\n",
        "    def recurse():\n",
        "        return recurse()\n",
        "    try:\n",
        "        try:\n",
        "            recurse()\n",
        "        except RecursionError:\n",
        "            return True\n",
        "        return False\n",
        "    finally:\n",
        "        sys.setrecursionlimit(original_limit)\n",
    );
    let context = Context::new(source);
    let module = parse_module(source).unwrap().into_syntax().body;
    let blockpy = lower_test_module_plan(&context, module);
    let exercise = blockpy
        .callable_defs
        .iter()
        .find(|callable| callable.names.bind_name == "exercise")
        .expect("missing lowered exercise callable");
    let rendered =
        crate::block_py::pretty::blockpy_module_to_string(&crate::block_py::BlockPyModule {
            callable_defs: vec![exercise.clone()],
        });
    assert!(
        rendered.contains("recurse = __dp_make_function"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
        "{rendered}"
    );
}

#[test]
fn lowering_recursive_local_function_treats_recurse_cell_as_local_state() {
    let source = concat!(
        "import sys\n",
        "def exercise():\n",
        "    original_limit = sys.getrecursionlimit()\n",
        "    sys.setrecursionlimit(50)\n",
        "    def recurse():\n",
        "        return recurse()\n",
        "    try:\n",
        "        try:\n",
        "            recurse()\n",
        "        except RecursionError:\n",
        "            return True\n",
        "        return False\n",
        "    finally:\n",
        "        sys.setrecursionlimit(original_limit)\n",
    );
    let context = Context::new(source);
    let module = parse_module(source).unwrap().into_syntax().body;
    let blockpy = lower_test_module_plan(&context, module);
    let exercise = blockpy
        .callable_defs
        .iter()
        .find(|callable| callable.names.bind_name == "exercise")
        .expect("missing lowered exercise callable");
    assert!(
        exercise.semantic.binding_kind("recurse")
            == Some(crate::block_py::BlockPyBindingKind::Cell(
                crate::block_py::BlockPyCellBindingKind::Owner
            )),
        "semantic_bindings={:?}",
        exercise.semantic.bindings,
    );
}

#[test]
fn lowering_recursive_local_function_finally_return_preserves_liveins() {
    let source = concat!(
        "import sys\n",
        "def exercise():\n",
        "    original_limit = sys.getrecursionlimit()\n",
        "    sys.setrecursionlimit(50)\n",
        "    def recurse():\n",
        "        return recurse()\n",
        "    try:\n",
        "        try:\n",
        "            recurse()\n",
        "        except RecursionError:\n",
        "            return True\n",
        "        return False\n",
        "    finally:\n",
        "        sys.setrecursionlimit(original_limit)\n",
    );
    let context = Context::new(source);
    let module = parse_module(source).unwrap().into_syntax().body;
    let blockpy = lower_test_module_plan(&context, module);
    let exercise = blockpy
        .callable_defs
        .iter()
        .find(|callable| callable.names.bind_name == "exercise")
        .expect("missing lowered exercise callable");
    let rendered =
        crate::block_py::pretty::blockpy_module_to_string(&crate::block_py::BlockPyModule {
            callable_defs: vec![exercise.clone()],
        });
    assert!(
        rendered.contains("jump ") && rendered.contains("(Return, _dp_try_abrupt_payload_"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("(None, Return, _dp_try_abrupt_payload_"),
        "{rendered}"
    );
}

#[test]
fn lowering_nonlocal_inner_captures_outer_cell() {
    let source = concat!(
        "def outer():\n",
        "    x = 5\n",
        "    def inner():\n",
        "        nonlocal x\n",
        "        x = 2\n",
        "        return x\n",
        "    return inner()\n",
    );
    let context = Context::new(source);
    let module = parse_module(source).unwrap().into_syntax().body;
    let blockpy = lower_test_module_plan(&context, module);
    let inner = blockpy
        .callable_defs
        .iter()
        .find(|callable| callable.names.bind_name == "inner")
        .expect("missing lowered inner callable");
    let outer = blockpy
        .callable_defs
        .iter()
        .find(|callable| callable.names.bind_name == "outer")
        .expect("missing lowered outer callable");
    assert!(
        inner
            .closure_layout()
            .as_ref()
            .expect("inner should have closure layout")
            .freevars
            .iter()
            .any(|slot| slot.storage_name == "x"),
        "{:?}",
        inner.closure_layout()
    );
    let rendered =
        crate::block_py::pretty::blockpy_module_to_string(&crate::block_py::BlockPyModule {
            callable_defs: vec![outer.clone()],
        });
    assert!(
            rendered.contains(
                "__dp_make_function(0, \"function\", __dp_tuple(__dp_tuple(\"x\", __dp_cell_ref(\"x\")))"
            ),
            "{rendered}"
        );
}

pub(crate) mod ast_symbol_analysis;
pub(crate) mod ast_to_ast;
pub(crate) mod blockpy_expr_simplify;
mod blockpy_generators;
pub mod blockpy_to_bb;
pub(crate) mod core_await_lower;
pub(crate) mod core_eval_order;
mod name_binding;
pub mod ruff_to_blockpy;
mod summarize_pass_shape;
mod trace;

use crate::block_py::{
    BlockPyPass, BlockPyStmt, CoreBlockPyExpr, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, Expr,
};

#[derive(Debug, Clone)]
pub struct RuffBlockPyPass;

impl BlockPyPass for RuffBlockPyPass {
    type Expr = Expr;
    type Stmt = BlockPyStmt<Self::Expr>;
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyPassWithAwaitAndYield;

impl BlockPyPass for CoreBlockPyPassWithAwaitAndYield {
    type Expr = CoreBlockPyExprWithAwaitAndYield;
    type Stmt = BlockPyStmt<Self::Expr>;
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyPassWithYield;

impl BlockPyPass for CoreBlockPyPassWithYield {
    type Expr = CoreBlockPyExprWithYield;
    type Stmt = BlockPyStmt<Self::Expr>;
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyPass;

impl BlockPyPass for CoreBlockPyPass {
    type Expr = CoreBlockPyExpr;
    type Stmt = BlockPyStmt<Self::Expr>;
}

#[derive(Debug, Clone)]
pub struct BbBlockPyPass;

impl BlockPyPass for BbBlockPyPass {
    type Expr = CoreBlockPyExpr;
    type Stmt = crate::block_py::BbStmt;
}

#[derive(Debug, Clone)]
pub struct PreparedBbBlockPyPass;

impl BlockPyPass for PreparedBbBlockPyPass {
    type Expr = CoreBlockPyExpr;
    type Stmt = crate::block_py::BbStmt;
}

pub(crate) use blockpy_to_bb::{
    lower_core_blockpy_module_bundle_to_bb_module,
    lower_yield_in_lowered_core_blockpy_module_bundle,
};
pub use blockpy_to_bb::{lower_try_jump_exception_flow, normalize_bb_module_for_codegen};

pub use ast_to_ast::rewrite_stmt::single_assigment::SingleNamedAssignmentPass;
pub(crate) use name_binding::lower_name_binding_in_core_blockpy_module;
pub(crate) use summarize_pass_shape::summarize_tracked_pass_shape;

#[cfg(test)]
mod tests {
    use crate::block_py::{
        BbBlock, BbStmt, BlockPyCallableScopeKind, BlockPyFunction, BlockPyFunctionKind,
        BlockPyModule, BlockPyTerm, CoreBlockPyExpr,
    };
    use crate::block_py::{ClosureInit, ClosureSlot};
    use crate::passes::{BbBlockPyPass, CoreBlockPyPass, RuffBlockPyPass};
    use crate::LoweringResult;
    use crate::{
        py_expr, transform_str_to_bb_ir_with_options, transform_str_to_ruff_with_options, Options,
    };
    struct TrackedLowering {
        result: LoweringResult,
        blockpy_module: BlockPyModule<RuffBlockPyPass>,
    }

    impl TrackedLowering {
        fn new(source: &str) -> Self {
            let blockpy_module =
                crate::transform_str_to_blockpy_with_options(source, Options::for_test())
                    .expect("transform should succeed");
            Self {
                result: transform_str_to_ruff_with_options(source, Options::for_test())
                    .expect("transform should succeed"),
                blockpy_module,
            }
        }

        fn blockpy_module(&self) -> BlockPyModule<RuffBlockPyPass> {
            self.blockpy_module.clone()
        }

        fn blockpy_text(&self) -> String {
            crate::block_py::pretty::blockpy_module_to_string(&self.blockpy_module())
        }

        fn semantic_blockpy_text(&self) -> String {
            self.pass_text("semantic_blockpy")
        }

        fn core_blockpy_with_await_and_yield_text(&self) -> String {
            self.pass_text("core_blockpy_with_await_and_yield")
        }

        fn core_blockpy_with_yield_text(&self) -> String {
            self.pass_text("core_blockpy_with_yield")
        }

        fn name_binding_text(&self) -> String {
            self.pass_text("name_binding")
        }

        fn pass_text(&self, name: &str) -> String {
            self.result
                .render_pass_text(name)
                .unwrap_or_else(|| panic!("expected renderable pass {name}"))
        }

        fn bb_module(&self) -> &BlockPyModule<BbBlockPyPass> {
            self.result
                .get_pass::<BlockPyModule<BbBlockPyPass>>("bb_blockpy")
                .expect("bb module should be available")
        }

        fn bb_function(&self, bind_name: &str) -> &BlockPyFunction<BbBlockPyPass> {
            function_by_name(self.bb_module(), bind_name)
        }
    }

    fn function_by_name<'a>(
        bb_module: &'a BlockPyModule<BbBlockPyPass>,
        bind_name: &str,
    ) -> &'a BlockPyFunction<BbBlockPyPass> {
        let resume_name = format!("{bind_name}_resume");
        if let Some(resume) = bb_module
            .callable_defs
            .iter()
            .find(|func| func.names.bind_name == resume_name)
        {
            return resume;
        }
        bb_module
            .callable_defs
            .iter()
            .find(|func| func.names.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing lowered function {bind_name}; got {:?}", bb_module))
    }

    fn slot_by_name<'a>(slots: &'a [ClosureSlot], logical_name: &str) -> &'a ClosureSlot {
        slots
            .iter()
            .find(|slot| slot.logical_name == logical_name)
            .unwrap_or_else(|| panic!("missing closure slot {logical_name}; got {slots:?}"))
    }

    fn expr_text(expr: &CoreBlockPyExpr) -> String {
        crate::block_py::pretty::bb_expr_text(expr)
    }

    fn callable_def_by_name<'a>(
        blockpy_module: &'a BlockPyModule<RuffBlockPyPass>,
        bind_name: &str,
    ) -> &'a BlockPyFunction<RuffBlockPyPass> {
        blockpy_module
            .callable_defs
            .iter()
            .find(|callable| callable.names.bind_name == bind_name)
            .unwrap_or_else(|| {
                panic!("missing callable definition {bind_name}; got {blockpy_module:?}")
            })
    }

    fn block_uses_text(block: &BbBlock, needle: &str) -> bool {
        block.body.iter().any(|op| match op {
            BbStmt::Assign(assign) => expr_text(&assign.value).contains(needle),
            BbStmt::Expr(expr) => expr_text(expr).contains(needle),
            BbStmt::Delete(delete) => delete.target.id.as_str().contains(needle),
        }) || match &block.term {
            BlockPyTerm::IfTerm(if_term) => expr_text(&if_term.test).contains(needle),
            BlockPyTerm::BranchTable(branch) => expr_text(&branch.index).contains(needle),
            BlockPyTerm::Raise(raise_stmt) => raise_stmt
                .exc
                .as_ref()
                .is_some_and(|value| expr_text(value).contains(needle)),
            BlockPyTerm::Return(value) => expr_text(value).contains(needle),
            _ => false,
        }
    }

    #[test]
    fn semantic_blockpy_keeps_plain_coroutines_without_fake_yield_marker() {
        let source = r#"
async def foo():
    return 1

async def classify():
    return await foo()
"#;

        let lowered = TrackedLowering::new(source);
        let rendered = lowered.pass_text("semantic_blockpy");
        assert!(rendered.contains("coroutine classify():"), "{rendered}");
        assert!(rendered.contains("return await foo()"), "{rendered}");
        assert!(!rendered.contains("yield __dp_NONE"), "{rendered}");
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_fstring_while_core_blockpy_expr_simplify_handles_it() {
        let source = r#"
def fmt(value):
    return f"{value=}"
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy = lowered.blockpy_text();
        assert!(blockpy.contains("f\"{value=}\""), "{blockpy}");

        let core_blockpy = lowered.core_blockpy_with_await_and_yield_text();
        assert!(core_blockpy.contains("\"value=\""), "{core_blockpy}");
        assert!(
            core_blockpy.contains("__dp_repr(value)")
                || core_blockpy.contains("__dp_load_global(__dp_globals(), \"__dp_repr\")(value)"),
            "{core_blockpy}"
        );
        assert!(
            core_blockpy.contains("__dp_format(__dp_repr(value))")
                || core_blockpy.contains(
                    "__dp_load_global(__dp_globals(), \"__dp_format\")(__dp_load_global(__dp_globals(), \"__dp_repr\")(value))"
                ),
            "{core_blockpy}"
        );

        let fmt = lowered.bb_function("fmt");
        assert!(
            fmt.blocks.iter().any(|block| {
                block_uses_text(block, "__dp_repr(value)")
                    || block_uses_text(block, "__dp_load_global(__dp_globals(), \"__dp_repr\")")
            }),
            "{fmt:?}"
        );
        assert!(
            fmt.blocks.iter().any(|block| {
                block_uses_text(block, "__dp_format(_dp_eval_2)")
                    || block_uses_text(block, "__dp_load_global(__dp_globals(), \"__dp_format\")")
            }),
            "{fmt:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_tstring_while_core_blockpy_expr_simplify_handles_it() {
        let source = r#"
def fmt(value):
    return t"{value}"
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy = lowered.blockpy_text();
        assert!(blockpy.contains("t\"{value}\""), "{blockpy}");

        let core_blockpy = lowered.core_blockpy_with_await_and_yield_text();
        assert!(
            core_blockpy
                .contains("__dp_templatelib_Interpolation(value, \"value\", __dp_NONE, \"\")"),
            "{core_blockpy}"
        );

        let fmt = lowered.bb_function("fmt");
        assert!(
            fmt.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_templatelib_Interpolation")),
            "{fmt:?}"
        );
        assert!(
            fmt.blocks
                .iter()
                .any(|block| block_uses_text(block, "\"value\"")),
            "{fmt:?}"
        );
        assert!(
            fmt.blocks
                .iter()
                .any(|block| block_uses_text(block, "\"\"")),
            "{fmt:?}"
        );
    }

    #[test]
    fn lowers_simple_if_function_into_basic_blocks() {
        let source = r#"
def foo(a, b):
    c = a + b
    if c > 5:
        print("hi", c)
    else:
        d = b + 1
        print(d)
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let foo = function_by_name(&bb_module, "foo");
        assert!(foo.blocks.len() >= 3, "{foo:?}");
        assert!(
            foo.blocks
                .iter()
                .any(|block| matches!(block.term, BlockPyTerm::IfTerm(_))),
            "{foo:?}"
        );
    }

    #[test]
    fn exposes_bb_ir_for_lowered_functions() {
        let source = r#"
def foo(a, b):
    if a:
        return b
    return a
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let foo = bb_module
            .callable_defs
            .iter()
            .find(|func| func.names.bind_name == "foo")
            .expect("foo should be lowered");
        assert_eq!(
            foo.entry_block().label_str(),
            foo.blocks
                .first()
                .expect("foo should have a first block")
                .label_str()
        );
        assert_ne!(
            foo.entry_block().label_str(),
            "start",
            "{:?}",
            foo.entry_block().label_str()
        );
        assert!(!foo.blocks.is_empty());
    }

    #[test]
    fn nested_global_function_def_stays_lowered() {
        let source = r#"
def build_qualnames():
    def global_function():
        def inner_function():
            global inner_global_function
            def inner_global_function():
                pass
            return inner_global_function
        return inner_function()
    return global_function()
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let inner_global_function = function_by_name(&bb_module, "inner_global_function");
        assert_eq!(
            inner_global_function.names.qualname,
            "inner_global_function"
        );
    }

    #[test]
    fn lowered_class_helper_records_class_scope_kind() {
        let source = r#"
class Box:
    value = 1
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy_module = lowered.blockpy_module();
        let class_helper = callable_def_by_name(&blockpy_module, "_dp_class_ns_Box");
        assert_eq!(
            class_helper.semantic.scope_kind,
            BlockPyCallableScopeKind::Class
        );
    }

    #[test]
    fn class_body_local_load_moves_to_name_binding_pass() {
        let source = r#"
class Box:
    y = 1
    z = y
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_class_lookup_global"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered
                .contains("__dp_class_lookup_global(_dp_class_ns, \"y\", __dp_globals())"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_nonlocal_load_moves_to_name_binding_pass() {
        let source = r#"
def outer():
    x = 1
    class Box:
        y = x
    return Box
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_class_lookup_cell"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered
                .contains("__dp_class_lookup_cell(_dp_class_ns, \"x\", _dp_cell_x)"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_function_binding_moves_to_name_binding_pass() {
        let source = r#"
class Box:
    def f(self):
        return 1
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            core_rendered.contains("f = __dp_make_function"),
            "{core_rendered}"
        );
        assert!(
            !core_rendered.contains("__dp_setitem(_dp_class_ns, \"f\","),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_setitem(_dp_class_ns, \"f\","),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_nonlocal_assignment_moves_to_name_binding_pass() {
        let source = r#"
def outer():
    x = 0
    class Box:
        nonlocal x
        x = 1
    return x
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(core_rendered.contains("x = 1"), "{core_rendered}");
        assert!(
            !core_rendered.contains("__dp_store_cell(_dp_cell_x, 1)"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_cell(_dp_cell_x, 1)"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_local_assignment_moves_to_name_binding_pass() {
        let source = r#"
class Box:
    x = 1
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(core_rendered.contains("x = 1"), "{core_rendered}");
        assert!(
            !core_rendered.contains("__dp_setitem(_dp_class_ns, \"x\", 1)"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_setitem(_dp_class_ns, \"x\", 1)"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_delete_moves_to_name_binding_pass() {
        let source = r#"
class Box:
    x = 1
    del x
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            core_rendered.contains("x = __dp_DELETED"),
            "{core_rendered}"
        );
        assert!(
            !core_rendered.contains("__dp_delitem(_dp_class_ns, \"x\")"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_delitem(_dp_class_ns, \"x\")"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_nonlocal_delete_moves_to_name_binding_pass() {
        let source = r#"
def outer():
    x = 1
    class Box:
        nonlocal x
        del x
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            core_rendered.contains("x = __dp_DELETED"),
            "{core_rendered}"
        );
        assert!(!core_rendered.contains("cell_contents"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_del_deref(_dp_cell_x)"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_except_name_global_binding_moves_to_name_binding_pass() {
        let source = r#"
class Box:
    global caught
    try:
        raise Exception("boom")
    except Exception as caught:
        seen = caught
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global(__dp_globals(), \"caught\""),
            "{core_rendered}"
        );
        assert!(
            core_rendered.contains("caught = __dp_current_exception()"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains(
                "__dp_store_global(__dp_globals(), \"caught\", __dp_current_exception())"
            ),
            "{name_binding_rendered}"
        );
        assert!(
            name_binding_rendered.contains("__dp_del_quietly(__dp_globals(), \"caught\")"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_except_name_nonlocal_binding_moves_to_name_binding_pass() {
        let source = r#"
def outer():
    x = "outer"
    class Box:
        nonlocal x
        try:
            raise Exception("boom")
        except Exception as x:
            pass
    return x
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_cell(_dp_cell_x, __dp_current_exception())"),
            "{core_rendered}"
        );
        assert!(
            core_rendered.contains("x = __dp_current_exception()"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_cell(_dp_cell_x, __dp_current_exception())"),
            "{name_binding_rendered}"
        );
        assert!(
            name_binding_rendered.contains("__dp_del_deref_quietly(_dp_cell_x)"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_except_name_local_binding_moves_to_name_binding_pass() {
        let source = r#"
class Box:
    try:
        raise Exception("boom")
    except Exception as caught:
        seen = str(caught)
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered
                .contains("__dp_setitem(_dp_class_ns, \"caught\", __dp_current_exception())"),
            "{core_rendered}"
        );
        assert!(
            core_rendered.contains("caught = __dp_current_exception()"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered
                .contains("__dp_setitem(_dp_class_ns, \"caught\", __dp_current_exception())"),
            "{name_binding_rendered}"
        );
        assert!(
            name_binding_rendered.contains("__dp_del_quietly(_dp_class_ns, \"caught\")"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_global_named_expr_moves_to_name_binding_pass() {
        let source = r#"
class Box:
    global y
    x = (y := 1)
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global(__dp_globals(), \"y\""),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("y = 1"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_global(__dp_globals(), \"y\", 1)"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_nonlocal_named_expr_moves_to_name_binding_pass() {
        let source = r#"
def outer():
    x = 0
    class Box:
        nonlocal x
        y = (x := 1)
    return x
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_cell(_dp_cell_x, 1)"),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("x = 1"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_cell(_dp_cell_x, 1)"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_global_for_target_moves_to_name_binding_pass() {
        let source = r#"
class Box:
    global y
    for y in [1]:
        pass
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global(__dp_globals(), \"y\""),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("y = _dp_tmp"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_global(__dp_globals(), \"y\", _dp_tmp"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_nonlocal_for_target_moves_to_name_binding_pass() {
        let source = r#"
def outer():
    x = 0
    class Box:
        nonlocal x
        for x in [1]:
            pass
    return x
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_cell(_dp_cell_x, _dp_tmp"),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("x = _dp_tmp"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_cell(_dp_cell_x, _dp_tmp"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_local_with_target_moves_to_name_binding_pass() {
        let source = r#"
from contextlib import nullcontext

class Box:
    with nullcontext(1) as value:
        seen = value
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            core_rendered.contains("value = __dp_contextmanager_enter("),
            "{core_rendered}"
        );
        assert!(
            !core_rendered
                .contains("__dp_setitem(_dp_class_ns, \"value\", __dp_contextmanager_enter("),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered
                .contains("__dp_setitem(_dp_class_ns, \"value\", __dp_contextmanager_enter("),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn class_body_nonlocal_with_target_moves_to_name_binding_pass() {
        let source = r#"
from contextlib import nullcontext

def outer():
    value = "outer"
    class Box:
        nonlocal value
        with nullcontext(1) as value:
            pass
    return value
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_cell(_dp_cell_value, __dp_contextmanager_enter("),
            "{core_rendered}"
        );
        assert!(
            core_rendered.contains("value = __dp_contextmanager_enter("),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered
                .contains("__dp_store_cell(_dp_cell_value, __dp_contextmanager_enter("),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn nested_class_binding_moves_to_name_binding_pass() {
        let source = r#"
class A:
    class B:
        pass
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            core_rendered.contains("B = _dp_define_class_B(_dp_class_ns_B, _dp_class_ns)"),
            "{core_rendered}"
        );
        assert!(
            !core_rendered.contains(
                "__dp_setitem(__dp_load_deleted_name(\"_dp_class_ns\", _dp_class_ns), \"B\","
            ),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains(
                "__dp_setitem(_dp_class_ns, \"B\", _dp_define_class_B(_dp_class_ns_B, _dp_class_ns))"
            ),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn lowered_callable_records_semantic_cell_owner_binding() {
        let source = r#"
def outer():
    def recurse():
        return recurse()
    return recurse
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy_module = lowered.blockpy_module();
        let outer = callable_def_by_name(&blockpy_module, "outer");
        assert_eq!(
            outer.semantic.binding_kind("recurse"),
            Some(crate::block_py::BlockPyBindingKind::Cell(
                crate::block_py::BlockPyCellBindingKind::Owner
            )),
            "{:?}",
            outer.semantic.bindings
        );
    }

    #[test]
    fn closure_backed_generator_does_not_lift_module_globals() {
        let source = r#"
def child():
    yield "start"

def delegator():
    result = yield from child()
    return ("done", result)
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let delegator = function_by_name(&bb_module, "delegator");
        let layout = delegator
            .closure_layout()
            .as_ref()
            .expect("closure-backed generator should record closure layout");
        assert!(
            !layout
                .cellvars
                .iter()
                .any(|slot| slot.logical_name == "child"),
            "{layout:?}"
        );
        assert!(
            !delegator.entry_liveins().iter().any(|name| name == "child"),
            "{delegator:?}"
        );
    }

    #[test]
    fn generator_resume_yield_from_blocks_drop_cell_storage_alias_params() {
        let source = r#"
def child():
    yield "start"

def delegator():
    result = yield from child()
    return ("done", result)
"#;

        let lowered = TrackedLowering::new(source);
        let core_module = lowered
            .result
            .get_pass::<BlockPyModule<CoreBlockPyPass>>("core_blockpy")
            .expect("expected core no-yield pass");
        let resume_function = core_module
            .callable_defs
            .iter()
            .find(|func| func.names.bind_name == "delegator_resume")
            .expect("expected hidden generator resume function");
        let yield_from_except = resume_function
            .blocks
            .iter()
            .find(|block| block.label.as_str().starts_with("yield_from_except_"))
            .expect("expected synthesized yield_from_except block");

        assert!(
            yield_from_except
                .params
                .iter()
                .any(|param| param.name.starts_with("_dp_yield_from_exc_")),
            "{yield_from_except:?}"
        );
        assert!(
            yield_from_except
                .params
                .iter()
                .all(|param| !param.name.starts_with("_dp_cell_")),
            "{yield_from_except:?}"
        );
    }

    #[test]
    fn blockpy_callable_def_retains_docstring_metadata() {
        let source = r#"
def documented():
    "hello doc"
    return 1
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy = lowered.blockpy_module();
        let documented = callable_def_by_name(&blockpy, "documented");
        let doc = documented
            .doc
            .as_ref()
            .expect("callable definition should retain doc metadata");
        assert_eq!(doc, "hello doc");
    }

    #[test]
    fn top_level_function_global_binding_moves_to_name_binding_pass() {
        let source = r#"
def f():
    return 1
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global"),
            "{core_rendered}"
        );
        assert!(
            core_rendered.contains("f = __dp_make_function"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_global(__dp_globals(), \"f\", ")
                && (name_binding_rendered.contains("__dp_make_function(")
                    || name_binding_rendered
                        .contains("__dp_load_global(__dp_globals(), \"__dp_make_function\")",)),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn top_level_global_assign_and_load_move_to_name_binding_pass() {
        let source = r#"
x = 1
y = x
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global"),
            "{core_rendered}"
        );
        assert!(
            !core_rendered.contains("__dp_load_global"),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("x = 1"), "{core_rendered}");
        assert!(core_rendered.contains("y = x"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_global(__dp_globals(), \"x\", 1)"),
            "{name_binding_rendered}"
        );
        assert!(
            name_binding_rendered.contains(
                "__dp_store_global(__dp_globals(), \"y\", __dp_load_global(__dp_globals(), \"x\"))"
            ),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn top_level_global_named_expr_moves_to_name_binding_pass() {
        let source = r#"
def f():
    return 1

x = (y := f())
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global"),
            "{core_rendered}"
        );
        assert!(
            !core_rendered.contains("__dp_load_global"),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("y = f()"), "{core_rendered}");
        assert!(core_rendered.contains("x = y"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains(
                "__dp_store_global(__dp_globals(), \"y\", __dp_load_global(__dp_globals(), \"f\")())"
            ),
            "{name_binding_rendered}"
        );
        assert!(
            name_binding_rendered.contains(
                "__dp_store_global(__dp_globals(), \"x\", __dp_load_global(__dp_globals(), \"y\"))"
            ),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn top_level_comprehension_named_expr_uses_global_decl_then_name_binding_pass() {
        let source = r#"
x = [y := i for i in [1, 2]]
"#;

        let lowered = TrackedLowering::new(source);
        let ast_rendered = lowered.pass_text("ast-to-ast");
        assert!(ast_rendered.contains("global y"), "{ast_rendered}");
        assert!(
            !ast_rendered.contains("__dp_store_global"),
            "{ast_rendered}"
        );

        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global"),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("y = i"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_global(__dp_globals(), \"y\", i)"),
            "{name_binding_rendered}"
        );
        assert!(
            name_binding_rendered.contains("__dp_store_global(__dp_globals(), \"x\", _dp_listcomp"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn top_level_for_target_global_binding_moves_to_name_binding_pass() {
        let source = r#"
for x in [1, 2]:
    pass
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global"),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("x = _dp_tmp"), "{core_rendered}");

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_store_global(__dp_globals(), \"x\", _dp_tmp"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn top_level_except_name_global_binding_moves_to_name_binding_pass() {
        let source = r#"
try:
    raise Exception("boom")
except Exception as exc:
    seen = exc
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_store_global(__dp_globals(), \"exc\""),
            "{core_rendered}"
        );
        assert!(
            core_rendered.contains("_dp_del_quietly(exc)"),
            "{core_rendered}"
        );
        assert!(
            core_rendered.contains("exc = __dp_current_exception()"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered
                .contains("__dp_store_global(__dp_globals(), \"exc\", __dp_current_exception())"),
            "{name_binding_rendered}"
        );
        assert!(
            name_binding_rendered.contains("__dp_del_quietly(__dp_globals(), \"exc\")"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn top_level_global_delete_moves_to_name_binding_pass() {
        let source = r#"
x = 1
del x
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            !core_rendered.contains("__dp_delitem(__dp_globals(), \"x\")"),
            "{core_rendered}"
        );
        assert!(
            core_rendered.contains("x = __dp_DELETED"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered.contains("__dp_delitem(__dp_globals(), \"x\")"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn nonlocal_assign_and_load_move_to_name_binding_pass() {
        let source = r#"
def outer():
    x = 1
    def inner():
        nonlocal x
        x = x + 1
        return x
    return inner()
"#;

        let lowered = TrackedLowering::new(source);
        let core_rendered = lowered.pass_text("core_blockpy");
        assert!(
            core_rendered.contains("x = __dp_add(x, 1)"),
            "{core_rendered}"
        );
        assert!(core_rendered.contains("return x"), "{core_rendered}");
        assert!(
            !core_rendered.contains("__dp_store_cell(_dp_cell_x, __dp_add("),
            "{core_rendered}"
        );
        assert!(
            !core_rendered.contains("return __dp_load_cell(_dp_cell_x)"),
            "{core_rendered}"
        );

        let name_binding_rendered = lowered.name_binding_text();
        assert!(
            name_binding_rendered
                .contains("__dp_store_cell(_dp_cell_x, __dp_add(__dp_load_cell(_dp_cell_x), 1))"),
            "{name_binding_rendered}"
        );
        assert!(
            name_binding_rendered.contains("return __dp_load_cell(_dp_cell_x)"),
            "{name_binding_rendered}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_assert_while_stmt_sequence_still_lowers_it() {
        let source = r#"
def check():
    assert cond, msg
"#;

        let lowered = TrackedLowering::new(source);
        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| matches!(block.term, crate::block_py::BlockPyTerm::IfTerm(_))),
            "{check:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_elif_while_stmt_sequence_still_lowers_it() {
        let source = r#"
def check(a, b):
    if a:
        return 1
    elif b:
        return 2
    else:
        return 3
"#;

        let lowered = TrackedLowering::new(source);
        let check = lowered.bb_function("check");
        let brif_count = check
            .blocks
            .iter()
            .filter(|block| matches!(block.term, crate::block_py::BlockPyTerm::IfTerm(_)))
            .count();
        assert!(brif_count >= 2, "{check:?}");
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_boolop_while_blockpy_expr_lowering_handles_it() {
        let source = r#"
def choose(a, b, c):
    return f(a and b or c)
"#;

        let lowered = TrackedLowering::new(source);
        let choose = lowered.bb_function("choose");
        assert!(
            choose
                .blocks
                .iter()
                .any(|block| matches!(block.term, crate::block_py::BlockPyTerm::IfTerm(_))),
            "{choose:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_if_expr_while_blockpy_expr_lowering_handles_it() {
        let source = r#"
def choose(cond, a, b):
    return f(a if cond else b)
"#;

        let lowered = TrackedLowering::new(source);
        let choose = lowered.bb_function("choose");
        assert!(
            choose
                .blocks
                .iter()
                .any(|block| matches!(block.term, crate::block_py::BlockPyTerm::IfTerm(_))),
            "{choose:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_named_expr_while_blockpy_expr_lowering_handles_it() {
        let source = r#"
def choose(y):
    return f((x := y))
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy_rendered = lowered.blockpy_text();
        assert!(blockpy_rendered.contains("x = y"), "{blockpy_rendered}");
        assert!(
            blockpy_rendered.contains("return f(x)"),
            "{blockpy_rendered}"
        );
        assert!(!blockpy_rendered.contains(":="), "{blockpy_rendered}");
    }

    #[test]
    fn scoped_helper_expr_pass_lowers_listcomp_before_blockpy() {
        let source = r#"
def choose(xs):
    return f([x for x in xs])
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy_rendered = lowered.blockpy_text();
        assert!(
            blockpy_rendered.contains("function choose.<locals>._dp_listcomp_"),
            "{blockpy_rendered}"
        );
        assert!(
            blockpy_rendered.contains("return f(_dp_listcomp"),
            "{blockpy_rendered}"
        );
    }

    #[test]
    fn scoped_helper_expr_pass_lowers_genexpr_before_blockpy() {
        let source = r#"
def choose(xs):
    return tuple(x for x in xs)
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy_rendered = lowered.blockpy_text();
        assert!(
            blockpy_rendered.contains("generator choose.<locals>.<genexpr>("),
            "{blockpy_rendered}"
        );
        assert!(
            blockpy_rendered.contains("return tuple(_dp_genexpr"),
            "{blockpy_rendered}"
        );
    }

    #[test]
    fn scoped_helper_expr_pass_lowers_lambda_before_blockpy() {
        let source = r#"
def choose():
    return f(lambda x: x + 1)
"#;

        let lowered = TrackedLowering::new(source);
        let blockpy_rendered = lowered.blockpy_text();
        assert!(
            blockpy_rendered.contains("function choose.<locals>.<lambda>("),
            "{blockpy_rendered}"
        );
        assert!(
            blockpy_rendered.contains("return f(_dp_lambda"),
            "{blockpy_rendered}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_async_generator_await_while_blockpy_generator_lowering_handles_it(
    ) {
        let source = r#"
class Once:
    def __await__(self):
        yield 1
        return 2

async def agen():
    value = await Once()
    yield value
"#;

        let lowered = TrackedLowering::new(source);
        let semantic_blockpy_rendered = lowered.semantic_blockpy_text();
        assert!(
            semantic_blockpy_rendered.contains("await Once()"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            !semantic_blockpy_rendered.contains("__dp_await_iter"),
            "{semantic_blockpy_rendered}"
        );

        let blockpy_rendered = lowered.core_blockpy_with_yield_text();
        assert!(
            blockpy_rendered.contains("__dp_await_iter"),
            "{blockpy_rendered}"
        );
        assert!(
            !blockpy_rendered.contains("await Once()"),
            "{blockpy_rendered}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_coroutine_await_while_blockpy_generator_lowering_handles_it() {
        let source = r#"
class Once:
    def __await__(self):
        yield 1
        return 2

async def run():
    value = await Once()
    return value
"#;

        let lowered = TrackedLowering::new(source);
        let semantic_blockpy_rendered = lowered.semantic_blockpy_text();
        assert!(
            semantic_blockpy_rendered.contains("await Once()"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            !semantic_blockpy_rendered.contains("__dp_await_iter"),
            "{semantic_blockpy_rendered}"
        );

        let blockpy_rendered = lowered.core_blockpy_with_yield_text();
        assert!(
            blockpy_rendered.contains("__dp_await_iter"),
            "{blockpy_rendered}"
        );
        assert!(
            !blockpy_rendered.contains("await Once()"),
            "{blockpy_rendered}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_async_generator_async_with_while_blockpy_generator_lowering_handles_it(
    ) {
        let source = r#"
async def agen(cm):
    async with cm as value:
        yield value
"#;

        let lowered = TrackedLowering::new(source);
        let semantic_blockpy_rendered = lowered.semantic_blockpy_text();
        assert!(
            semantic_blockpy_rendered.contains("await __dp_asynccontextmanager_aenter"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            semantic_blockpy_rendered.contains("__dp_asynccontextmanager_get_aexit"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            !semantic_blockpy_rendered.contains("__dp_await_iter"),
            "{semantic_blockpy_rendered}"
        );

        let blockpy_rendered = lowered.core_blockpy_with_yield_text();
        assert!(
            blockpy_rendered.contains("__dp_await_iter"),
            "{blockpy_rendered}"
        );
        assert!(
            blockpy_rendered.contains("__dp_asynccontextmanager_aenter"),
            "{blockpy_rendered}"
        );
        assert!(
            !blockpy_rendered.contains("async with cm as value"),
            "{blockpy_rendered}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_coroutine_async_with_while_blockpy_generator_lowering_handles_it(
    ) {
        let source = r#"
async def run(cm):
    async with cm as value:
        return value
"#;

        let lowered = TrackedLowering::new(source);
        let semantic_blockpy_rendered = lowered.semantic_blockpy_text();
        assert!(
            semantic_blockpy_rendered.contains("await __dp_asynccontextmanager_aenter"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            semantic_blockpy_rendered.contains("__dp_asynccontextmanager_get_aexit"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            !semantic_blockpy_rendered.contains("__dp_await_iter"),
            "{semantic_blockpy_rendered}"
        );

        let blockpy_rendered = lowered.core_blockpy_with_yield_text();
        assert!(
            blockpy_rendered.contains("__dp_await_iter"),
            "{blockpy_rendered}"
        );
        assert!(
            blockpy_rendered.contains("__dp_asynccontextmanager_aenter"),
            "{blockpy_rendered}"
        );
        assert!(
            !blockpy_rendered.contains("async with cm as value"),
            "{blockpy_rendered}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_match_while_stmt_sequence_still_lowers_it() {
        let source = r#"
def check(x):
    match x:
        case 1:
            return 10
        case _:
            return 20
"#;

        let lowered = TrackedLowering::new(source);
        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| matches!(block.term, crate::block_py::BlockPyTerm::IfTerm(_))),
            "{check:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_raise_from_while_stmt_sequence_still_lowers_it() {
        let source = r#"
def check():
    raise ValueError() from None
"#;

        let lowered = TrackedLowering::new(source);
        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_raise_from")),
            "{check:?}"
        );
        assert!(
            check
                .blocks
                .iter()
                .any(|block| { matches!(block.term, crate::block_py::BlockPyTerm::Raise(_)) }),
            "{check:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_typed_try_while_later_passes_still_lower_it() {
        let source = r#"
def check():
    try:
        work()
    except ValueError as exc:
        handle(exc)
"#;

        let lowered = TrackedLowering::new(source);
        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_exception_matches")),
            "{check:?}"
        );
        assert!(
            check.blocks.iter().any(|block| block.exc_edge.is_some()),
            "{check:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_try_star_while_later_passes_still_lower_it() {
        let source = r#"
def check():
    try:
        work()
    except* ValueError as exc:
        handle(exc)
"#;

        let lowered = TrackedLowering::new(source);
        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_exceptiongroup_split")),
            "{check:?}"
        );
    }

    #[test]
    fn ast_to_ast_can_lower_import_while_later_passes_still_lower_it() {
        let source = r#"
import pkg.sub as alias
"#;

        let lowered = TrackedLowering::new(source);

        let module_init = lowered.bb_function("_dp_module_init");
        assert!(
            module_init.blocks.iter().any(|block| {
                block_uses_text(block, "__dp_import_(")
                    || block_uses_text(block, "__dp_load_global(__dp_globals(), \"__dp_import_\")")
            }),
            "{module_init:?}"
        );
        assert!(
            module_init.blocks.iter().any(|block| {
                block_uses_text(block, "__dp_import_attr")
                    || block_uses_text(
                        block,
                        "__dp_load_global(__dp_globals(), \"__dp_import_attr\")",
                    )
            }),
            "{module_init:?}"
        );
    }

    #[test]
    fn ast_to_ast_can_lower_import_from_while_later_passes_still_lower_it() {
        let source = r#"
from pkg.mod import name as alias
"#;

        let lowered = TrackedLowering::new(source);

        let module_init = lowered.bb_function("_dp_module_init");
        assert!(
            module_init.blocks.iter().any(|block| {
                block_uses_text(block, "__dp_import_(")
                    || block_uses_text(block, "__dp_load_global(__dp_globals(), \"__dp_import_\")")
            }),
            "{module_init:?}"
        );
        assert!(
            module_init.blocks.iter().any(|block| {
                block_uses_text(block, "__dp_import_attr")
                    || block_uses_text(
                        block,
                        "__dp_load_global(__dp_globals(), \"__dp_import_attr\")",
                    )
            }),
            "{module_init:?}"
        );
    }

    #[test]
    fn ast_to_ast_can_lower_type_alias_while_later_passes_still_lower_it() {
        let source = r#"
type Alias[T] = list[T]
"#;

        let lowered = TrackedLowering::new(source);

        let module_init = lowered.bb_function("_dp_module_init");
        assert!(
            module_init
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_typing_TypeAliasType")),
            "{module_init:?}"
        );
    }

    #[test]
    fn ast_to_ast_can_lower_augassign_while_later_passes_still_lower_it() {
        let source = r#"
def bump(x):
    x += 1
    return x
"#;

        let lowered = TrackedLowering::new(source);

        let bump = lowered.bb_function("bump");
        assert!(
            bump.blocks.iter().any(|block| match block.body.as_slice() {
                [stmt] => matches!(
                    stmt,
                    BbStmt::Assign(assign) if expr_text(&assign.value).contains("__dp_iadd")
                ),
                _ => false,
            }),
            "{bump:?}"
        );
    }

    #[test]
    fn single_named_assignment_leaves_annassign_for_later_passes() {
        let source = r#"
def f():
    x: int = 1
"#;

        let mut module = ruff_python_parser::parse_module(source)
            .expect("parse should succeed")
            .into_syntax();
        let context = crate::passes::ast_to_ast::context::Context::new(Options::for_test(), source);

        crate::passes::ast_to_ast::ast_rewrite::rewrite_with_pass(
            &context,
            Some(&crate::passes::SingleNamedAssignmentPass),
            None,
            crate::passes::ast_to_ast::body::suite_mut(&mut module.body),
        );

        let rendered =
            crate::ruff_ast_to_string(crate::passes::ast_to_ast::body::suite_ref(&module.body));
        assert!(rendered.contains("x: int = 1"), "{rendered}");
    }

    #[test]
    fn closure_backed_generator_records_explicit_closure_layout() {
        let source = r#"
def outer(scale):
    factor = scale
    def gen(a):
        total = a
        yield total + factor
        total = total + 1
        yield total
    return gen
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let gen = function_by_name(&bb_module, "gen");
        let layout = gen
            .closure_layout()
            .as_ref()
            .expect("sync generator should record closure layout");

        let factor = slot_by_name(&layout.freevars, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, ClosureInit::InheritedCapture);

        let a = slot_by_name(&layout.cellvars, "a");
        assert_eq!(a.storage_name, "_dp_cell_a");
        assert_eq!(a.init, ClosureInit::Parameter);

        let total = slot_by_name(&layout.cellvars, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");
        assert_eq!(total.init, ClosureInit::Deferred);

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, ClosureInit::RuntimePcUnstarted);
    }

    #[test]
    fn closure_backed_generator_layout_preserves_try_exception_slots() {
        let source = r#"
def gen():
    try:
        yield 1
    except ValueError:
        return 2
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let gen = function_by_name(&bb_module, "gen");
        let layout = gen
            .closure_layout()
            .as_ref()
            .expect("sync generator should record closure layout");

        let try_exc = layout
            .cellvars
            .iter()
            .find(|slot| slot.logical_name.starts_with("_dp_try_exc_"))
            .unwrap_or_else(|| panic!("missing try-exception slot in {layout:?}"));
        assert_eq!(
            try_exc.storage_name,
            format!("_dp_cell_{}", try_exc.logical_name)
        );
        assert_eq!(try_exc.init, ClosureInit::DeletedSentinel);
        assert!(
            layout
                .runtime_cells
                .iter()
                .any(|slot| slot.logical_name == "_dp_pc"),
            "{layout:?}"
        );
    }

    #[test]
    fn closure_backed_coroutine_records_explicit_closure_layout() {
        let source = r#"
class Once:
    def __await__(self):
        yield 1
        return 2

def outer(scale):
    factor = scale
    async def run():
        total = 1
        total += factor
        total += await Once()
        return total
    return run
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        let layout = run
            .closure_layout()
            .as_ref()
            .expect("closure-backed coroutine should record closure layout");

        let factor = slot_by_name(&layout.freevars, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, ClosureInit::InheritedCapture);

        let total = slot_by_name(&layout.cellvars, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, ClosureInit::RuntimePcUnstarted);
    }

    #[test]
    fn closure_backed_async_generator_records_explicit_closure_layout() {
        let source = r#"
def outer(scale):
    factor = scale
    async def agen():
        total = 1
        yield total + factor
        total += 1
        yield total + factor
    return agen
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let agen = function_by_name(&bb_module, "agen");
        let layout = agen
            .closure_layout()
            .as_ref()
            .expect("closure-backed async generator should record closure layout");

        let factor = slot_by_name(&layout.freevars, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, ClosureInit::InheritedCapture);

        let total = slot_by_name(&layout.cellvars, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, ClosureInit::RuntimePcUnstarted);
    }

    #[test]
    fn async_comprehension_lowering_emits_only_closure_backed_generator_callables() {
        let source = r#"
import asyncio

async def agen():
    for i in range(4):
        await asyncio.sleep(0)
        yield i

async def outer(scale):
    values = [x + scale async for x in agen()]
    return (value * 2 async for value in agen() if value in values)
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let generator_callables = bb_module
            .callable_defs
            .iter()
            .filter(|func| {
                matches!(
                    func.lowered_kind(),
                    BlockPyFunctionKind::Generator | BlockPyFunctionKind::AsyncGenerator
                )
            })
            .collect::<Vec<_>>();
        let generator_names = generator_callables
            .iter()
            .map(|func| format!("{} :: {}", func.names.bind_name, func.names.qualname))
            .collect::<Vec<_>>();
        assert!(
            !generator_callables.is_empty(),
            "expected generator-like BB callables; got {}",
            bb_module
                .callable_defs
                .iter()
                .map(|func| format!("{} :: {}", func.names.bind_name, func.names.qualname))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(
            generator_callables
                .iter()
                .all(|func| func.closure_layout().is_some()),
            "expected only closure-backed generator callables; got {}",
            generator_names.join(", ")
        );
    }

    #[test]
    fn lowers_while_break_continue_into_basic_blocks() {
        let source = r#"
def run(limit):
    i = 0
    out = []
    while i < limit:
        i = i + 1
        if i == 2:
            continue
        if i == 5:
            break
        out.append(i)
    else:
        out.append(99)
    return out, i
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BlockPyTerm::IfTerm(_))),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BlockPyTerm::Jump(_))),
            "{run:?}"
        );
    }

    #[test]
    fn lowers_for_else_break_into_basic_blocks() {
        let source = r#"
def run(items):
    out = []
    for x in items:
        if x == 2:
            break
        out.append(x)
    else:
        out.append(99)
    return out
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_next_or_sentinel")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_iter")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BlockPyTerm::IfTerm(_))),
            "{run:?}"
        );
    }

    #[test]
    fn lowers_async_for_else_directly_without_completed_flag() {
        let source = r#"
async def run():
    async for x in ait:
        body()
    else:
        done()
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        let debug = format!("{run:?}");
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_anext_or_sentinel")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_aiter")),
            "{run:?}"
        );
        assert!(!debug.contains("_dp_completed_"), "{debug}");
    }

    #[test]
    fn semantic_blockpy_lowers_async_for_to_awaited_fetch_before_await_lowering() {
        let source = r#"
async def run():
    async for x in ait:
        body()
"#;

        let lowered = TrackedLowering::new(source);
        let semantic_blockpy_rendered = lowered.semantic_blockpy_text();
        assert!(
            semantic_blockpy_rendered.contains("await __dp_anext_or_sentinel"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            semantic_blockpy_rendered.contains("__dp_aiter"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            !semantic_blockpy_rendered.contains("yield from __dp_await_iter"),
            "{semantic_blockpy_rendered}"
        );
    }

    #[test]
    fn omits_synthetic_end_block_when_unreachable() {
        let source = r#"
def f():
    return 1
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert_eq!(
            f.entry_block().label_str(),
            f.blocks
                .first()
                .expect("f should have a first block")
                .label_str()
        );
        assert_ne!(f.entry_block().label_str(), "start", "{f:?}");
        assert!(
            !f.blocks.iter().any(|block| block.label == "_dp_bb_f_0"),
            "{f:?}"
        );
    }

    #[test]
    fn folds_jump_to_trivial_none_return() {
        let source = r#"
def f():
    x = 1
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_NONE")),
            "{f:?}"
        );
        assert!(
            !f.blocks
                .iter()
                .any(|block| matches!(block.term, BlockPyTerm::Jump(_))),
            "{f:?}"
        );
    }

    #[test]
    fn debug_generator_filter_source_order_ir() {
        let pass_source = r#"
class Field:
    def __init__(self, name, *, init, kw_only=False):
        self.name = name
        self.init = init
        self.kw_only = kw_only

def fields_in_init_order(fields):
    return tuple(
        field.name
        for field in fields
        if field.init and not field.kw_only
    )
"#;
        let fail_source = r#"
def fields_in_init_order(fields):
    return tuple(
        field.name
        for field in fields
        if field.init and not field.kw_only
    )

class Field:
    def __init__(self, name, *, init, kw_only=False):
        self.name = name
        self.init = init
        self.kw_only = kw_only
"#;

        for (name, source) in [("pass", pass_source), ("fail", fail_source)] {
            let lowered = transform_str_to_ruff_with_options(source, Options::for_test())
                .expect("transform should succeed");
            let blockpy = lowered
                .get_pass::<crate::block_py::BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
                .cloned()
                .expect("expected lowered semantic BlockPy module");
            let blockpy_rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
            eprintln!("==== {name} BLOCKPY ====\n{blockpy_rendered}");

            let bb_module = transform_str_to_bb_ir_with_options(source, Options::for_test())
                .expect("transform should succeed")
                .expect("bb module should be available");
            let function_names = bb_module
                .callable_defs
                .iter()
                .map(|func| format!("{} :: {}", func.names.bind_name, func.names.qualname))
                .collect::<Vec<_>>();
            eprintln!(
                "==== {name} BB FUNCTIONS ====\n{}",
                function_names.join("\n")
            );
            let gen = bb_module
                .callable_defs
                .iter()
                .find(|func| func.names.bind_name.contains("_dp_genexpr"))
                .unwrap_or_else(|| panic!("missing genexpr helper in {name}"));
            eprintln!("==== {name} BB {:?} ====\n{gen:#?}", gen.names.qualname);

            let prepared = crate::passes::lower_try_jump_exception_flow(&bb_module)
                .expect("jit prep should succeed");
            let prepared_gen = prepared
                .callable_defs
                .iter()
                .find(|func| func.names.bind_name.contains("_dp_genexpr"))
                .unwrap_or_else(|| panic!("missing prepared genexpr helper in {name}"));
            for label in ["_dp_bb__dp_genexpr_1_44", "_dp_bb__dp_genexpr_1_45"] {
                if let Some(block) = prepared_gen
                    .blocks
                    .iter()
                    .find(|block| block.label == label)
                {
                    eprintln!("==== {name} PREPARED {label} ====\n{block:#?}");
                }
            }
        }
    }

    #[test]
    fn closure_backed_simple_generator_records_one_resume_per_yield() {
        let source = r#"
def make_counter(delta):
    outer_capture = delta
    def gen():
        total = 1
        total += outer_capture
        sent = yield total
        total += sent
        yield total
    return gen()
"#;

        let bb_module = transform_str_to_bb_ir_with_options(source, Options::for_test())
            .expect("transform should succeed")
            .expect("bb module should be available");
        let gen = bb_module
            .callable_defs
            .iter()
            .find(|func| func.names.bind_name == "gen")
            .expect("missing visible generator factory");
        assert_eq!(gen.lowered_kind(), &BlockPyFunctionKind::Generator);
    }

    #[test]
    fn lowers_outer_with_nested_nonlocal_inner() {
        let source = r#"
def outer():
    x = 5
    def inner():
        nonlocal x
        x = 2
        return x
    return inner()
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let outer = function_by_name(&bb_module, "outer");
        let inner = function_by_name(&bb_module, "inner");
        assert_eq!(
            outer.entry_block().label_str(),
            outer
                .blocks
                .first()
                .expect("outer should have a first block")
                .label_str()
        );
        assert_eq!(
            inner.entry_block().label_str(),
            inner
                .blocks
                .first()
                .expect("inner should have a first block")
                .label_str()
        );
        assert_ne!(outer.entry_block().label_str(), "start", "{outer:?}");
        assert_ne!(inner.entry_block().label_str(), "start", "{inner:?}");
        assert!(
            outer
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "_dp_cell_x")),
            "{outer:?}"
        );
    }

    #[test]
    fn lowers_try_finally_with_return_via_dispatch() {
        let source = r#"
def f(x):
    try:
        if x:
            return 1
    finally:
        cleanup()
    return 2
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks.iter().any(|block| block.exc_edge.is_some()),
            "{f:?}"
        );
        let debug = format!("{f:?}");
        assert!(!debug.contains("finally:"), "{debug}");
        assert!(!debug.contains("_dp_try_reason_"), "{debug}");
        assert!(!debug.contains("_dp_try_value_"), "{debug}");
        assert!(debug.contains("_dp_try_abrupt_kind_"), "{debug}");
        assert!(debug.contains("_dp_try_abrupt_payload_"), "{debug}");
    }

    #[test]
    fn lowers_nested_with_cleanup_and_inner_return_without_hanging() {
        let source = r#"
from pathlib import Path
import tempfile

def run():
    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "example.txt"
        with open(path, "w", encoding="utf8") as handle:
            handle.write("payload")
        with open(path, "r", encoding="utf8") as handle:
            return "ok"
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        assert!(
            run.blocks.iter().any(|block| block.exc_edge.is_some()),
            "{run:?}"
        );
    }

    #[test]
    fn lowers_plain_try_except_with_try_jump_dispatch() {
        let source = r#"
try:
    print(1)
except Exception:
    print(2)
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let init_fn = function_by_name(&bb_module, "_dp_module_init");
        assert!(
            init_fn.blocks.iter().any(|block| block.exc_edge.is_some()),
            "{init_fn:?}"
        );
    }

    #[test]
    fn module_init_rebinds_lowered_top_level_function_defs() {
        let source = r#"
def outer_read():
    x = 5

    def inner():
        return x

    return inner
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let init_fn = function_by_name(&bb_module, "_dp_module_init");
        assert!(
            init_fn
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_store_global")),
            "{init_fn:?}"
        );
        assert!(
            init_fn
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "\"outer_read\"")),
            "{init_fn:?}"
        );
    }

    #[test]
    fn ast_to_ast_module_init_does_not_inject_global_prelude() {
        let source = r#"
VALUE = 1

def build():
    return VALUE

class Box:
    item = VALUE
"#;

        let lowered = TrackedLowering::new(source);
        let rendered = lowered.pass_text("ast-to-ast");
        assert!(rendered.contains("def _dp_module_init()"), "{rendered}");
        assert!(!rendered.contains("global VALUE"), "{rendered}");
        assert!(!rendered.contains("global build"), "{rendered}");
        assert!(!rendered.contains("global Box"), "{rendered}");
    }

    #[test]
    fn module_init_rebinds_top_level_assignments_and_classes_without_global_prelude() {
        let source = r#"
VALUE = 1

class Box:
    item = VALUE
"#;

        let lowered = TrackedLowering::new(source);
        let rendered = lowered.pass_text("ast-to-ast");
        assert!(!rendered.contains("global VALUE"), "{rendered}");
        assert!(!rendered.contains("global Box"), "{rendered}");

        let init_fn = lowered.bb_function("_dp_module_init");
        assert!(
            init_fn
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_store_global")),
            "{init_fn:?}"
        );
        assert!(
            init_fn
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "\"VALUE\"")),
            "{init_fn:?}"
        );
        assert!(
            init_fn
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "\"Box\"")),
            "{init_fn:?}"
        );
    }

    #[test]
    fn lowers_try_star_except_star_via_exceptiongroup_split() {
        let source = r#"
def f():
    try:
        raise ExceptionGroup("eg", [ValueError(1)])
    except* ValueError as exc:
        return exc
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_exceptiongroup_split")),
            "{f:?}"
        );
        assert!(
            f.blocks.iter().any(|block| block.exc_edge.is_some()),
            "{f:?}"
        );
    }

    #[test]
    fn dead_tail_local_binding_still_raises_unbound() {
        let source = r#"
def f():
    print(x)
    return
    x = 1
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        let debug = format!("{f:?}");
        assert!(debug.contains("load_deleted_name"), "{debug}");
        assert!(debug.contains("DELETED"), "{debug}");
        assert!(!debug.contains("x = 1"), "{debug}");
    }

    #[test]
    fn matches_dp_lookup_call_with_decoded_name_arg() {
        let expr =
            py_expr!("__dp_getattr(__dp__, __dp_decode_literal_bytes(b\"current_exception\"))");
        assert!(crate::block_py::exception::is_dp_lookup_call(
            &expr,
            "current_exception",
        ));
    }
}

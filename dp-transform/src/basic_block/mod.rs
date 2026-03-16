mod annotation_export;
mod ast_symbol_analysis;
pub(crate) mod ast_to_ast;
mod await_lower;
pub mod bb_ir;
pub mod block_py;
mod blockpy_expr_simplify;
mod blockpy_generators;
mod blockpy_to_bb;
mod bound_names;
pub(crate) mod cfg_ir;
mod expr_utils;
mod function_identity;
mod function_lowering;
mod param_specs;
mod ruff_to_blockpy;
mod stmt_utils;

// Ruff AST -> BbModule
pub use block_py::pretty::blockpy_module_to_string;
pub(crate) use blockpy_to_bb::{
    core_blockpy_module_bundle_without_await_or_yield_to_bundle,
    lower_awaits_in_lowered_blockpy_module_bundle_plan,
    lower_awaits_in_lowered_core_blockpy_module_bundle,
    lower_core_blockpy_module_bundle_to_bb_module,
    lower_generators_in_lowered_blockpy_module_bundle_plan,
    lower_yield_in_lowered_blockpy_module_export_plan,
    lower_yield_in_lowered_core_blockpy_module_bundle,
    resolved_lowered_blockpy_module_bundle_plan_to_export_plan,
    semantic_blockpy_module_bundle_without_yield_to_bundle,
    simplify_lowered_blockpy_module_bundle_exprs, SemanticBlockPyModulePlanWithAwaits,
};
pub use blockpy_to_bb::{lower_try_jump_exception_flow, normalize_bb_module_for_codegen};
pub use blockpy_to_bb::{
    project_lowered_module_callable_defs, LoweredBlockPyModuleBundle,
    LoweredCoreBlockPyModuleBundle,
};
pub use function_lowering::SingleNamedAssignmentPass;

#[cfg(test)]
mod tests {
    use crate::basic_block::bb_ir::BindingTarget;
    use crate::basic_block::bb_ir::{BbBlock, BbExpr, BbFunction, BbFunctionKind, BbModule};
    use crate::basic_block::bb_ir::{BbClosureInit, BbClosureSlot, BbOp, BbTerm};
    use crate::basic_block::block_py::BlockPyModule;
    use crate::{
        py_expr, transform_str_to_bb_ir_with_options, transform_str_to_ruff_with_options, Options,
    };
    use crate::{LoweringResult, RewrittenAstAfterInitialSimplify};
    use ruff_python_ast::StmtBody;

    struct TrackedLowering {
        result: LoweringResult,
    }

    impl TrackedLowering {
        fn new(source: &str) -> Self {
            Self {
                result: transform_str_to_ruff_with_options(source, Options::for_test())
                    .expect("transform should succeed"),
            }
        }

        fn rewritten_ast(&self) -> &StmtBody {
            self.result
                .get_pass::<RewrittenAstAfterInitialSimplify>()
                .map(|pass| &pass.0)
                .expect("expected post-initial-simplify rewritten Ruff AST pass")
        }

        fn rewritten_ast_text(&self) -> String {
            crate::ruff_ast_to_string(self.rewritten_ast())
        }

        fn blockpy_module(&self) -> BlockPyModule {
            let bundle = self
                .result
                .get_pass::<crate::basic_block::LoweredBlockPyModuleBundle>()
                .expect("expected lowered semantic BlockPy bundle");
            crate::basic_block::project_lowered_module_callable_defs(bundle, |lowered| {
                lowered.callable_def()
            })
        }

        fn blockpy_text(&self) -> String {
            crate::basic_block::blockpy_module_to_string(&self.blockpy_module())
        }

        fn core_blockpy_module(&self) -> crate::basic_block::block_py::CoreBlockPyModule {
            let bundle = self
                .result
                .get_pass::<crate::basic_block::LoweredCoreBlockPyModuleBundle>()
                .expect("expected lowered core BlockPy bundle");
            crate::basic_block::project_lowered_module_callable_defs(bundle, |lowered| {
                lowered.callable_def()
            })
        }

        fn core_blockpy_text(&self) -> String {
            crate::basic_block::blockpy_module_to_string(&self.core_blockpy_module())
        }

        fn bb_module(&self) -> &BbModule {
            self.result
                .bb_module
                .as_ref()
                .expect("bb module should be available")
        }

        fn bb_function(&self, bind_name: &str) -> &BbFunction {
            function_by_name(self.bb_module(), bind_name)
        }
    }

    fn function_by_name<'a>(bb_module: &'a BbModule, bind_name: &str) -> &'a BbFunction {
        let direct = bb_module
            .callable_defs
            .iter()
            .find(|func| func.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing lowered function {bind_name}; got {:?}", bb_module));
        if direct.closure_layout.is_some() {
            return direct;
        }
        bb_module
            .callable_defs
            .iter()
            .find(|func| func.bind_name == format!("{bind_name}_resume"))
            .unwrap_or(direct)
    }

    fn slot_by_name<'a>(slots: &'a [BbClosureSlot], logical_name: &str) -> &'a BbClosureSlot {
        slots
            .iter()
            .find(|slot| slot.logical_name == logical_name)
            .unwrap_or_else(|| panic!("missing closure slot {logical_name}; got {slots:?}"))
    }

    fn expr_text(expr: &BbExpr) -> String {
        crate::ruff_ast_to_string(&expr.to_expr())
    }

    fn callable_def_by_name<'a>(
        blockpy_module: &'a crate::basic_block::block_py::BlockPyModule,
        bind_name: &str,
    ) -> &'a crate::basic_block::block_py::BlockPyCallableDef {
        blockpy_module
            .callable_defs
            .iter()
            .find(|callable| callable.bind_name == bind_name)
            .unwrap_or_else(|| {
                panic!("missing callable definition {bind_name}; got {blockpy_module:?}")
            })
    }

    fn block_uses_text(block: &BbBlock, needle: &str) -> bool {
        block.body.iter().any(|op| match op {
            BbOp::Assign(assign) => expr_text(&assign.value).contains(needle),
            BbOp::Expr(expr) => expr_text(&expr.value).contains(needle),
            BbOp::Delete(delete) => delete
                .targets
                .iter()
                .any(|expr| expr_text(expr).contains(needle)),
        }) || match &block.term {
            BbTerm::BrIf { test, .. } => expr_text(&test).contains(needle),
            BbTerm::BrTable { index, .. } => expr_text(&index).contains(needle),
            BbTerm::Raise { exc, cause } => {
                exc.as_ref()
                    .is_some_and(|value| expr_text(value).contains(needle))
                    || cause
                        .as_ref()
                        .is_some_and(|value| expr_text(value).contains(needle))
            }
            BbTerm::Ret(value) => value
                .as_ref()
                .is_some_and(|ret| expr_text(ret).contains(needle)),
            _ => false,
        }
    }

    fn assert_rewritten_ast_contains(lowered: &TrackedLowering, needle: &str) {
        let rendered = lowered.rewritten_ast_text();
        assert!(rendered.contains(needle), "{rendered}");
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_fstring_while_core_blockpy_expr_simplify_handles_it() {
        let source = r#"
def fmt(value):
    return f"{value=}"
"#;

        let lowered = TrackedLowering::new(source);
        assert_rewritten_ast_contains(&lowered, "f\"{value=}\"");

        let blockpy = lowered.blockpy_text();
        assert!(blockpy.contains("f\"{value=}\""), "{blockpy}");

        let core_blockpy = lowered.core_blockpy_text();
        assert!(core_blockpy.contains("\"value=\""), "{core_blockpy}");
        assert!(
            core_blockpy.contains("__dp_format(__dp_repr(value))"),
            "{core_blockpy}"
        );

        let fmt = lowered.bb_function("fmt");
        assert!(
            fmt.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_format(__dp_repr(value))")),
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
        assert_rewritten_ast_contains(&lowered, "t\"{value}\"");

        let blockpy = lowered.blockpy_text();
        assert!(blockpy.contains("t\"{value}\""), "{blockpy}");

        let core_blockpy = lowered.core_blockpy_text();
        assert!(
            core_blockpy.contains("__dp_templatelib_Interpolation(value, \"value\", None, \"\")"),
            "{core_blockpy}"
        );

        let fmt = lowered.bb_function("fmt");
        assert!(
            fmt.blocks.iter().any(|block| block_uses_text(
                block,
                "__dp_templatelib_Interpolation(value, \"value\", None, \"\")"
            )),
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
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
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
            .find(|func| func.bind_name == "foo")
            .expect("foo should be lowered");
        assert_eq!(foo.entry_label(), "start", "{:?}", foo.entry_label());
        assert!(!foo.blocks.is_empty());
    }

    #[test]
    fn nested_global_function_def_lowers_as_module_global() {
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
            inner_global_function.binding_target,
            BindingTarget::ModuleGlobal,
            "{inner_global_function:?}"
        );
        assert_eq!(inner_global_function.qualname, "inner_global_function");
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
            .closure_layout
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
            !delegator.entry_liveins.iter().any(|name| name == "child"),
            "{delegator:?}"
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
        assert_eq!(crate::ruff_ast_to_string(doc).trim(), "\"hello doc\"");
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_assert_while_stmt_sequence_still_lowers_it() {
        let source = r#"
def check():
    assert cond, msg
"#;

        let lowered = TrackedLowering::new(source);
        assert_rewritten_ast_contains(&lowered, "assert cond, msg");

        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| matches!(block.term, crate::basic_block::bb_ir::BbTerm::BrIf { .. })),
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
        assert_rewritten_ast_contains(&lowered, "elif b");

        let check = lowered.bb_function("check");
        let brif_count = check
            .blocks
            .iter()
            .filter(|block| matches!(block.term, crate::basic_block::bb_ir::BbTerm::BrIf { .. }))
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
        assert_rewritten_ast_contains(&lowered, "a and b or c");

        let choose = lowered.bb_function("choose");
        assert!(
            choose
                .blocks
                .iter()
                .any(|block| matches!(block.term, crate::basic_block::bb_ir::BbTerm::BrIf { .. })),
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
        assert_rewritten_ast_contains(&lowered, "a if cond else b");

        let choose = lowered.bb_function("choose");
        assert!(
            choose
                .blocks
                .iter()
                .any(|block| matches!(block.term, crate::basic_block::bb_ir::BbTerm::BrIf { .. })),
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
        assert_rewritten_ast_contains(&lowered, "(x := y)");

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
            blockpy_rendered.contains("function _dp_listcomp"),
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
            blockpy_rendered.contains("function _dp_genexpr"),
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
            blockpy_rendered.contains("function _dp_lambda"),
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
        assert_rewritten_ast_contains(&lowered, "value = await Once()");

        let blockpy_rendered = lowered.blockpy_text();
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
        assert_rewritten_ast_contains(&lowered, "value = await Once()");

        let blockpy_rendered = lowered.blockpy_text();
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
        assert_rewritten_ast_contains(&lowered, "async with cm as value");

        let blockpy_rendered = lowered.blockpy_text();
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
        assert_rewritten_ast_contains(&lowered, "async with cm as value");

        let blockpy_rendered = lowered.blockpy_text();
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
        assert_rewritten_ast_contains(&lowered, "match x");

        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| matches!(block.term, crate::basic_block::bb_ir::BbTerm::BrIf { .. })),
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
        assert_rewritten_ast_contains(&lowered, "raise ValueError() from None");

        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_raise_from")),
            "{check:?}"
        );
        assert!(
            check.blocks.iter().any(|block| {
                matches!(block.term, crate::basic_block::bb_ir::BbTerm::Raise { .. })
            }),
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
        assert_rewritten_ast_contains(&lowered, "except ValueError as exc");

        let check = lowered.bb_function("check");
        assert!(
            check
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_exception_matches")),
            "{check:?}"
        );
        assert!(
            check
                .blocks
                .iter()
                .any(|block| block.meta.exc_target_label.is_some()),
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
        assert_rewritten_ast_contains(&lowered, "except* ValueError as exc");

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
    fn rewritten_ruff_ast_can_keep_import_while_later_passes_still_lower_it() {
        let source = r#"
import pkg.sub as alias
"#;

        let lowered = TrackedLowering::new(source);
        assert_rewritten_ast_contains(&lowered, "import pkg.sub as alias");

        let module_init = lowered.bb_function("_dp_module_init");
        assert!(
            module_init
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_import_(")),
            "{module_init:?}"
        );
        assert!(
            module_init
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_import_attr")),
            "{module_init:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_import_from_while_later_passes_still_lower_it() {
        let source = r#"
from pkg.mod import name as alias
"#;

        let lowered = TrackedLowering::new(source);
        assert_rewritten_ast_contains(&lowered, "from pkg.mod import name as alias");

        let module_init = lowered.bb_function("_dp_module_init");
        assert!(
            module_init
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_import_(")),
            "{module_init:?}"
        );
        assert!(
            module_init
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_import_attr")),
            "{module_init:?}"
        );
    }

    #[test]
    fn rewritten_ruff_ast_can_keep_type_alias_while_later_passes_still_lower_it() {
        let source = r#"
type Alias[T] = list[T]
"#;

        let lowered = TrackedLowering::new(source);
        assert_rewritten_ast_contains(&lowered, "type Alias[T] = ");

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
    fn rewritten_ruff_ast_can_keep_augassign_while_later_passes_still_lower_it() {
        let source = r#"
def bump(x):
    x += 1
    return x
"#;

        let lowered = TrackedLowering::new(source);
        assert_rewritten_ast_contains(&lowered, "x += 1");

        let bump = lowered.bb_function("bump");
        assert!(
            bump.blocks.iter().any(|block| match block.body.as_slice() {
                [BbOp::Assign(assign)] => expr_text(&assign.value).contains("__dp_iadd"),
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
        let context =
            crate::basic_block::ast_to_ast::context::Context::new(Options::for_test(), source);

        crate::basic_block::ast_to_ast::ast_rewrite::rewrite_with_pass(
            &context,
            Some(&crate::basic_block::SingleNamedAssignmentPass),
            None,
            &mut module.body,
        );

        let rendered = crate::ruff_ast_to_string(&module.body);
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
            .closure_layout
            .as_ref()
            .expect("sync generator should record closure layout");

        let factor = slot_by_name(&layout.freevars, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, BbClosureInit::InheritedCapture);

        let a = slot_by_name(&layout.cellvars, "a");
        assert_eq!(a.storage_name, "_dp_cell_a");
        assert_eq!(a.init, BbClosureInit::Parameter);

        let total = slot_by_name(&layout.cellvars, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");
        assert_eq!(total.init, BbClosureInit::Deferred);

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, BbClosureInit::RuntimePcUnstarted);
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
            .closure_layout
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
        assert_eq!(try_exc.init, BbClosureInit::DeletedSentinel);
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
            .closure_layout
            .as_ref()
            .expect("closure-backed coroutine should record closure layout");

        let factor = slot_by_name(&layout.freevars, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, BbClosureInit::InheritedCapture);

        let total = slot_by_name(&layout.cellvars, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, BbClosureInit::RuntimePcUnstarted);
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
            .closure_layout
            .as_ref()
            .expect("closure-backed async generator should record closure layout");

        let factor = slot_by_name(&layout.freevars, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, BbClosureInit::InheritedCapture);

        let total = slot_by_name(&layout.cellvars, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, BbClosureInit::RuntimePcUnstarted);
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
                    func.kind,
                    BbFunctionKind::Generator { .. } | BbFunctionKind::AsyncGenerator { .. }
                )
            })
            .collect::<Vec<_>>();
        let generator_names = generator_callables
            .iter()
            .map(|func| format!("{} :: {}", func.bind_name, func.qualname))
            .collect::<Vec<_>>();
        assert!(
            !generator_callables.is_empty(),
            "expected generator-like BB callables; got {}",
            bb_module
                .callable_defs
                .iter()
                .map(|func| format!("{} :: {}", func.bind_name, func.qualname))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(
            generator_callables
                .iter()
                .all(|func| func.closure_layout.is_some()),
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
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::Jump(_))),
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
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
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
        assert_eq!(f.entry_label(), "start", "{f:?}");
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
                .any(|block| matches!(block.term, BbTerm::Ret(None))),
            "{f:?}"
        );
        assert!(
            !f.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::Jump(_))),
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
                .get_pass::<crate::basic_block::LoweredBlockPyModuleBundle>()
                .map(|bundle| {
                    crate::basic_block::project_lowered_module_callable_defs(bundle, |lowered| {
                        lowered.callable_def()
                    })
                })
                .expect("expected lowered semantic BlockPy bundle");
            let blockpy_rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
            eprintln!("==== {name} BLOCKPY ====\n{blockpy_rendered}");

            let bb_module = transform_str_to_bb_ir_with_options(source, Options::for_test())
                .expect("transform should succeed")
                .expect("bb module should be available");
            let function_names = bb_module
                .callable_defs
                .iter()
                .map(|func| format!("{} :: {}", func.bind_name, func.qualname))
                .collect::<Vec<_>>();
            eprintln!(
                "==== {name} BB FUNCTIONS ====\n{}",
                function_names.join("\n")
            );
            let gen = bb_module
                .callable_defs
                .iter()
                .find(|func| func.bind_name.contains("_dp_genexpr"))
                .unwrap_or_else(|| panic!("missing genexpr helper in {name}"));
            eprintln!("==== {name} BB {:?} ====\n{gen:#?}", gen.qualname);

            let prepared = crate::basic_block::lower_try_jump_exception_flow(&bb_module)
                .expect("jit prep should succeed");
            let prepared_gen = prepared
                .callable_defs
                .iter()
                .find(|func| func.bind_name.contains("_dp_genexpr"))
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
        let gen = function_by_name(&bb_module, "gen");
        let crate::basic_block::bb_ir::BbFunctionKind::Generator { resume_pcs, .. } = &gen.kind
        else {
            panic!("expected generator kind, got {:?}", gen.kind);
        };
        assert_eq!(resume_pcs.len(), 3, "{resume_pcs:?}");
        assert_eq!(
            resume_pcs.iter().map(|(_, pc)| *pc).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "{resume_pcs:?}"
        );
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
        assert_eq!(outer.entry_label(), "start", "{outer:?}");
        assert_eq!(inner.entry_label(), "start", "{inner:?}");
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
            f.blocks
                .iter()
                .any(|block| block.meta.exc_target_label.is_some()),
            "{f:?}"
        );
        let debug = format!("{f:?}");
        assert!(!debug.contains("finally:"), "{debug}");
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
            run.blocks
                .iter()
                .any(|block| block.meta.exc_target_label.is_some()),
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
        let module_init = bb_module
            .module_init
            .as_ref()
            .expect("module init should be present");
        let init_fn = function_by_name(&bb_module, module_init);
        assert!(
            init_fn
                .blocks
                .iter()
                .any(|block| block.meta.exc_target_label.is_some()),
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
        let module_init = bb_module
            .module_init
            .as_ref()
            .expect("module init should be present");
        let init_fn = function_by_name(&bb_module, module_init);
        let debug = format!("{init_fn:?}");
        assert!(debug.contains("__dp_store_global"), "{debug}");
        assert!(debug.contains("outer_read"), "{debug}");
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
            f.blocks
                .iter()
                .any(|block| block.meta.exc_target_label.is_some()),
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
        assert!(crate::basic_block::block_py::exception::is_dp_lookup_call(
            &expr,
            "current_exception",
        ));
    }
}

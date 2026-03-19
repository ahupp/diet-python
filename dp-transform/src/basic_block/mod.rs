mod annotation_export;
mod ast_symbol_analysis;
pub(crate) mod ast_to_ast;
mod await_lower;
pub mod bb_ir;
pub mod block_py;
mod blockpy_expr_simplify;
mod blockpy_generators;
mod bound_names;
mod cfg_trace;
mod core_await_lower;
mod core_eval_order;
mod function_identity;
mod function_lowering;
pub mod ruff_to_blockpy;
mod stmt_utils;

// Ruff AST -> BbModule
pub use block_py::pretty::blockpy_module_to_string;
pub mod blockpy_to_bb;
pub use blockpy_to_bb::project_lowered_module_callable_defs;
pub(crate) use blockpy_to_bb::LoweredBlockPyModuleBundlePlan;
pub(crate) use blockpy_to_bb::{
    lower_awaits_in_lowered_core_blockpy_module_bundle, lower_blockpy_module_plan_to_bundle,
    lower_core_blockpy_module_bundle_to_bb_module,
    lower_yield_in_lowered_core_blockpy_module_bundle,
    lowered_blockpy_module_bundle_plan_to_semantic_blockpy_module,
    simplify_lowered_blockpy_module_bundle_exprs,
};
pub use blockpy_to_bb::{lower_try_jump_exception_flow, normalize_bb_module_for_codegen};
pub(crate) use core_eval_order::make_eval_order_explicit_in_lowered_core_blockpy_module_bundle;
pub use function_lowering::SingleNamedAssignmentPass;

use crate::basic_block::block_py::{
    BlockPyModule, CfgModule, CoreBlockPyExpr, CoreBlockPyExprWithoutAwait,
    CoreBlockPyExprWithoutAwaitOrYield,
};
use crate::basic_block::blockpy_to_bb::{
    LoweredCoreBlockPyFunction, LoweredCoreBlockPyFunctionWithoutAwait,
    LoweredCoreBlockPyFunctionWithoutAwaitOrYield,
};
use crate::basic_block::ruff_to_blockpy::LoweredBlockPyFunction;
use crate::transformer::Transformer;
use ruff_python_ast::{self as ast, Expr, Stmt};

#[derive(Default)]
struct RuffExprShapeCollector {
    summary: crate::PassShapeSummary,
}

impl Transformer for RuffExprShapeCollector {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Await(_) => self.summary.contains_await = true,
            Expr::Yield(_) | Expr::YieldFrom(_) => self.summary.contains_yield = true,
            Expr::Call(call)
                if matches!(
                    call.func.as_ref(),
                    Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "__dp_add"
                ) =>
            {
                self.summary.contains_dp_add = true;
            }
            _ => {}
        }
        crate::transformer::walk_expr(self, expr);
    }
}

fn merge_pass_shape_summary(total: &mut crate::PassShapeSummary, part: crate::PassShapeSummary) {
    total.contains_await |= part.contains_await;
    total.contains_yield |= part.contains_yield;
    total.contains_dp_add |= part.contains_dp_add;
}

fn summarize_ruff_expr(expr: &Expr) -> crate::PassShapeSummary {
    let mut expr = expr.clone();
    let mut collector = RuffExprShapeCollector::default();
    collector.visit_expr(&mut expr);
    collector.summary
}

fn summarize_ruff_stmt(stmt: &Stmt) -> crate::PassShapeSummary {
    let mut stmt = stmt.clone();
    let mut collector = RuffExprShapeCollector::default();
    collector.visit_stmt(&mut stmt);
    collector.summary
}

fn summarize_ruff_stmt_list(stmts: &[Box<Stmt>]) -> crate::PassShapeSummary {
    let mut summary = crate::PassShapeSummary::default();
    for stmt in stmts {
        merge_pass_shape_summary(&mut summary, summarize_ruff_stmt(stmt));
    }
    summary
}

fn summarize_blockpy_stmt_fragment<E: Clone + Into<Expr>>(
    fragment: &block_py::BlockPyStmtFragment<E>,
    summary: &mut crate::PassShapeSummary,
) {
    for stmt in &fragment.body {
        summarize_blockpy_stmt(stmt, summary);
    }
    if let Some(term) = &fragment.term {
        summarize_blockpy_term(term, summary);
    }
}

fn summarize_blockpy_stmt<E: Clone + Into<Expr>>(
    stmt: &block_py::BlockPyStmt<E>,
    summary: &mut crate::PassShapeSummary,
) {
    match stmt {
        block_py::BlockPyStmt::Assign(assign) => {
            merge_pass_shape_summary(summary, summarize_ruff_expr(&assign.value.clone().into()));
        }
        block_py::BlockPyStmt::Expr(expr) => {
            merge_pass_shape_summary(summary, summarize_ruff_expr(&expr.clone().into()));
        }
        block_py::BlockPyStmt::Delete(_) => {}
        block_py::BlockPyStmt::If(if_stmt) => {
            merge_pass_shape_summary(summary, summarize_ruff_expr(&if_stmt.test.clone().into()));
            summarize_blockpy_stmt_fragment(&if_stmt.body, summary);
            summarize_blockpy_stmt_fragment(&if_stmt.orelse, summary);
        }
    }
}

fn summarize_blockpy_term<E: Clone + Into<Expr>>(
    term: &block_py::BlockPyTerm<E>,
    summary: &mut crate::PassShapeSummary,
) {
    match term {
        block_py::BlockPyTerm::Jump(_) | block_py::BlockPyTerm::TryJump(_) => {}
        block_py::BlockPyTerm::IfTerm(if_term) => {
            merge_pass_shape_summary(summary, summarize_ruff_expr(&if_term.test.clone().into()));
        }
        block_py::BlockPyTerm::BranchTable(branch) => {
            merge_pass_shape_summary(summary, summarize_ruff_expr(&branch.index.clone().into()));
        }
        block_py::BlockPyTerm::Raise(raise) => {
            if let Some(exc) = &raise.exc {
                merge_pass_shape_summary(summary, summarize_ruff_expr(&exc.clone().into()));
            }
        }
        block_py::BlockPyTerm::Return(value) => {
            if let Some(value) = value {
                merge_pass_shape_summary(summary, summarize_ruff_expr(&value.clone().into()));
            }
        }
    }
}

fn summarize_blockpy_module<E: Clone + Into<Expr>>(
    module: &block_py::BlockPyModule<E>,
) -> crate::PassShapeSummary {
    let mut summary = crate::PassShapeSummary::default();
    for callable in &module.callable_defs {
        for block in &callable.blocks {
            for stmt in &block.body {
                summarize_blockpy_stmt(stmt, &mut summary);
            }
            summarize_blockpy_term(&block.term, &mut summary);
        }
    }
    summary
}

fn summarize_semantic_blockpy_plan(
    plan: &LoweredBlockPyModuleBundlePlan,
) -> crate::PassShapeSummary {
    summarize_blockpy_module(&lowered_blockpy_module_bundle_plan_to_semantic_blockpy_module(plan))
}

fn summarize_semantic_blockpy_bundle(
    bundle: &CfgModule<LoweredBlockPyFunction>,
) -> crate::PassShapeSummary {
    let blockpy = project_lowered_module_callable_defs(
        bundle,
        |lowered| -> &crate::basic_block::block_py::BlockPyCallableDef<Expr> { lowered },
    );
    summarize_blockpy_module(&blockpy)
}

fn summarize_core_blockpy_bundle(
    bundle: &CfgModule<LoweredCoreBlockPyFunction>,
) -> crate::PassShapeSummary {
    let blockpy = project_lowered_module_callable_defs(
        bundle,
        |lowered| -> &crate::basic_block::block_py::BlockPyCallableDef<CoreBlockPyExpr> { lowered },
    );
    summarize_blockpy_module(&blockpy)
}

fn summarize_core_blockpy_module(
    module: &block_py::BlockPyModule<CoreBlockPyExpr>,
) -> crate::PassShapeSummary {
    summarize_blockpy_module(module)
}

fn summarize_core_blockpy_bundle_without_await(
    bundle: &CfgModule<LoweredCoreBlockPyFunctionWithoutAwait>,
) -> crate::PassShapeSummary {
    let blockpy = project_lowered_module_callable_defs(
        bundle,
        |lowered| -> &crate::basic_block::block_py::BlockPyCallableDef<CoreBlockPyExprWithoutAwait> { lowered },
    );
    summarize_blockpy_module(&blockpy)
}

fn summarize_core_blockpy_module_without_await(
    module: &block_py::BlockPyModule<CoreBlockPyExprWithoutAwait>,
) -> crate::PassShapeSummary {
    summarize_blockpy_module(module)
}

fn summarize_core_blockpy_module_without_await_or_yield(
    module: &block_py::BlockPyModule<CoreBlockPyExprWithoutAwaitOrYield>,
) -> crate::PassShapeSummary {
    summarize_blockpy_module(module)
}

fn summarize_core_blockpy_bundle_without_await_or_yield(
    bundle: &CfgModule<LoweredCoreBlockPyFunctionWithoutAwaitOrYield>,
) -> crate::PassShapeSummary {
    let blockpy = project_lowered_module_callable_defs(
        bundle,
        |lowered| -> &crate::basic_block::block_py::BlockPyCallableDef<
            CoreBlockPyExprWithoutAwaitOrYield,
        > { lowered },
    );
    summarize_blockpy_module(&blockpy)
}

pub(crate) fn summarize_tracked_pass_shape(
    result: &crate::LoweringResult,
    name: &str,
) -> Option<crate::PassShapeSummary> {
    if let Some(plan) = result.get_pass::<LoweredBlockPyModuleBundlePlan>(name) {
        return Some(summarize_semantic_blockpy_plan(plan));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<Expr>>(name) {
        return Some(summarize_blockpy_module(module));
    }
    if let Some(bundle) = result.get_pass::<CfgModule<LoweredBlockPyFunction>>(name) {
        return Some(summarize_semantic_blockpy_bundle(bundle));
    }
    if let Some(bundle) = result.get_pass::<CfgModule<LoweredCoreBlockPyFunction>>(name) {
        return Some(summarize_core_blockpy_bundle(bundle));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyExpr>>(name) {
        return Some(summarize_core_blockpy_module(module));
    }
    if let Some(bundle) = result.get_pass::<CfgModule<LoweredCoreBlockPyFunctionWithoutAwait>>(name)
    {
        return Some(summarize_core_blockpy_bundle_without_await(bundle));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyExprWithoutAwait>>(name) {
        return Some(summarize_core_blockpy_module_without_await(module));
    }
    if let Some(bundle) =
        result.get_pass::<CfgModule<LoweredCoreBlockPyFunctionWithoutAwaitOrYield>>(name)
    {
        return Some(summarize_core_blockpy_bundle_without_await_or_yield(bundle));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyExprWithoutAwaitOrYield>>(name)
    {
        return Some(summarize_blockpy_module(module));
    }
    None
}

fn render_semantic_blockpy_plan(plan: &LoweredBlockPyModuleBundlePlan) -> String {
    blockpy_module_to_string(&lowered_blockpy_module_bundle_plan_to_semantic_blockpy_module(plan))
}

fn render_semantic_blockpy_bundle(bundle: &CfgModule<LoweredBlockPyFunction>) -> String {
    let blockpy = project_lowered_module_callable_defs(
        bundle,
        |lowered| -> &crate::basic_block::block_py::BlockPyCallableDef<Expr> { lowered },
    );
    blockpy_module_to_string(&blockpy)
}

fn render_core_blockpy_bundle(bundle: &CfgModule<LoweredCoreBlockPyFunction>) -> String {
    let blockpy = project_lowered_module_callable_defs(
        bundle,
        |lowered| -> &crate::basic_block::block_py::BlockPyCallableDef<CoreBlockPyExpr> { lowered },
    );
    blockpy_module_to_string(&blockpy)
}

fn render_core_blockpy_module(module: &block_py::BlockPyModule<CoreBlockPyExpr>) -> String {
    blockpy_module_to_string(module)
}

fn render_core_blockpy_bundle_without_await(
    bundle: &CfgModule<LoweredCoreBlockPyFunctionWithoutAwait>,
) -> String {
    let blockpy = project_lowered_module_callable_defs(
        bundle,
        |lowered| -> &crate::basic_block::block_py::BlockPyCallableDef<CoreBlockPyExprWithoutAwait> { lowered },
    );
    blockpy_module_to_string(&blockpy)
}

fn render_core_blockpy_module_without_await(
    module: &block_py::BlockPyModule<CoreBlockPyExprWithoutAwait>,
) -> String {
    blockpy_module_to_string(module)
}

fn render_core_blockpy_module_without_await_or_yield(
    module: &block_py::BlockPyModule<CoreBlockPyExprWithoutAwaitOrYield>,
) -> String {
    blockpy_module_to_string(module)
}

fn render_core_blockpy_bundle_without_await_or_yield(
    bundle: &CfgModule<LoweredCoreBlockPyFunctionWithoutAwaitOrYield>,
) -> String {
    let blockpy = project_lowered_module_callable_defs(
        bundle,
        |lowered| -> &crate::basic_block::block_py::BlockPyCallableDef<
            CoreBlockPyExprWithoutAwaitOrYield,
        > { lowered },
    );
    blockpy_module_to_string(&blockpy)
}

fn render_bb_module(bundle: &bb_ir::BbModule) -> String {
    let mut out = String::new();
    if bundle
        .callable_defs
        .iter()
        .any(|function| function.names.bind_name == "_dp_module_init")
    {
        out.push_str("module_init: _dp_module_init\n\n");
    }
    for function in &bundle.callable_defs {
        out.push_str(&format!(
            "function {} [{}] entry={}\n",
            function.names.qualname,
            function.names.display_name,
            function.entry_label(),
        ));
        out.push_str(&format!("kind: {:?}\n", function.lowered_kind()));
        let param_names = function.params.names();
        if !param_names.is_empty() {
            out.push_str(&format!("params: {}\n", param_names.join(", ")));
        }
        for block in &function.blocks {
            let params = if block.meta.params.is_empty() {
                String::new()
            } else {
                format!("({})", block.meta.params.join(", "))
            };
            out.push_str(&format!("\n{}{}:\n", block.label, params));
            for stmt in &block.body {
                out.push_str("    ");
                out.push_str(&bb_ir::bb_stmt_text(stmt));
                out.push('\n');
            }
            out.push_str("    ");
            out.push_str(&render_bb_term(&block.term));
            out.push('\n');
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn render_bb_term(
    term: &crate::basic_block::block_py::BlockPyTerm<
        crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield,
    >,
) -> String {
    match term {
        crate::basic_block::block_py::BlockPyTerm::Jump(label) => format!("jump {label}"),
        crate::basic_block::block_py::BlockPyTerm::IfTerm(if_term) => format!(
            "if {} then {} else {}",
            bb_ir::bb_expr_text(&if_term.test),
            if_term.then_label,
            if_term.else_label
        ),
        crate::basic_block::block_py::BlockPyTerm::BranchTable(branch) => format!(
            "br_table index={} targets=[{}] default={}",
            bb_ir::bb_expr_text(&branch.index),
            branch
                .targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            branch.default_label
        ),
        crate::basic_block::block_py::BlockPyTerm::Raise(raise_stmt) => {
            bb_ir::bb_raise_text(raise_stmt)
        }
        crate::basic_block::block_py::BlockPyTerm::Return(value) => value
            .as_ref()
            .map(|value| format!("return {}", bb_ir::bb_expr_text(value)))
            .unwrap_or_else(|| "return".to_string()),
        crate::basic_block::block_py::BlockPyTerm::TryJump(_) => {
            panic!("TryJump is not allowed in BbTerm")
        }
    }
}

pub(crate) fn render_tracked_pass_text(
    result: &crate::LoweringResult,
    name: &str,
) -> Option<String> {
    if let Some(body) = result.get_pass::<ruff_python_ast::Suite>(name) {
        return Some(crate::ruff_ast_to_string(body));
    }
    if let Some(plan) = result.get_pass::<LoweredBlockPyModuleBundlePlan>(name) {
        return Some(render_semantic_blockpy_plan(plan));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<Expr>>(name) {
        return Some(blockpy_module_to_string(module));
    }
    if let Some(bundle) = result.get_pass::<CfgModule<LoweredBlockPyFunction>>(name) {
        return Some(render_semantic_blockpy_bundle(bundle));
    }
    if let Some(bundle) = result.get_pass::<CfgModule<LoweredCoreBlockPyFunction>>(name) {
        return Some(render_core_blockpy_bundle(bundle));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyExpr>>(name) {
        return Some(render_core_blockpy_module(module));
    }
    if let Some(bundle) = result.get_pass::<CfgModule<LoweredCoreBlockPyFunctionWithoutAwait>>(name)
    {
        return Some(render_core_blockpy_bundle_without_await(bundle));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyExprWithoutAwait>>(name) {
        return Some(render_core_blockpy_module_without_await(module));
    }
    if let Some(bundle) =
        result.get_pass::<CfgModule<LoweredCoreBlockPyFunctionWithoutAwaitOrYield>>(name)
    {
        return Some(render_core_blockpy_bundle_without_await_or_yield(bundle));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyExprWithoutAwaitOrYield>>(name)
    {
        return Some(render_core_blockpy_module_without_await_or_yield(module));
    }
    if let Some(bundle) = result.get_pass::<bb_ir::BbModule>(name) {
        return Some(render_bb_module(bundle));
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::basic_block::bb_ir::{BbBlock, BbFunction, BbModule};
    use crate::basic_block::block_py::BlockPyFunctionKind;
    use crate::basic_block::block_py::{
        BlockPyModule, BlockPyStmt, BlockPyTerm, CoreBlockPyExprWithoutAwaitOrYield,
    };
    use crate::basic_block::block_py::{ClosureInit, ClosureSlot};
    use crate::LoweringResult;
    use crate::{
        py_expr, transform_str_to_bb_ir_with_options, transform_str_to_ruff_with_options, Options,
    };
    use ruff_python_ast::Expr;
    struct TrackedLowering {
        result: LoweringResult,
        blockpy_module: BlockPyModule<Expr>,
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

        fn blockpy_module(&self) -> BlockPyModule<Expr> {
            self.blockpy_module.clone()
        }

        fn blockpy_text(&self) -> String {
            crate::basic_block::blockpy_module_to_string(&self.blockpy_module())
        }

        fn semantic_blockpy_text(&self) -> String {
            self.pass_text("semantic_blockpy")
        }

        fn core_blockpy_text(&self) -> String {
            self.pass_text("core_blockpy")
        }

        fn pass_text(&self, name: &str) -> String {
            crate::basic_block::render_tracked_pass_text(&self.result, name)
                .unwrap_or_else(|| panic!("expected renderable pass {name}"))
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
            .find(|func| func.names.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing lowered function {bind_name}; got {:?}", bb_module));
        if direct.closure_layout().is_some() {
            return direct;
        }
        bb_module
            .callable_defs
            .iter()
            .find(|func| func.names.bind_name == format!("{bind_name}_resume"))
            .unwrap_or(direct)
    }

    fn slot_by_name<'a>(slots: &'a [ClosureSlot], logical_name: &str) -> &'a ClosureSlot {
        slots
            .iter()
            .find(|slot| slot.logical_name == logical_name)
            .unwrap_or_else(|| panic!("missing closure slot {logical_name}; got {slots:?}"))
    }

    fn expr_text(expr: &CoreBlockPyExprWithoutAwaitOrYield) -> String {
        crate::basic_block::bb_ir::bb_expr_text(expr)
    }

    fn callable_def_by_name<'a>(
        blockpy_module: &'a crate::basic_block::block_py::BlockPyModule,
        bind_name: &str,
    ) -> &'a crate::basic_block::block_py::BlockPyCallableDef {
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
            BlockPyStmt::Assign(assign) => expr_text(&assign.value).contains(needle),
            BlockPyStmt::Expr(expr) => expr_text(expr).contains(needle),
            BlockPyStmt::Delete(delete) => delete.target.id.as_str().contains(needle),
            BlockPyStmt::If(_) => false,
        }) || match &block.term {
            BlockPyTerm::IfTerm(if_term) => expr_text(&if_term.test).contains(needle),
            BlockPyTerm::BranchTable(branch) => expr_text(&branch.index).contains(needle),
            BlockPyTerm::Raise(raise_stmt) => raise_stmt
                .exc
                .as_ref()
                .is_some_and(|value| expr_text(value).contains(needle)),
            BlockPyTerm::Return(value) => value
                .as_ref()
                .is_some_and(|ret| expr_text(ret).contains(needle)),
            BlockPyTerm::TryJump(_) => false,
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

        let core_blockpy = lowered.core_blockpy_text();
        assert!(core_blockpy.contains("\"value=\""), "{core_blockpy}");
        assert!(
            core_blockpy.contains("__dp_format(__dp_repr(value))"),
            "{core_blockpy}"
        );

        let core_blockpy_with_explicit_eval_order =
            lowered.pass_text("core_blockpy_with_explicit_eval_order");
        assert!(
            core_blockpy_with_explicit_eval_order.contains("_dp_eval_"),
            "{core_blockpy_with_explicit_eval_order}"
        );
        assert!(
            core_blockpy_with_explicit_eval_order.contains("__dp_repr(value)"),
            "{core_blockpy_with_explicit_eval_order}"
        );
        assert!(
            core_blockpy_with_explicit_eval_order.contains("__dp_format(_dp_eval_"),
            "{core_blockpy_with_explicit_eval_order}"
        );

        let fmt = lowered.bb_function("fmt");
        assert!(
            fmt.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_format(_dp_eval_")),
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

        let core_blockpy = lowered.core_blockpy_text();
        assert!(
            core_blockpy.contains("__dp_templatelib_Interpolation(value, \"value\", None, \"\")"),
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
        assert_eq!(foo.entry_label(), "start", "{:?}", foo.entry_label());
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
    fn rewritten_ruff_ast_can_keep_assert_while_stmt_sequence_still_lowers_it() {
        let source = r#"
def check():
    assert cond, msg
"#;

        let lowered = TrackedLowering::new(source);
        let check = lowered.bb_function("check");
        assert!(
            check.blocks.iter().any(|block| matches!(
                block.term,
                crate::basic_block::block_py::BlockPyTerm::IfTerm(_)
            )),
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
            .filter(|block| {
                matches!(
                    block.term,
                    crate::basic_block::block_py::BlockPyTerm::IfTerm(_)
                )
            })
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
            choose.blocks.iter().any(|block| matches!(
                block.term,
                crate::basic_block::block_py::BlockPyTerm::IfTerm(_)
            )),
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
            choose.blocks.iter().any(|block| matches!(
                block.term,
                crate::basic_block::block_py::BlockPyTerm::IfTerm(_)
            )),
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
            blockpy_rendered.contains("function choose.<locals>.<genexpr>("),
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
        let semantic_blockpy_rendered = lowered.semantic_blockpy_text();
        assert!(
            semantic_blockpy_rendered.contains("await Once()"),
            "{semantic_blockpy_rendered}"
        );
        assert!(
            !semantic_blockpy_rendered.contains("__dp_await_iter"),
            "{semantic_blockpy_rendered}"
        );

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
        let check = lowered.bb_function("check");
        assert!(
            check.blocks.iter().any(|block| matches!(
                block.term,
                crate::basic_block::block_py::BlockPyTerm::IfTerm(_)
            )),
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
            check.blocks.iter().any(|block| {
                matches!(
                    block.term,
                    crate::basic_block::block_py::BlockPyTerm::Raise(_)
                )
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
    fn ast_to_ast_can_lower_import_from_while_later_passes_still_lower_it() {
        let source = r#"
from pkg.mod import name as alias
"#;

        let lowered = TrackedLowering::new(source);

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
                [BlockPyStmt::Assign(assign)] => expr_text(&assign.value).contains("__dp_iadd"),
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
            crate::basic_block::ast_to_ast::body::suite_mut(&mut module.body),
        );

        let rendered = crate::ruff_ast_to_string(crate::basic_block::ast_to_ast::body::suite_ref(
            &module.body,
        ));
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
                .any(|block| matches!(block.term, BlockPyTerm::Return(None))),
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
                .get_pass::<crate::basic_block::block_py::BlockPyModule<Expr>>("semantic_blockpy")
                .cloned()
                .expect("expected lowered semantic BlockPy module");
            let blockpy_rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
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

            let prepared = crate::basic_block::lower_try_jump_exception_flow(&bb_module)
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
        let gen = function_by_name(&bb_module, "gen");
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
        let init_fn = function_by_name(&bb_module, "_dp_module_init");
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

use crate::basic_block;
use crate::basic_block::ast_to_ast::ast_rewrite::rewrite_with_pass;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_class_def;
use crate::basic_block::ast_to_ast::rewrite_stmt::function_def::rewrite_ast_to_lowered_blockpy_module_plan;
use crate::basic_block::ast_to_ast::scope::{analyze_module_scope, BindingKind};
use crate::basic_block::ast_to_ast::simplify::lower_surrogate_string_literals;
use crate::basic_block::ast_to_ast::{
    ast_rewrite::ExprRewritePass,
    ast_rewrite::LoweredExpr,
    body::{body_from_suite, suite_mut, take_suite, Suite},
    rewrite_expr::lower_scoped_helper_expr,
    rewrite_future_annotations, rewrite_names, rewrite_stmt,
};
use crate::basic_block::bb_ir::BbModule;
use crate::basic_block::block_py::{BlockPyModule, CfgModule};
use crate::basic_block::blockpy_to_bb::LoweredCoreBlockPyFunctionWithoutAwaitOrYield;
use crate::basic_block::blockpy_to_bb::{
    LoweredCoreBlockPyFunction, LoweredCoreBlockPyFunctionWithoutAwait,
};
use crate::basic_block::ruff_to_blockpy::LoweredBlockPyFunction;
use crate::PassTracker;
use ruff_python_ast::{self as ast, Expr, Stmt};

pub fn rewrite_module(context: &Context, module: Suite) -> (PassTracker, BbModule) {
    let mut pass_tracker = PassTracker::new();
    let bb_module = rewrite_module_with_tracker(context, module, &mut pass_tracker);
    (pass_tracker, bb_module)
}

pub(crate) fn rewrite_module_with_tracker(
    context: &Context,
    module: Suite,
    pass_tracker: &mut PassTracker,
) -> BbModule {
    let (_module, semantic_blockpy): (Suite, BlockPyModule<Expr>) =
        pass_tracker.run_pass("ast-to-ast", || {
            let mut module = body_from_suite(module);

            // The transform now has a single lowering strategy: basic-block form.
            lower_surrogate_string_literals(context, suite_mut(&mut module));

            rewrite_future_annotations::rewrite(context, suite_mut(&mut module));

            // Rewrite names like "__foo" in class bodies to "_<class_name>__foo"
            rewrite_class_def::private::rewrite_private_names(context, suite_mut(&mut module));

            // Replace annotated assignments ("x: int = 1") with regular assignments,
            // and either drop the annotations (in functions) or generate an
            // __annotate__ function (in modules and classes)
            rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(
                context,
                suite_mut(&mut module),
            );

            wrap_module_init(suite_mut(&mut module));

            // Lower helper-scoped expressions that synthesize nested defs for Python
            // scoping semantics before the more direct BlockPy expr lowering boundary.
            rewrite_with_pass(
                context,
                None,
                Some(&ScopedHelperExprPass),
                suite_mut(&mut module),
            );

            // Lower multi-target assignment / delete shapes to the single-name forms
            // that the later BlockPy lowering expects.
            rewrite_with_pass(
                context,
                Some(&basic_block::SingleNamedAssignmentPass),
                None,
                suite_mut(&mut module),
            );

            let scope = analyze_module_scope(suite_mut(&mut module));

            // Replace global / nonlocal and class-body scoping with explicit loads/stores.
            //  - globals: __dp__.load/store_global(globals(), name)
            //  - nonlocal: create a cell in the outermost scope, and access with __dp__.load/store_cell(cell, value)
            //  - class-body: class_body_load_cell/global(_dp_class_ns, name, cell / globals()) captures "try class, then outer"
            rewrite_names::rewrite_explicit_bindings(
                context,
                scope.clone(),
                suite_mut(&mut module),
            );

            rewrite_class_def::class_body::rewrite_class_body_scopes(
                context,
                scope,
                suite_mut(&mut module),
            );
            rewrite_ast_to_lowered_blockpy_module_plan(context, take_suite(&mut module))
        });
    let semantic_blockpy: BlockPyModule<Expr> =
        pass_tracker.run_pass("semantic_blockpy", || semantic_blockpy.clone());

    let lowered_blockpy_module: CfgModule<LoweredBlockPyFunction> = pass_tracker
        .run_pass("blockpy", || {
            basic_block::lower_blockpy_module_plan_to_bundle(context, semantic_blockpy)
        });
    let core_blockpy: CfgModule<LoweredCoreBlockPyFunction> = pass_tracker
        .run_pass("core_blockpy", || {
            basic_block::simplify_lowered_blockpy_module_bundle_exprs(&lowered_blockpy_module)
        });
    let core_blockpy_with_explicit_eval_order: CfgModule<LoweredCoreBlockPyFunction> = pass_tracker
        .run_pass("core_blockpy_with_explicit_eval_order", || {
            basic_block::make_eval_order_explicit_in_lowered_core_blockpy_module_bundle(
                core_blockpy,
            )
        });
    let core_blockpy_without_await: CfgModule<LoweredCoreBlockPyFunctionWithoutAwait> =
        pass_tracker.run_pass("core_blockpy_without_await", || {
            basic_block::lower_awaits_in_lowered_core_blockpy_module_bundle(
                core_blockpy_with_explicit_eval_order,
            )
        });
    let core_blockpy_without_await_or_yield: CfgModule<
        LoweredCoreBlockPyFunctionWithoutAwaitOrYield,
    > = pass_tracker.run_pass("core_blockpy_without_await_or_yield", || {
        basic_block::lower_yield_in_lowered_core_blockpy_module_bundle(core_blockpy_without_await)
    });
    let bb_module: BbModule = pass_tracker.run_pass("bb", || {
        basic_block::lower_core_blockpy_module_bundle_to_bb_module(
            &core_blockpy_without_await_or_yield,
        )
    });
    bb_module
}

fn is_module_docstring(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    )
}

fn is_future_import(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::ImportFrom(ast::StmtImportFrom { module, .. })
            if module.as_ref().map(|name| name.id.as_str()) == Some("__future__")
    )
}

pub(crate) fn wrap_module_init(module: &mut Suite) {
    let mut global_names = {
        let scope = analyze_module_scope(module);
        let bindings = scope.scope_bindings();
        bindings
            .iter()
            .filter_map(|(name, kind)| {
                if *kind == BindingKind::Local {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    global_names.sort();

    let mut prelude = Vec::new();
    let mut init_body = Vec::new();
    let mut seen_non_prelude = false;
    let mut docstring_seen = false;

    for stmt in std::mem::take(module) {
        let stmt_ref = stmt.as_ref();
        if !seen_non_prelude {
            if !docstring_seen && is_module_docstring(stmt_ref) {
                prelude.push(stmt);
                docstring_seen = true;
                continue;
            }
            docstring_seen = true;
            if is_future_import(stmt_ref) {
                prelude.push(stmt);
                continue;
            }
            seen_non_prelude = true;
        }
        init_body.push(*stmt);
    }

    if init_body.is_empty() {
        init_body.push(crate::py_stmt!("pass"));
    }

    let global_stmts = global_names
        .into_iter()
        .map(|name| crate::py_stmt!("global {name:id}", name = name.as_str()))
        .collect::<Vec<_>>();

    let module_init: ast::StmtFunctionDef = crate::py_stmt_typed!(
        r#"
def _dp_module_init():
    {global_stmts:stmt}
    {init_body:stmt}
"#,
        global_stmts = global_stmts,
        init_body = init_body,
    );

    prelude.push(Box::new(Stmt::FunctionDef(module_init)));
    *module = prelude;
}

pub struct ScopedHelperExprPass;

impl ExprRewritePass for ScopedHelperExprPass {
    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        match expr {
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => lower_scoped_helper_expr(context, expr),
            other => LoweredExpr::unmodified(other),
        }
    }
}

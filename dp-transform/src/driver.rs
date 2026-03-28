use crate::block_py::BlockPyModule;
use crate::passes::ast_to_ast::ast_rewrite::rewrite_with_pass;
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::rewrite_class_def;
use crate::passes::ast_to_ast::rewrite_expr::ScopedHelperExprPass;
use crate::passes::ast_to_ast::{
    body::{split_docstring, Suite},
    rewrite_future_annotations, rewrite_stmt,
    semantic::SemanticAstState,
};
use crate::passes::blockpy_expr_simplify::simplify_blockpy_callable_def_exprs;
use crate::passes::core_await_lower::lower_awaits_in_core_blockpy_module;
use crate::passes::ruff_to_blockpy::rewrite_ast_to_lowered_blockpy_module_plan_with_module;
use crate::passes::{
    self, CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield,
    ResolvedStorageBlockPyPass, RuffBlockPyPass,
};
use crate::PassTracker;
use ruff_python_ast::{self as ast, Stmt};

#[derive(Clone)]
struct AstToAstPassResult {
    module: Suite,
    semantic_state: SemanticAstState,
}

impl crate::TrackedPassText for AstToAstPassResult {
    fn render_tracked_pass_text(&self) -> String {
        crate::ruff_ast_to_string(&self.module)
    }
}

fn rewrite_ast_to_ast_module((context, mut module): (&Context, Suite)) -> AstToAstPassResult {
    // The transform now has a single lowering strategy: basic-block form.
    let future_imports = rewrite_future_annotations::rewrite(context, &mut module);

    // Rewrite names like "__foo" in class bodies to "_<class_name>__foo"
    rewrite_class_def::private::rewrite_private_names(context, &mut module);

    // Replace annotated assignments ("x: int = 1") with regular assignments,
    // and either drop the annotations (in functions) or generate an
    // __annotate__ function (in modules and classes)
    rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(context, &mut module);

    // Lower helper-scoped expressions that synthesize nested defs for Python
    // scoping semantics before the more direct BlockPy expr lowering boundary.
    rewrite_with_pass(context, None, Some(&ScopedHelperExprPass), &mut module);

    let mut semantic_state = SemanticAstState::from_ruff(&mut module);

    /*

    Wrap the module body in a synthesized `_dp_module_init` function.  It is assigned the same scope table as the
    module body so everythign remains e.g globals instead of locals.  This (combined with the similar but much
    more complicated) class rewrite below, lets us only deal with functions throughout the rest of the pipeline.
     */
    wrap_module_init(&mut semantic_state, &mut module);

    rewrite_class_def::class_body::rewrite_class_body_scopes(
        context,
        &mut semantic_state,
        &mut module,
    );
    let invalid_future_stmts =
        rewrite_future_annotations::invalid_future_feature_syntax_error_stmts(&future_imports);
    if !invalid_future_stmts.is_empty() {
        let [Stmt::FunctionDef(module_init)] = &mut module.as_mut_slice() else {
            panic!("expected wrapped module root before inserting invalid future error stubs");
        };
        module_init.body.splice(0..0, invalid_future_stmts);
    }

    AstToAstPassResult {
        module,
        semantic_state,
    }
}

fn lower_semantic_blockpy(
    (context, module, semantic_state): (&Context, &mut Suite, &SemanticAstState),
) -> BlockPyModule<RuffBlockPyPass> {
    rewrite_ast_to_lowered_blockpy_module_plan_with_module(context, module, semantic_state)
}

fn lower_core_blockpy_with_await_and_yield(
    module: BlockPyModule<RuffBlockPyPass>,
) -> BlockPyModule<CoreBlockPyPassWithAwaitAndYield> {
    module.map_callable_defs(simplify_blockpy_callable_def_exprs)
}

fn lower_bb_prepared(
    name_binding: &BlockPyModule<ResolvedStorageBlockPyPass>,
) -> BlockPyModule<ResolvedStorageBlockPyPass> {
    passes::lower_try_jump_exception_flow(name_binding)
        .expect("bb_prepared pass should succeed for valid BB lowering")
}

fn lower_bb_codegen(
    (bb_prepared, source): (&BlockPyModule<ResolvedStorageBlockPyPass>, &str),
) -> BlockPyModule<ResolvedStorageBlockPyPass> {
    passes::normalize_bb_module_strings(bb_prepared, source)
}

fn lower_bb_trace(
    (bb_codegen, config): (
        BlockPyModule<ResolvedStorageBlockPyPass>,
        &passes::TraceConfig,
    ),
) -> BlockPyModule<ResolvedStorageBlockPyPass> {
    let mut traced = bb_codegen;
    passes::instrument_bb_module_for_trace(&mut traced, config);
    traced
}

fn lower_bb_validate(
    bb_traced: BlockPyModule<ResolvedStorageBlockPyPass>,
) -> BlockPyModule<ResolvedStorageBlockPyPass> {
    passes::validate_prepared_bb_module(&bb_traced)
        .expect("bb_validate pass should succeed for valid prepared BB lowering");
    bb_traced
}

pub(crate) fn rewrite_module_with_tracker(
    context: &Context,
    module: &mut Suite,
    pass_tracker: &mut PassTracker,
) -> BlockPyModule<ResolvedStorageBlockPyPass> {
    let AstToAstPassResult {
        module: ast_module,
        semantic_state,
    } = pass_tracker.run_pass(
        "ast-to-ast",
        (context, std::mem::take(module)),
        rewrite_ast_to_ast_module,
    );
    *module = ast_module;

    /*

       Convert all flow control into a block-and-jump structure.  For example,

       ```
       x = 0
       while (y := x + 1) < 5:
           print(x)
           x += 1
       ```

       would turn into something like:

       ```
       block start:
           y = x + 1
           if y < 5:
               jump body
           else:
               jump end
       block body:
           print(x)
           x += 1
           jump start
       block end:
           return None
       ```

       This removes while/with/for from the AST, as well as expressions that
       interact with the block structure like walrus and those that short circuit like bool ops.

       "def" is replaced by a call to
       `__dp_make_function(function_id, kind, closure, param_defaults, module_globals, annotate_fn)`.

       try/except are replaced by an exception handling block, and each block in the `try` has exc_edge
       set to that handler.  except block has it's own exc_edge to ensure exceptions in except
       still jump to finally.
    */

    let semantic_blockpy: BlockPyModule<RuffBlockPyPass> = pass_tracker.run_pass(
        "semantic_blockpy",
        (context, module, &semantic_state),
        lower_semantic_blockpy,
    );

    /*
    Simplify expressions:
      - replace operators with intrinsic calls, so that something like:
            `a[1] + b[2]`

        becomes:
            ```
            __dp_add(__dp_getitem(a, 1), __dp_getitem(b, 2))
            ```
    */
    let core_blockpy: BlockPyModule<CoreBlockPyPassWithAwaitAndYield> = pass_tracker.run_pass(
        "core_blockpy_with_await_and_yield",
        semantic_blockpy,
        lower_core_blockpy_with_await_and_yield,
    );

    /*
      A very simple pass to rewrite `await foo` into  `yield from __dp_await_iter(foo)`
    */
    let core_blockpy_without_await: BlockPyModule<CoreBlockPyPassWithYield> = pass_tracker
        .run_pass(
            "core_blockpy_with_yield",
            core_blockpy,
            lower_awaits_in_core_blockpy_module,
        );

    /*
     Convert generators into a state machine, driven by an internal `resume(send, throw)` function.

     `resume` carries state in closure cells, with blocks split at yield/resume points.

    */
    let core_blockpy_without_await_or_yield: BlockPyModule<CoreBlockPyPass> = pass_tracker
        .run_pass(
            "core_blockpy",
            core_blockpy_without_await,
            passes::lower_yield_in_lowered_core_blockpy_module_bundle,
        );
    let name_binding: BlockPyModule<ResolvedStorageBlockPyPass> = pass_tracker.run_pass(
        "name_binding",
        core_blockpy_without_await_or_yield,
        passes::lower_name_binding_in_core_blockpy_module,
    );
    let trace_config = passes::parse_trace_env();
    let bb_prepared: BlockPyModule<ResolvedStorageBlockPyPass> =
        pass_tracker.run_pass("bb_prepared", &name_binding, lower_bb_prepared);
    let bb_codegen: BlockPyModule<ResolvedStorageBlockPyPass> = pass_tracker.run_pass(
        "bb_codegen",
        (&bb_prepared, context.source.as_str()),
        lower_bb_codegen,
    );
    let bb_traced: BlockPyModule<ResolvedStorageBlockPyPass> = if let Some(config) = trace_config {
        pass_tracker.run_pass("bb_trace", (bb_codegen, &config), lower_bb_trace)
    } else {
        bb_codegen
    };
    pass_tracker.run_pass("bb_validate", bb_traced, lower_bb_validate)
}

pub(crate) fn wrap_module_init(semantic_state: &mut SemanticAstState, module: &mut Suite) {
    let (docstring, mut init_body) = split_docstring(module);
    if let Some(docstring) = docstring {
        init_body.insert(
            0,
            crate::py_stmt!("__doc__ = {value:literal}", value = docstring),
        );
    }

    if init_body.is_empty() {
        init_body.push(crate::py_stmt!("pass"));
    }

    let module_init: ast::StmtFunctionDef = crate::py_stmt_typed!(
        r#"
def _dp_module_init():
    {init_body:stmt}
"#,
        init_body = init_body,
    );
    semantic_state.synthesize_module_init_scope(&module_init);

    *module = vec![Stmt::FunctionDef(module_init)];
}

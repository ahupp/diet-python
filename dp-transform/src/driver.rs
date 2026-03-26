use crate::block_py::BlockPyModule;
use crate::passes::ast_to_ast::ast_rewrite::rewrite_with_pass;
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::rewrite_class_def;
use crate::passes::ast_to_ast::rewrite_expr::ScopedHelperExprPass;
use crate::passes::ast_to_ast::simplify::lower_surrogate_string_literals;
use crate::passes::ast_to_ast::{
    body::{split_docstring, suite_mut, Suite},
    rewrite_future_annotations, rewrite_stmt,
    semantic::SemanticAstState,
};
use crate::passes::blockpy_expr_simplify::simplify_blockpy_callable_def_exprs;
use crate::passes::core_await_lower::lower_awaits_in_core_blockpy_module;
use crate::passes::ruff_to_blockpy::rewrite_ast_to_lowered_blockpy_module_plan_with_module;
use crate::passes::{
    self, BbBlockPyPass, CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield,
    CoreBlockPyPassWithYield, LocatedCoreBlockPyPass, PreparedBbBlockPyPass, RuffBlockPyPass,
};
use crate::PassTracker;
use ruff_python_ast::{self as ast, Stmt};

pub(crate) fn rewrite_module_with_tracker(
    context: &Context,
    module: &mut Suite,
    pass_tracker: &mut PassTracker,
) -> BlockPyModule<PreparedBbBlockPyPass> {
    let mut semantic_state: Option<SemanticAstState> = None;
    let ast_module = pass_tracker.run_renderable_pass("ast-to-ast", || {
        let mut module = std::mem::take(module);

        // The transform now has a single lowering strategy: basic-block form.
        lower_surrogate_string_literals(context, suite_mut(&mut module));

        let future_imports = rewrite_future_annotations::rewrite(context, suite_mut(&mut module));

        // Rewrite names like "__foo" in class bodies to "_<class_name>__foo"
        rewrite_class_def::private::rewrite_private_names(context, suite_mut(&mut module));

        // Replace annotated assignments ("x: int = 1") with regular assignments,
        // and either drop the annotations (in functions) or generate an
        // __annotate__ function (in modules and classes)
        rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(
            context,
            suite_mut(&mut module),
        );

        // Lower helper-scoped expressions that synthesize nested defs for Python
        // scoping semantics before the more direct BlockPy expr lowering boundary.
        rewrite_with_pass(
            context,
            None,
            Some(&ScopedHelperExprPass),
            suite_mut(&mut module),
        );

        let mut rewrite_semantic_state = SemanticAstState::from_ruff(suite_mut(&mut module));
        wrap_module_init(&mut rewrite_semantic_state, suite_mut(&mut module));

        rewrite_class_def::class_body::rewrite_class_body_scopes(
            context,
            &mut rewrite_semantic_state,
            suite_mut(&mut module),
        );
        let invalid_future_stmts =
            rewrite_future_annotations::invalid_future_feature_syntax_error_stmts(&future_imports);
        if !invalid_future_stmts.is_empty() {
            let [Stmt::FunctionDef(module_init)] = suite_mut(&mut module).as_mut_slice() else {
                panic!("expected wrapped module root before inserting invalid future error stubs");
            };
            suite_mut(&mut module_init.body).splice(0..0, invalid_future_stmts);
        }
        semantic_state = Some(rewrite_semantic_state);
        module
    });
    *module = ast_module;
    let semantic_state = semantic_state.expect("semantic AST state should be available");

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

    let semantic_blockpy: BlockPyModule<RuffBlockPyPass> = pass_tracker
        .run_renderable_pass("semantic_blockpy", || {
            rewrite_ast_to_lowered_blockpy_module_plan_with_module(context, module, &semantic_state)
        });

    /*
    Simplify expressions:
      - replace operators with intrinsic calls, so that something like:
            `a[1] + b[2]`

        becomes:
            ```
            __dp_add(__dp_getitem(a, 1), __dp_getitem(b, 2))
            ```
    */
    let core_blockpy: BlockPyModule<CoreBlockPyPassWithAwaitAndYield> = pass_tracker
        .run_renderable_pass("core_blockpy_with_await_and_yield", || {
            semantic_blockpy.map_callable_defs(simplify_blockpy_callable_def_exprs)
        });

    /*
      Rewrite `await foo` to  `yield from __dp_await_iter(foo)`
    */
    let core_blockpy_without_await: BlockPyModule<CoreBlockPyPassWithYield> = pass_tracker
        .run_renderable_pass("core_blockpy_with_yield", || {
            lower_awaits_in_core_blockpy_module(core_blockpy)
        });

    /*
     Convert generators into a state machine, driven by an internal `resume(send, throw)` function.

     `resume` carries state in closure cells, with blocks split at yield/resume points.

    */
    let core_blockpy_without_await_or_yield: BlockPyModule<CoreBlockPyPass> = pass_tracker
        .run_renderable_pass("core_blockpy", || {
            passes::lower_yield_in_lowered_core_blockpy_module_bundle(core_blockpy_without_await)
        });
    let name_binding: BlockPyModule<LocatedCoreBlockPyPass> = pass_tracker
        .run_renderable_pass("name_binding", || {
            passes::lower_name_binding_in_core_blockpy_module(core_blockpy_without_await_or_yield)
        });
    let bb_module: BlockPyModule<BbBlockPyPass> = pass_tracker
        .run_renderable_pass("bb_blockpy", || {
            passes::lower_core_blockpy_module_bundle_to_bb_module(name_binding)
        });
    let bb_prepared: BlockPyModule<PreparedBbBlockPyPass> =
        pass_tracker.run_renderable_pass("bb_prepared", || {
            passes::lower_try_jump_exception_flow(&bb_module)
                .expect("bb_prepared pass should succeed for valid BB lowering")
        });
    let bb_traced: BlockPyModule<PreparedBbBlockPyPass> =
        pass_tracker.run_renderable_pass("bb_trace", || {
            let mut traced = bb_prepared.clone();
            if let Some(config) = passes::parse_trace_env() {
                passes::instrument_bb_module_for_trace(&mut traced, &config);
            }
            traced
        });
    pass_tracker.run_renderable_pass("bb_codegen", || {
        passes::normalize_bb_module_strings(&bb_traced)
    })
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

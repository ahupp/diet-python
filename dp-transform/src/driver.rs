use crate::block_py::BlockPyModule;
use crate::passes::ast_to_ast::ast_rewrite::rewrite_with_pass;
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::rewrite_class_def;
use crate::passes::ast_to_ast::rewrite_expr::ScopedHelperExprPass;
use crate::passes::ast_to_ast::scope::{analyze_module_scope, BindingKind};
use crate::passes::ast_to_ast::simplify::lower_surrogate_string_literals;
use crate::passes::ast_to_ast::{
    body::{body_from_suite, suite_mut, take_suite, Suite},
    rewrite_future_annotations, rewrite_names, rewrite_stmt,
    semantic::{debug_assert_matches_scope_tree, SemanticAstState},
};
use crate::passes::blockpy_expr_simplify::simplify_blockpy_callable_def_exprs;
use crate::passes::core_await_lower::lower_awaits_in_core_blockpy_module;
use crate::passes::ruff_to_blockpy::rewrite_ast_to_lowered_blockpy_module_plan_with_module;
use crate::passes::{
    self, BbBlockPyPass, CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield,
    CoreBlockPyPassWithYield, PreparedBbBlockPyPass, RuffBlockPyPass,
};
use crate::PassTracker;
use ruff_python_ast::{self as ast, Expr, Stmt};

pub(crate) fn rewrite_module_with_tracker(
    context: &Context,
    module: &mut Suite,
    pass_tracker: &mut PassTracker,
) -> BlockPyModule<PreparedBbBlockPyPass> {
    let mut semantic_state: Option<SemanticAstState> = None;
    let mut function_identity_state: Option<SemanticAstState> = None;
    let mut ast_module = pass_tracker.run_renderable_pass("ast-to-ast", || {
        let mut module = body_from_suite(std::mem::take(module));

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
            Some(&passes::SingleNamedAssignmentPass),
            None,
            suite_mut(&mut module),
        );

        let expected_module_scope = analyze_module_scope(suite_mut(&mut module));
        let mut rewrite_semantic_state = SemanticAstState::from_ruff(
            suite_mut(&mut module),
            Some(expected_module_scope.clone()),
        );
        let mut current_semantic_state =
            SemanticAstState::from_scope_tree(suite_mut(&mut module), expected_module_scope);

        // Replace global / nonlocal and class-body scoping with explicit loads/stores.
        //  - globals: __dp__.load/store_global(globals(), name)
        //  - nonlocal: create a cell in the outermost scope, and access with __dp__.load/store_cell(cell, value)
        //  - class-body: class_body_load_cell/global(_dp_class_ns, name, cell / globals()) captures "try class, then outer"
        rewrite_names::rewrite_explicit_bindings(
            context,
            &rewrite_semantic_state,
            suite_mut(&mut module),
        );

        rewrite_class_def::class_body::rewrite_class_body_scopes(
            context,
            &mut current_semantic_state,
            suite_mut(&mut module),
        );
        current_semantic_state.mirror_function_scope_overrides_to(
            &mut rewrite_semantic_state,
            suite_mut(&mut module),
        );
        if cfg!(debug_assertions) {
            debug_assert_matches_scope_tree(suite_mut(&mut module), &rewrite_semantic_state);
        }
        suite_mut(&mut module).splice(
            0..0,
            rewrite_future_annotations::invalid_future_feature_syntax_error_stmts(&future_imports),
        );
        semantic_state = Some(rewrite_semantic_state.clone());
        function_identity_state = Some(rewrite_semantic_state);
        module
    });
    *module = take_suite(&mut ast_module);
    let semantic_state = semantic_state.expect("semantic AST state should be available");
    let function_identity_state =
        function_identity_state.expect("function identity semantic AST state should be available");

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
       `__dp_make_function(function_id, closure, param_defaults, module_globals, annotate_fn)`.

       try/except are replaced by an exception handling block, and each block in the `try` has exc_edge
       set to that handler.  except block has it's own exc_edge to ensure exceptions in except
       still jump to finally.
    */

    let semantic_blockpy: BlockPyModule<RuffBlockPyPass> =
        pass_tracker.run_renderable_pass("semantic_blockpy", || {
            rewrite_ast_to_lowered_blockpy_module_plan_with_module(
                context,
                module,
                &semantic_state,
                &function_identity_state,
            )
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
        .run_renderable_pass("core_blockpy", || {
            semantic_blockpy.map_callable_defs(simplify_blockpy_callable_def_exprs)
        });

    /*
      Rewrite `await foo` to  `yield from __dp_await_iter(foo)`
    */
    let core_blockpy_without_await: BlockPyModule<CoreBlockPyPassWithYield> = pass_tracker
        .run_renderable_pass("core_blockpy_without_await", || {
            lower_awaits_in_core_blockpy_module(core_blockpy)
        });

    /*
     Convert generators into a state machine, driven by an internal `resume(send, throw)` function.

     `resume` carries state in closure cells, with blocks split at yield/resume points.

    */
    let core_blockpy_without_await_or_yield: BlockPyModule<CoreBlockPyPass> = pass_tracker
        .run_renderable_pass("core_blockpy_without_await_or_yield", || {
            passes::lower_yield_in_lowered_core_blockpy_module_bundle(core_blockpy_without_await)
        });
    let bb_module: BlockPyModule<BbBlockPyPass> = pass_tracker.run_renderable_pass("bb", || {
        passes::lower_core_blockpy_module_bundle_to_bb_module(core_blockpy_without_await_or_yield)
    });
    let bb_prepared: BlockPyModule<PreparedBbBlockPyPass> =
        pass_tracker.run_renderable_pass("bb_prepared", || {
            passes::lower_try_jump_exception_flow(&bb_module)
                .expect("bb_prepared pass should succeed for valid BB lowering")
        });
    pass_tracker.run_renderable_pass("bb_codegen", || {
        passes::normalize_bb_module_for_codegen(&bb_prepared)
    })
}

fn is_module_docstring(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
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
        if !seen_non_prelude {
            if !docstring_seen && is_module_docstring(&stmt) {
                prelude.push(stmt);
                docstring_seen = true;
                continue;
            }
            docstring_seen = true;
            seen_non_prelude = true;
        }
        init_body.push(stmt);
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

    prelude.push(Stmt::FunctionDef(module_init));
    *module = prelude;
}

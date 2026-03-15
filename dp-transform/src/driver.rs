use crate::basic_block;
use crate::basic_block::ast_to_ast::ast_rewrite::rewrite_with_pass;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_class_def;
use crate::basic_block::ast_to_ast::rewrite_stmt::function_def::rewrite_ast_to_lowered_blockpy_module;
use crate::basic_block::ast_to_ast::scope::{analyze_module_scope, BindingKind};
use crate::basic_block::ast_to_ast::simplify::lower_surrogate_string_literals;
use crate::basic_block::ast_to_ast::{
    ast_rewrite::ExprRewritePass, ast_rewrite::LoweredExpr, rewrite_expr::lower_expr,
    rewrite_future_annotations, rewrite_names, rewrite_stmt,
};
use crate::basic_block::bb_ir::BbModule;
use crate::basic_block::block_py::BlockPyModule;
use crate::PassTracker;
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};

pub fn rewrite_module(context: &Context, module: &mut StmtBody) -> (BlockPyModule, BbModule) {
    let mut pass_tracker = PassTracker::new();
    let bb_module = rewrite_module_with_tracker(context, module, &mut pass_tracker);
    let blockpy_module = pass_tracker
        .get::<BlockPyModule>()
        .expect("rewrite_module should always track semantic BlockPy")
        .clone();
    (blockpy_module, bb_module)
}

pub(crate) fn rewrite_module_with_tracker(
    context: &Context,
    module: &mut StmtBody,
    pass_tracker: &mut PassTracker,
) -> BbModule {
    // The transform now has a single lowering strategy: basic-block form.
    lower_surrogate_string_literals(context, module);

    rewrite_future_annotations::rewrite(context, module);

    // Rewrite names like "__foo" in class bodies to "_<class_name>__foo"
    rewrite_class_def::private::rewrite_private_names(context, module);

    // Replace annotated assignments ("x: int = 1") with regular assignments,
    // and either drop the annotations (in functions) or generate an
    // __annotate__ function (in modules and classes)
    rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(context, module);

    wrap_module_init(module);

    // Lower helper-scoped expressions that synthesize nested defs for Python
    // scoping semantics before the more direct BlockPy expr lowering boundary.
    rewrite_with_pass(context, None, Some(&ScopedHelperExprPass), module);

    // Lower many kinds of statements and expressions into simpler forms. This removes:
    // for, with, augassign, annassign, get/set/del item, unpack, multi-target assignment,
    // operators, and comparisons.
    rewrite_with_pass(
        context,
        Some(&basic_block::SingleNamedAssignmentPass),
        Some(&SimplifyExprPass),
        module,
    );
    pass_tracker.add_pass("rewritten_ast_after_initial_simplify", || {
        crate::RewrittenAstAfterInitialSimplify(module.clone())
    });

    let scope = analyze_module_scope(module);

    // Replace global / nonlocal and class-body scoping with explicit loads/stores.
    //  - globals: __dp__.load/store_global(globals(), name)
    //  - nonlocal: create a cell in the outermost scope, and access with __dp__.load/store_cell(cell, value)
    //  - class-body: class_body_load_cell/global(_dp_class_ns, name, cell / globals()) captures "try class, then outer"
    rewrite_names::rewrite_explicit_bindings(context, scope.clone(), module);

    rewrite_class_def::class_body::rewrite_class_body_scopes(context, scope, module);

    let lowered_blockpy_module = rewrite_ast_to_lowered_blockpy_module(context, module);
    pass_tracker.add_pass("rewritten_ast_for_lowering", || module.clone());
    pass_tracker.add_pass("semantic_blockpy", || {
        basic_block::project_lowered_module_callable_defs(&lowered_blockpy_module, |lowered| {
            &lowered.callable_def
        })
    });
    let core_blockpy_bundle =
        basic_block::simplify_lowered_blockpy_module_bundle_exprs(&lowered_blockpy_module);
    pass_tracker.add_pass("core_blockpy", || {
        basic_block::project_lowered_module_callable_defs(&core_blockpy_bundle, |lowered| {
            &lowered.callable_def
        })
    });
    let bb_module =
        basic_block::lower_core_blockpy_module_bundle_to_bb_module(&core_blockpy_bundle);
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

pub(crate) fn wrap_module_init(module: &mut StmtBody) {
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

    for stmt in std::mem::take(&mut module.body) {
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
    module.body = prelude;
}

pub struct SimplifyExprPass;

pub struct ScopedHelperExprPass;

impl ExprRewritePass for ScopedHelperExprPass {
    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        match expr {
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => lower_expr(context, expr),
            other => LoweredExpr::unmodified(other),
        }
    }
}

impl ExprRewritePass for SimplifyExprPass {
    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        match expr {
            Expr::If(_) => LoweredExpr::unmodified(expr),
            other => lower_expr(context, other),
        }
    }
}

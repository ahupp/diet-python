use super::context::Context;
use crate::basic_block;
use crate::basic_block::bb_ir::BbModule;
use crate::ensure_import;
use crate::transform::ast_rewrite::rewrite_with_pass;
use crate::transform::rewrite_class_def;
use crate::transform::scope::{analyze_module_scope, BindingKind};
use crate::transform::simplify::{lower_string_literals_to_bytes, strip_generated_passes};
use crate::transform::{
    ast_rewrite::ExprRewritePass, rewrite_expr, rewrite_future_annotations, rewrite_names,
};
use crate::transform::{ast_rewrite::LoweredExpr, rewrite_expr::lower_expr, rewrite_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use std::collections::HashMap;

pub struct RewriteModuleResult {
    pub function_name_map: HashMap<String, (String, String)>,
    pub bb_module: Option<BbModule>,
}

pub fn rewrite_module(context: &Context, module: &mut StmtBody) -> RewriteModuleResult {
    // The transform now has a single lowering strategy: basic-block form.
    rewrite_future_annotations::rewrite(context, module);

    // Rewrite names like "__foo" in class bodies to "_<class_name>__foo"
    rewrite_class_def::private::rewrite_private_names(context, module);

    // Replace annotated assignments ("x: int = 1") with regular assignments,
    // and either drop the annotations (in functions) or generate an
    // __annotate__ function (in modules and classes)
    rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(context, module);

    wrap_module_init(module);

    // Lower many kinds of statements and expressions into simpler forms. This removes:
    // for, with, augassign, annassign, get/set/del item, unpack, multi-target assignment,
    // operators, comparisons, and comprehensions.
    rewrite_with_pass(
        context,
        Some(&basic_block::BBSimplifyStmtPass),
        Some(&SimplifyExprPass),
        module,
    );

    let scope = analyze_module_scope(module);

    // Replace global / nonlocal and class-body scoping with explicit loads/stores.
    //  - globals: __dp__.load/store_global(globals(), name)
    //  - nonlocal: create a cell in the outermost scope, and access with __dp__.load/store_cell(cell, value)
    //  - class-body: class_body_load_cell/global(_dp_class_ns, name, cell / globals()) captures "try class, then outer"
    rewrite_names::rewrite_explicit_bindings(scope.clone(), module);

    rewrite_class_def::class_body::rewrite_class_body_scopes(context, scope, module);
    // Re-run simplification to lower any constructs introduced by later passes.
    rewrite_with_pass(
        context,
        Some(&basic_block::BBSimplifyStmtPass),
        Some(&SimplifyExprPass),
        module,
    );

    strip_generated_passes(context, module);

    let bb_scope = analyze_module_scope(module);
    let bb_function_identity =
        basic_block::collect_function_identity_by_node(module, bb_scope.clone());

    if context.options.truthy {
        rewrite_expr::truthy::rewrite(module);
    }

    if context.options.inject_import {
        ensure_import::ensure_imports(context, module);
    }

    // Lower string literals into byte-literal decode form in the normal transform
    // pipeline so downstream representations (including BB IR) see one
    // consistent pure-Python shape.
    lower_string_literals_to_bytes(module);

    let bb_module = if context.options.emit_basic_blocks {
        let bb_module = basic_block::rewrite_with_function_identity_and_collect_ir(
            context,
            module,
            bb_function_identity,
        );
        // BB lowering directly emits index-based destructuring for simple targets
        // (e.g. `a, b = _dp_tmp`). Complex targets still flow through the standard
        // assignment rewrite to preserve Python semantics.
        rewrite_with_pass(
            context,
            Some(&basic_block::BBSimplifyStmtPass),
            Some(&SimplifyExprPass),
            module,
        );
        strip_generated_passes(context, module);
        Some(bb_module)
    } else {
        None
    };

    RewriteModuleResult {
        function_name_map: HashMap::new(),
        bb_module,
    }
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

fn wrap_module_init(module: &mut StmtBody) {
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

impl ExprRewritePass for SimplifyExprPass {
    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        lower_expr(context, expr)
    }
}

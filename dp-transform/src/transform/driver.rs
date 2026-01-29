


use ruff_python_ast::{Expr, Stmt, StmtBody};
use super::{
    context::{Context},
    rewrite_import, 
};
use crate::transform::{rewrite_expr, rewrite_function_def, rewrite_future_annotations, rewrite_names};
use crate::transform::simplify::strip_generated_passes;
use crate::ensure_import;
use crate::transform::ast_rewrite::rewrite_with_pass;
use crate::transform::scope::{analyze_module_scope};
use crate::transform::{ast_rewrite::{LoweredExpr, Rewrite, RewritePass}, rewrite_expr::{lower_expr}, rewrite_stmt};
use crate::{
    transform::rewrite_class_def,
};



pub fn rewrite_module(context: &Context, module: &mut StmtBody) {

    rewrite_future_annotations::rewrite(context, module);

    // Rewrite names like "__foo" in class bodies to "_<class_name>__foo"
    rewrite_class_def::private::rewrite_private_names(context, module);

        
    // Replace annotated assignments ("x: int = 1") with regular assignments,
    // and either drop the annotations (in functions) or generate an
    // __annotate__ function (in modules and classes)
    rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(context, module);


    // Lower many kinds of statements and expressions into simpler forms. This removes:
    // for, with, augassign, annassign, get/set/del item, unpack, multi-target assignment,
    // operators, comparisons, and comprehensions.
    rewrite_with_pass(context, &SimplifyPass, module);

    let scope = analyze_module_scope(module);

    // Rename functions to _dp_fn_<original_name>, manually update
    // __qualname__/__name__ and apply decorators, and then assign the _dp_fn_
    // name back to the original name.  This:
    //  - give correct qualname even when the transform inserts functions
    //  - avoids making the method name visible inside method bodies.  
    //  - The assignment back to the original name will be re-written by later scoping passes
    //    to the correct global dict / cell load/store / class body load/store operation 
    rewrite_function_def::rewrite_function_defs(context, scope.clone(), module);

    rewrite_class_def::class_body::rewrite_class_body_scopes(context, scope.clone(), module);

    let scope = analyze_module_scope(module);

    // Replace global / nonlocal and class-body scoping with explicit loads/stores.
    //  - globals: __dp__.load/store_global(globals(), name)
    //  - nonlocal: create a cell in the outermost scope, and access with __dp__.load/store_cell(cell, value)
    //  - class-body: class_body_load_cell/global(_dp_class_ns, name, cell / globals()) captures "try class, then outer"
    rewrite_names::rewrite_explicit_bindings(scope, module);

    strip_generated_passes(context, module);

    if context.options.truthy {
        rewrite_expr::truthy::rewrite(module);
    }

    if context.options.cleanup_dp_globals {
        module
            .body
            .push(crate::py_stmt!("__dp__.cleanup_dp_globals(globals())").into());
    }
    if context.options.inject_import {
        ensure_import::ensure_imports(context, module);
    }
}


pub struct SimplifyPass;

impl RewritePass for SimplifyPass {

    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::With(with) => rewrite_stmt::with::rewrite(context, with),
            Stmt::While(while_stmt) => rewrite_stmt::loop_cond::rewrite_while(context, while_stmt),
            Stmt::For(for_stmt) => rewrite_stmt::loop_cond::rewrite_for(context, for_stmt),
            Stmt::Assert(assert) => rewrite_stmt::assert::rewrite(assert),
            Stmt::Try(try_stmt) => rewrite_stmt::exception::rewrite_try(try_stmt),
            Stmt::If(if_stmt) => rewrite_stmt::loop_cond::expand_if_chain(if_stmt),
            Stmt::Match(match_stmt) => rewrite_stmt::match_case::rewrite(context, match_stmt),
            Stmt::Import(import) => rewrite_import::rewrite(import),
            Stmt::ImportFrom(import_from) => {
                rewrite_import::rewrite_from(context, import_from)
            }
            Stmt::Assign(assign) => rewrite_stmt::assign_del::rewrite_assign(context, assign),
            Stmt::AugAssign(aug) => rewrite_stmt::assign_del::rewrite_aug_assign(context, aug),
            Stmt::Delete(del) => rewrite_stmt::assign_del::rewrite_delete(del),
            Stmt::Raise(raise) => rewrite_stmt::exception::rewrite_raise(raise),
            Stmt::TypeAlias(type_alias) => rewrite_stmt::type_alias::rewrite_type_alias(context, type_alias),
            Stmt::AnnAssign(_) => panic!("should be removed by rewrite_ann_assign_to_dunder_annotate"),

            other => Rewrite::Unmodified(other),
        }
    }

    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        lower_expr(context, expr)
    }
}

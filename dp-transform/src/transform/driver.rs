


use ruff_python_ast::{Expr, Stmt};
use super::{
    context::{Context},
    rewrite_import, 
};
use crate::{transform::{rewrite_expr, rewrite_future_annotations, rewrite_names}};
use crate::transform::simplify::strip_generated_passes;
use crate::{ensure_import};
use crate::transform::ast_rewrite::rewrite_with_pass;
use crate::transform::scope::{analyze_module_scope};
use crate::transform::{ast_rewrite::{LoweredExpr, Rewrite, RewritePass}, rewrite_expr::{lower_expr}, rewrite_stmt};
use crate::{
    transform::rewrite_class_def,
};


pub fn rewrite_module(context: &Context, module: &mut Vec<Stmt>) {
    
    rewrite_future_annotations::rewrite(module);

    // Rewrite private names before scope analysis
    rewrite_class_def::private::rewrite_class_body(module, None);

    rewrite_with_pass(context, &SimplifyPass, module);

    let scope = analyze_module_scope(module);

    rewrite_class_def::class_body::rewrite_class_body_scopes(context, scope.clone(), module);
    rewrite_names::rewrite_explicit_bindings(scope, module);
    rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(module);

    strip_generated_passes(module);

    if context.options.truthy {
        rewrite_expr::truthy::rewrite(module);
    }

    if context.options.cleanup_dp_globals {
        module.extend(crate::py_stmt!("__dp__.cleanup_dp_globals(globals())"));
    }

    if context.options.inject_import {
        ensure_import::ensure_import(module);
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
                rewrite_import::rewrite_from(context, import_from.clone())
            }
            Stmt::AnnAssign(ann_assign) => rewrite_stmt::assign_del::rewrite_ann_assign(ann_assign),
            Stmt::Assign(assign) => rewrite_stmt::assign_del::rewrite_assign(context, assign),
            Stmt::AugAssign(aug) => rewrite_stmt::assign_del::rewrite_aug_assign(context, aug),
            Stmt::Delete(del) => rewrite_stmt::assign_del::rewrite_delete(del),
            Stmt::Raise(raise) => rewrite_stmt::exception::rewrite_raise(raise),
            other => Rewrite::Unmodified(other),
        }
    }


    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        lower_expr(context, expr)
    }
}

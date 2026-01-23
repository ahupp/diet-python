

use log::{log_enabled, trace, Level};
use ruff_python_ast::{self as ast, Expr, Stmt};
use super::{
    context::{Context},
    rewrite_import, 
};
use crate::transform::{rewrite_expr, rewrite_future_annotations};
use crate::transform::simplify::strip_generated_passes;
use crate::{ensure_import, py_expr, template};
use crate::transform::ast_rewrite::rewrite_with_pass;
use crate::transform::scope::{Scope, analyze_module_scope};
use crate::transform::{ast_rewrite::{LoweredExpr, Rewrite, RewritePass}, rewrite_expr::{comprehension, lower_expr}, rewrite_stmt};
use crate::{
    transform::rewrite_class_def,
};


pub fn rewrite_module(context: &Context, module: &mut Vec<Stmt>) {
    
    rewrite_future_annotations::rewrite(module);
    rewrite_with_pass(context, &SimplifyPass, module);

    let scope = analyze_module_scope(module);

    let class_pass = ScopeAwareLoweringPass::new(scope);
    rewrite_with_pass(context, &class_pass, module);

    // Collapse `py_stmt!` templates after all rewrites.
    template::flatten(module);

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
            Stmt::If(if_stmt)
                if if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| clause.test.is_some()) =>
            {
                Rewrite::Visit(vec![rewrite_stmt::loop_cond::expand_if_chain(if_stmt).into()])
            }
            Stmt::Match(match_stmt) => rewrite_stmt::match_case::rewrite(context, match_stmt),
            Stmt::Import(import) => rewrite_import::rewrite(import, &context.options),
            Stmt::ImportFrom(import_from) => {
                rewrite_import::rewrite_from(context, import_from.clone())
            }

            Stmt::AnnAssign(ann_assign) => rewrite_stmt::assign_del::rewrite_ann_assign(ann_assign),
            Stmt::Assign(assign) => rewrite_stmt::assign_del::rewrite_assign(context, assign),
            Stmt::AugAssign(aug) => rewrite_stmt::assign_del::rewrite_aug_assign(context, aug),
            Stmt::Delete(del) => rewrite_stmt::assign_del::rewrite_delete(del),
            Stmt::Raise(raise) => rewrite_stmt::exception::rewrite_raise(raise),
            other => Rewrite::Walk(vec![other]),
        }
    }


    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        lower_expr(context, expr)
    }
}

struct ScopeAwareLoweringPass {
    scope: std::sync::Arc<Scope>,
}

impl ScopeAwareLoweringPass {
    fn new(scope: std::sync::Arc<Scope>) -> Self {
        Self {
            scope
        }
    }
}

impl RewritePass for ScopeAwareLoweringPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        let (mut output, visit) = match stmt {
            Stmt::FunctionDef(mut func_def) => {
                if log_enabled!(Level::Trace) {
                    trace!(
                        "class_lower: function {} body_len={}",
                        func_def.name.id,
                        func_def.body.len()
                    );
                }

                let scope = self.scope.ensure_child_scope_for_function(&func_def);
        
                let pass = ScopeAwareLoweringPass::new(scope);
                rewrite_with_pass(context, &pass, &mut func_def.body);

                rewrite_stmt::annotation::rewrite_ann_assign_delete(&mut func_def.body);

                (vec![Stmt::FunctionDef(func_def)], false)
            }
            Stmt::ClassDef(mut class_def) => {
                if log_enabled!(Level::Trace) {
                    trace!(
                        "class_lower: class {} body_len={}",
                        class_def.name.id,
                        class_def.body.len()
                    );
                }
             
                // Rewrite private names before scope analysis
                rewrite_class_def::private::rewrite_class_body(&mut class_def.body, &class_def.name.id.to_string());

                let class_scope = self.scope.ensure_child_scope_for_class(&class_def);   

                rewrite_stmt::annotation::rewrite_ann_assign_to_dunder_annotate(&mut class_def.body);

                match rewrite_class_def::rewrite(context, class_scope.as_ref(), class_def) {
                    Rewrite::Walk(stmts) => (stmts, false),
                    Rewrite::Visit(stmts) => (stmts, true),
                }
            }
            _ => (vec![stmt], false),
        };

        rewrite_with_pass(context, &SimplifyPass, &mut output);

        if visit {
            Rewrite::Visit(output)
        } else {
            Rewrite::Walk(output)
        }
    }

    fn lower_expr(&self, context: &Context, expr: Expr) -> LoweredExpr {
        match expr {
            Expr::Lambda(lambda) => {
                comprehension::rewrite_lambda(lambda, context, self.scope.as_ref())
            }
            Expr::Generator(generator) => {
                comprehension::rewrite_generator(generator, context, self.scope.as_ref())
            }
            Expr::ListComp(ast::ExprListComp {
                elt, generators, ..
            }) => {
                comprehension::rewrite(context, self.scope.as_ref(),  *elt, generators, "list", "append")
            }
            Expr::SetComp(ast::ExprSetComp {
                elt, generators, ..
            }) => {
                comprehension::rewrite(context, self.scope.as_ref(), *elt, generators, "set", "add")
            }
            Expr::DictComp(ast::ExprDictComp {
                key,
                value,
                generators,
                ..
            }) => {
                comprehension::rewrite(context, 
                    self.scope.as_ref(), 
                    py_expr!("({key:expr}, {value:expr})", key = *key, value = *value), 
                    generators, "dict", 
                    "update"
                )
            }
    
            _ => lower_expr(context, expr)
        }
    }
}

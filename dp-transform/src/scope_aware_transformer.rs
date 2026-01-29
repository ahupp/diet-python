use std::{sync::Arc};

use ruff_python_ast::Stmt;

use crate::{Scope, transformer::{Transformer, walk_stmt}};



pub trait ScopeAwareTransformer: Transformer + Sized {


    fn enter_scope(&self, scope: Arc<Scope>) -> Self;
    
    fn scope(&self) -> &Arc<Scope>;

    fn visit_stmt_scope_aware(&mut self, stmt: &mut Stmt) {
        Transformer::visit_stmt(self, stmt);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                for decorator in &mut func_def.decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = func_def.type_params.as_mut() {
                    self.visit_type_params(type_params);
                }
                self.visit_parameters(&mut func_def.parameters);
                if let Some(returns) = func_def.returns.as_mut() {
                    self.visit_annotation(returns);
                }
        
                let func_scope = self.scope().tree.child_scope_for_function(func_def)
                    .expect("no child scope for class");
                let mut child_transformer = self.enter_scope(func_scope);
                child_transformer.visit_stmt_scope_aware(stmt);
            }
            Stmt::ClassDef(class_def) => {
                for decorator in &mut class_def.decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = class_def.type_params.as_mut() {
                    self.visit_type_params(type_params);
                }
                if let Some(arguments) = class_def.arguments.as_mut() {
                    self.visit_arguments(arguments);
                }
        
                let class_scope = self.scope().tree.child_scope_for_class(class_def)
                    .expect("no child scope for class");
                let mut child_transformer = self.enter_scope(class_scope);
                child_transformer.visit_stmt_scope_aware(stmt);
            }
            _ => {
                walk_stmt(self, stmt);
            }
        }
    }
}

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use super::{
    body::{suite_mut, Suite},
    context::Context,
    semantic::{SemanticAstState, SemanticScope, SemanticScopeKind},
};
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{
    passes::ast_to_ast::{ast_rewrite::Rewrite, rewrite_import},
    passes::ruff_to_blockpy,
};

pub fn rewrite_explicit_bindings(
    context: &Context,
    semantic_state: &SemanticAstState,
    body: &mut Suite,
) {
    let mut rewriter = NameScopeRewriter::new(context, semantic_state.module_scope());
    rewriter.visit_body(body);
}

struct NameScopeRewriter<'a> {
    context: &'a Context,
    scope: SemanticScope,
}

impl<'a> NameScopeRewriter<'a> {
    fn new(context: &'a Context, scope: SemanticScope) -> Self {
        Self { context, scope }
    }

    fn is_class_scope(&self) -> bool {
        matches!(self.scope.kind(), SemanticScopeKind::Class)
    }

    fn visit_target_expr_preserving_names(&mut self, expr: &mut Expr) {
        if matches!(
            expr,
            Expr::Name(ast::ExprName {
                ctx: ExprContext::Store | ExprContext::Del,
                ..
            })
        ) {
            return;
        }
        walk_expr(self, expr);
    }
}

impl Transformer for NameScopeRewriter<'_> {
    fn visit_body(&mut self, body: &mut Suite) {
        let mut rewritten = Vec::with_capacity(body.len());
        for stmt in std::mem::take(body) {
            for mut stmt in self.rewrite_stmt_list(stmt) {
                self.visit_stmt(&mut stmt);
                rewritten.push(stmt);
            }
        }
        *body = rewritten;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::For(for_stmt) => {
                self.visit_expr(for_stmt.iter.as_mut());
                self.visit_target_expr_preserving_names(for_stmt.target.as_mut());
                self.visit_body(suite_mut(&mut for_stmt.body));
                self.visit_body(suite_mut(&mut for_stmt.orelse));
            }
            Stmt::With(with_stmt) => {
                for item in &mut with_stmt.items {
                    self.visit_expr(&mut item.context_expr);
                    if let Some(optional_vars) = item.optional_vars.as_mut() {
                        self.visit_target_expr_preserving_names(optional_vars.as_mut());
                    }
                }
                self.visit_body(suite_mut(&mut with_stmt.body));
            }
            Stmt::Global(_) => return,
            Stmt::Nonlocal(_) => return,
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                assert!(targets.len() == 1);
                self.visit_expr(value.as_mut());
            }
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

                let child_scope = self
                    .scope
                    .child_scope_for_function(func_def)
                    .expect("no child scope for function");

                let mut child_rewriter = NameScopeRewriter::new(self.context, child_scope);
                child_rewriter.visit_body(suite_mut(&mut func_def.body));
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

                let class_scope = self
                    .scope
                    .child_scope_for_class(class_def)
                    .expect("no child scope for class");

                NameScopeRewriter::new(self.context, class_scope)
                    .visit_body(suite_mut(&mut class_def.body));
            }
            Stmt::AnnAssign(_) => {
                panic!("AnnAssign should be gone now");
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if self.is_class_scope() {
            match expr {
                Expr::Lambda(ast::ExprLambda { parameters, .. }) => {
                    if let Some(parameters) = parameters {
                        self.visit_parameters(parameters);
                    }
                    return;
                }
                Expr::Generator(ast::ExprGenerator { generators, .. })
                | Expr::ListComp(ast::ExprListComp { generators, .. })
                | Expr::SetComp(ast::ExprSetComp { generators, .. })
                | Expr::DictComp(ast::ExprDictComp { generators, .. }) => {
                    if let Some(first) = generators.first_mut() {
                        self.visit_expr(&mut first.iter);
                    }
                    return;
                }
                _ => {}
            }
        }
        match expr {
            Expr::Named(named) => {
                self.visit_expr(named.value.as_mut());
                return;
            }
            Expr::Name(name) if matches!(name.ctx, ExprContext::Store | ExprContext::Del) => {
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}

impl NameScopeRewriter<'_> {
    fn rewrite_stmt_list(&self, stmt: Stmt) -> Vec<Stmt> {
        match stmt {
            Stmt::Import(import) => self.rewrite_nested_stmt_list(rewrite_import::rewrite(import)),
            Stmt::ImportFrom(import_from) => self
                .rewrite_nested_stmt_list(rewrite_import::rewrite_from(self.context, import_from)),
            Stmt::TypeAlias(type_alias) => self.rewrite_nested_stmt_list(
                ruff_to_blockpy::rewrite_type_alias_stmt(self.context, type_alias),
            ),
            Stmt::AugAssign(augassign) => self.rewrite_nested_stmt_list(
                ruff_to_blockpy::rewrite_augassign_stmt(self.context, augassign),
            ),
            other => vec![other],
        }
    }

    fn rewrite_nested_stmt_list(&self, rewrite: Rewrite) -> Vec<Stmt> {
        match rewrite {
            Rewrite::Unmodified(stmt) => vec![stmt],
            Rewrite::Walk(stmts) => stmts
                .into_iter()
                .flat_map(|stmt| self.rewrite_stmt_list(stmt))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::rewrite_explicit_bindings;
    use crate::passes::ast_to_ast::context::Context;
    use crate::passes::ast_to_ast::semantic::SemanticAstState;
    use crate::passes::ast_to_ast::Options;
    use ruff_python_parser::parse_module;

    #[test]
    fn recursive_local_function_does_not_emit_early_function_binding_cell_sync() {
        let source = concat!(
            "def outer():\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    return recurse()\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let semantic_state = SemanticAstState::from_ruff(&mut module);
        rewrite_explicit_bindings(&context, &semantic_state, &mut module);
        let rendered = module
            .iter()
            .map(crate::ruff_ast_to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
            "{rendered}"
        );
    }

    #[test]
    fn nested_class_binding_does_not_emit_stale_local_cell_sync() {
        let source = concat!(
            "def outer():\n",
            "    class A:\n",
            "        pass\n",
            "    class B:\n",
            "        def probe(self):\n",
            "            return A\n",
            "    return B\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let semantic_state = SemanticAstState::from_ruff(&mut module);
        rewrite_explicit_bindings(&context, &semantic_state, &mut module);
        let rendered = module
            .iter()
            .map(crate::ruff_ast_to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !rendered.contains("__dp_store_cell(_dp_cell_A, A)"),
            "{rendered}"
        );
    }
}

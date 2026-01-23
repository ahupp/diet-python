use ruff_python_ast::{self as ast, name::Name, Expr, ExprContext, Stmt};

use crate::{body_transform::{Transformer, walk_expr, walk_parameter, walk_stmt}};
use log::{log_enabled, trace, Level};


pub(crate) fn rewrite_class_body<'a>(
    body: &mut Vec<Stmt>,
    class_name: &str,
    ) {
    if log_enabled!(Level::Trace) {
        trace!(
            "rewrite_class_body: class {} body_len={}",
            class_name,
            body.len()
        );
    }
    let mut rewriter = PrivateRewriter::new(class_name);
    rewriter.visit_body(body);
}

struct PrivateRewriter {
    class_name: String,
}

impl PrivateRewriter {
    fn new(class_name: &str) -> Self {
        Self {
            class_name: class_name.to_string(),
        }
    }

    pub fn maybe_mangle(&self, attr: &str) -> Option<String> {
        if !attr.starts_with("__") || attr.ends_with("__") {
            return None;
        }

        let mut class_name = self.class_name.as_str();
        while class_name.starts_with('_') {
            class_name = &class_name[1..];
        }

        if class_name.is_empty() {
            return None;
        }

        Some(format!("_{}{}", class_name, attr))
    }

    fn mangle_identifier(&self, name: &mut ast::Identifier) {
        if let Some(mangled) = self.maybe_mangle(name.as_str()) {
            name.id = Name::new(mangled);
        }
    }

    fn mangle_name(&self, name: &mut Name) {
        if let Some(mangled) = self.maybe_mangle(name.as_str()) {
            *name = Name::new(mangled);
        }
    }

}

impl Transformer for PrivateRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::ClassDef(_) => {
                // Do not recurse into nested classes; they are rewritten separately.
            }
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                for name in names {
                    self.mangle_identifier(name);
                }
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    self.mangle_identifier(name);
                }
            }
            Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) => {
                self.mangle_name(&mut name.id);
                walk_stmt(self, stmt);
            }
            _ => {
                walk_stmt(self, stmt);
            }
        }
    }

    fn visit_parameter(&mut self, parameter: &mut ast::Parameter) {
        self.mangle_identifier(&mut parameter.name);
        walk_parameter(self, parameter);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, ctx, .. })
                if matches!(
                    ctx,
                    ExprContext::Load | ExprContext::Store | ExprContext::Del
                ) =>
            {
                self.mangle_name(id);
            }
            Expr::Attribute(ast::ExprAttribute { attr, .. }) => {
                self.mangle_identifier(attr);
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

use std::sync::Arc;
use ruff_python_ast::StmtBody;
use crate::template::into_body;
use crate::transformer::{Transformer, walk_stmt};
use ruff_python_ast::{name::Name, Stmt};

use crate::transform::rewrite_stmt;
use crate::{Scope, py_expr, py_stmt};
use crate::transform::context::Context;

pub fn rewrite_function_defs(context: &Context, scope: Arc<Scope>, body: &mut StmtBody) {
    let mut rewriter = FunctionDefRewriter {
        context,
        scope,
    };
    rewriter.visit_body(body);
}

struct FunctionDefRewriter<'a> {
    context: &'a Context,
    scope: Arc<Scope>,
}

impl<'a> Transformer for FunctionDefRewriter<'a> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                let original_name = func_def.name.id.to_string();
                let display_name = if original_name.starts_with("_dp_lambda_") {
                    "<lambda>"
                } else if original_name.starts_with("_dp_genexpr_") {
                    "<genexpr>"
                } else if original_name.starts_with("_dp_listcomp_") {
                    "<listcomp>"
                } else if original_name.starts_with("_dp_setcomp_") {
                    "<setcomp>"
                } else if original_name.starts_with("_dp_dictcomp_") {
                    "<dictcomp>"
                } else {
                    original_name.as_str()
                };

                let child_scope = self.scope.child_scope_for_function(func_def).unwrap();

                let qualname = child_scope.qualnamer.qualname.to_string();
                let mut rewriter = FunctionDefRewriter {
                    context: self.context,
                    scope: child_scope,
                };
                rewriter.visit_body(&mut func_def.body);
                
                let decorators = std::mem::take(&mut func_def.decorator_list);

                let temp_name = format!("_dp_fn_{original_name}");
                func_def.name.id = Name::new(temp_name.as_str());

                // Decorators should be applied after name/qualname updates
                let decorated = rewrite_stmt::decorator::rewrite(
                    decorators,
                    py_expr!(r"{temp_name:id}", temp_name = temp_name.as_str()),
                );

                let suffix = py_stmt!(r#"
__dp__.update_fn({temp_name:id}, {qualname:literal}, {display_name:literal})
{original_name:id} = {decorated:expr}
del {temp_name:id}
"#,
                    decorated = decorated,
                    original_name = original_name.as_str(),
                    temp_name = temp_name.as_str(),
                    qualname = qualname.as_str(),
                    display_name = display_name,
                );
                *stmt = into_body(vec![func_def.clone().into(), suffix]);
            }
            Stmt::ClassDef(class_def) => {

                let child_scope = self.scope.child_scope_for_class(class_def).unwrap();

                let mut class_rewriter = FunctionDefRewriter {
                    context: self.context,
                    scope: child_scope,
                };
                class_rewriter.visit_body(&mut class_def.body);
            }
            _ => walk_stmt(self, stmt),
        }
    }

}

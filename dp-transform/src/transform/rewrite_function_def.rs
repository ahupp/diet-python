use crate::template::into_body;
use crate::transformer::{walk_stmt, Transformer};
use ruff_python_ast::StmtBody;
use ruff_python_ast::{name::Name, Stmt};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::transform::context::Context;
use crate::transform::rewrite_stmt;
use crate::{py_expr, py_stmt, Scope};

pub type FunctionRenameMap = HashMap<String, (String, String)>;

pub fn rewrite_function_defs(
    context: &Context,
    scope: Arc<Scope>,
    body: &mut StmtBody,
) -> FunctionRenameMap {
    let renamed = Rc::new(RefCell::new(FunctionRenameMap::new()));
    let mut rewriter = FunctionDefRewriter {
        context,
        scope,
        next_fn_id: Rc::new(Cell::new(0)),
        renamed: renamed.clone(),
    };
    rewriter.visit_body(body);
    let result = renamed.borrow().clone();
    result
}

struct FunctionDefRewriter<'a> {
    context: &'a Context,
    scope: Arc<Scope>,
    next_fn_id: Rc<Cell<usize>>,
    renamed: Rc<RefCell<FunctionRenameMap>>,
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
                    next_fn_id: self.next_fn_id.clone(),
                    renamed: self.renamed.clone(),
                };
                rewriter.visit_body(&mut func_def.body);

                let decorators = std::mem::take(&mut func_def.decorator_list);

                let fn_id = self.next_fn_id.get();
                self.next_fn_id.set(fn_id + 1);
                let temp_name = format!("_dp_fn_{original_name}_{fn_id}");
                func_def.name.id = Name::new(temp_name.as_str());
                self.renamed.borrow_mut().insert(
                    temp_name.clone(),
                    (display_name.to_string(), qualname.clone()),
                );

                // Decorators should be applied after name/qualname updates
                let decorated = rewrite_stmt::decorator::rewrite(
                    decorators,
                    py_expr!(r"{temp_name:id}", temp_name = temp_name.as_str()),
                );

                let suffix = if self.context.options.eval_mode {
                    py_stmt!(
                        r#"
{original_name:id} = {decorated:expr}
del {temp_name:id}
"#,
                        decorated = decorated,
                        original_name = original_name.as_str(),
                        temp_name = temp_name.as_str(),
                    )
                } else {
                    py_stmt!(
                        r#"
__dp__.update_fn({temp_name:id}, {qualname:literal}, {display_name:literal})
{original_name:id} = {decorated:expr}
del {temp_name:id}
"#,
                        decorated = decorated,
                        original_name = original_name.as_str(),
                        temp_name = temp_name.as_str(),
                        qualname = qualname.as_str(),
                        display_name = display_name,
                    )
                };
                *stmt = into_body(vec![func_def.clone().into(), suffix]);
            }
            Stmt::ClassDef(class_def) => {
                let child_scope = self.scope.child_scope_for_class(class_def).unwrap();

                let mut class_rewriter = FunctionDefRewriter {
                    context: self.context,
                    scope: child_scope,
                    next_fn_id: self.next_fn_id.clone(),
                    renamed: self.renamed.clone(),
                };
                class_rewriter.visit_body(&mut class_def.body);
            }
            _ => walk_stmt(self, stmt),
        }
    }
}

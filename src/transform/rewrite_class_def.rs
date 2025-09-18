use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::make_tuple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;
use std::cell::Cell;

struct MethodTransformer {
    uses_class: Cell<bool>,
    first_arg: Option<String>,
}

impl Transformer for MethodTransformer {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        match expr {
            Expr::Call(call) => {
                if let Expr::Name(ast::ExprName { id, .. }) = call.func.as_ref() {
                    if id == "super"
                        && call.arguments.args.is_empty()
                        && call.arguments.keywords.is_empty()
                    {
                        if let Some(arg) = &self.first_arg {
                            *expr = py_expr!("super({arg:id}, __class__)", arg = arg.as_str());
                            self.uses_class.set(true);
                        }
                    }
                }
            }
            Expr::Name(ast::ExprName { id, .. }) => {
                if id == "__class__" {
                    self.uses_class.set(true);
                }
            }
            _ => {}
        }
    }
}

fn rewrite_method(func_def: &mut ast::StmtFunctionDef, class_name: &str) {
    let first_arg = func_def
        .parameters
        .posonlyargs
        .first()
        .map(|a| a.parameter.name.to_string())
        .or_else(|| {
            func_def
                .parameters
                .args
                .first()
                .map(|a| a.parameter.name.to_string())
        });

    let transformer = MethodTransformer {
        uses_class: Cell::new(false),
        first_arg,
    };
    for stmt in &mut func_def.body {
        walk_stmt(&transformer, stmt);
    }
    if transformer.uses_class.get() {
        let cls_name = format!("_dp_class_{}", class_name);
        let assign = py_stmt!("__class__ = {c:id}", c = cls_name.as_str());
        func_def.body.insert(0, assign);
    }
}

pub fn rewrite(
    ast::StmtClassDef {
        name,
        body,
        arguments,
        ..
    }: ast::StmtClassDef,
    decorated: bool,
) -> Stmt {
    let class_name = name.id.as_str().to_string();

    // Build namespace function body
    // TODO: correctly calculate the qualname of the class when nested
    let mut ns_body = Vec::new();

    let mut original_body = body;
    if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = original_body.first() {
        if let Expr::StringLiteral(_) = value.as_ref() {
            ns_body.push(py_stmt!(
                r#"_dp_temp_ns["__doc__"] = _ns["__doc__"] = {doc:expr}"#,
                doc = value.clone(),
            ));
            original_body.remove(0);
        }
    }

    for stmt in original_body {
        match stmt {
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                if let [Expr::Name(ast::ExprName { id, .. })] = targets.as_slice() {
                    ns_body.push(py_stmt!(
                        r#"_dp_temp_ns[{id:literal}] = _ns[{id:literal}] = {v:expr}"#,
                        id = id.as_str(),
                        v = value
                    ));
                }
            }
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(v),
                ..
            }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    ns_body.push(py_stmt!(
                        r#"_dp_temp_ns[{id:literal}] = _ns[{id:literal}] = {v:expr}"#,
                        id = id.as_str(),
                        v = *v
                    ));
                }
            }
            Stmt::FunctionDef(mut func_def) => {
                rewrite_method(&mut func_def, &class_name);
                let fn_name = func_def.name.id.to_string();

                let mk_func = py_stmt!(
                    r#"
def _dp_mk_{fn_name:id}():
    {fn_def:stmt}
    {fn_name:id}.__qualname__ = _ns["__qualname__"] + {suffix:literal}
    return {fn_name:id}

_dp_temp_ns[{fn_name:literal}] = _ns[{fn_name:literal}] = _dp_mk_{fn_name:id}()
                "#,
                    fn_def = Stmt::FunctionDef(func_def),
                    fn_name = fn_name.as_str(),
                    suffix = format!(".{}", fn_name)
                );
                ns_body.push(mk_func);
            }
            other => ns_body.push(other),
        }
    }

    // Build class helper function
    let mut bases = Vec::new();
    let mut kw_keys = Vec::new();
    let mut kw_vals = Vec::new();
    if let Some(args) = arguments {
        bases.extend(args.args.into_vec());
        for kw in args.keywords.into_vec() {
            if let Some(arg) = kw.arg {
                kw_keys.push(py_expr!("{arg:literal}", arg = arg.as_str()));
                kw_vals.push(kw.value);
            }
        }
    }
    let has_kw = !kw_keys.is_empty();

    let prepare_dict = if has_kw {
        let items: Vec<ast::DictItem> = kw_keys
            .into_iter()
            .zip(kw_vals.into_iter())
            .map(|(k, v)| ast::DictItem {
                key: Some(k),
                value: v,
            })
            .collect();
        Expr::Dict(ast::ExprDict {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            items,
        })
    } else {
        py_expr!("None")
    };

    let final_assignment = if decorated {
        py_stmt!(
            "_dp_class_{class_name:id} = _dp_make_class_{class_name:id}()",
            class_name = class_name.as_str(),
        )
    } else {
        py_stmt!(
            "{class_name:id} = _dp_class_{class_name:id} = _dp_make_class_{class_name:id}()",
            class_name = class_name.as_str(),
        )
    };

    py_stmt!(
        r#"
def _dp_ns_{class_name:id}(_ns):
    _dp_temp_ns = {}
    _dp_temp_ns["__module__"] = _ns["__module__"] = __name__
    _dp_temp_ns["__qualname__"] = _ns["__qualname__"] = {class_name:literal}

    {ns_body:stmt}

def _dp_make_class_{class_name:id}():
    bases = __dp__.resolve_bases({bases:expr})
    meta, ns, kwds = __dp__.prepare_class({class_name:literal}, bases, {prepare_dict:expr})
    _dp_ns_{class_name:id}(ns)
    return meta({class_name:literal}, bases, ns, **kwds)

{final_assignment:stmt}
"#,
        bases = make_tuple(bases),
        class_name = class_name.as_str(),
        ns_body = ns_body,
        prepare_dict = prepare_dict,
        final_assignment = final_assignment,
    )
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_class_def.txt");
}

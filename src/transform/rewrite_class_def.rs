use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::{make_tuple, single_stmt};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};
use ruff_text_size::TextRange;
use std::collections::BTreeSet;
use std::mem::take;

#[derive(Default)]
struct LoadBeforeStoreCollector {
    loads: BTreeSet<String>,
    seen_store: BTreeSet<String>,
    captured_names: BTreeSet<String>,
}

struct LoadBeforeStoreResult {
    captured_names: BTreeSet<String>,
}

impl LoadBeforeStoreCollector {
    fn collect_from_body(body: &mut Vec<Stmt>) -> LoadBeforeStoreResult {
        let mut collector = Self::default();
        collector.visit_body(body);
        collector.finish()
    }

    fn finish(self) -> LoadBeforeStoreResult {
        LoadBeforeStoreResult {
            captured_names: self.captured_names,
        }
    }
}

impl Transformer for LoadBeforeStoreCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef {
                decorator_list,
                parameters,
                returns,
                type_params,
                ..
            }) => {
                for decorator in decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = type_params {
                    self.visit_type_params(type_params);
                }
                self.visit_parameters(parameters);
                if let Some(expr) = returns {
                    self.visit_annotation(expr);
                }
            }
            Stmt::ClassDef(ast::StmtClassDef {
                decorator_list,
                arguments,
                type_params,
                ..
            }) => {
                for decorator in decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = type_params {
                    self.visit_type_params(type_params);
                }
                if let Some(arguments) = arguments {
                    self.visit_arguments(arguments);
                }
            }
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                self.visit_expr(value);
                for target in targets.iter_mut() {
                    self.visit_expr(target);
                }

                if let [Expr::Name(ast::ExprName { id, .. })] = targets.as_slice() {
                    let name = id.as_str();
                    if self.loads.contains(name) {
                        self.captured_names.insert(name.to_string());
                    }
                }
            }
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => {
                self.visit_expr(value);
                self.visit_expr(target);

                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    let name = id.as_str();
                    if self.loads.contains(name) {
                        self.captured_names.insert(name.to_string());
                    }
                }
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, ctx, .. }) => match ctx {
                ExprContext::Load => {
                    if !self.seen_store.contains(id.as_str()) {
                        self.loads.insert(id.to_string());
                    }
                }
                ExprContext::Store => {
                    let name = id.to_string();
                    self.seen_store.insert(name.clone());
                }
                _ => {}
            },
            Expr::Lambda(ast::ExprLambda { parameters, .. }) => {
                if let Some(parameters) = parameters {
                    self.visit_parameters(parameters);
                }
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

struct MethodTransformer {
    uses_class: bool,
    first_arg: Option<String>,
}

impl Transformer for MethodTransformer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        walk_expr(self, expr);
        match expr {
            Expr::Call(call) => {
                if let Expr::Name(ast::ExprName { id, .. }) = call.func.as_ref() {
                    if id == "super"
                        && call.arguments.args.is_empty()
                        && call.arguments.keywords.is_empty()
                    {
                        if let Some(arg) = &self.first_arg {
                            *expr = py_expr!("super(__class__, {arg:id})", arg = arg.as_str());
                            self.uses_class = true;
                        }
                    }
                }
            }
            Expr::Name(ast::ExprName { id, .. }) => {
                if id == "__class__" {
                    self.uses_class = true;
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

    let mut transformer = MethodTransformer {
        uses_class: false,
        first_arg,
    };
    for stmt in &mut func_def.body {
        walk_stmt(&mut transformer, stmt);
    }
    if transformer.uses_class {
        let cls_name = format!("_dp_class_{}", class_name);
        let assign = py_stmt!("__class__ = {c:id}", c = cls_name.as_str());
        func_def.body.insert(0, assign);
    }
}

pub fn rewrite(
    ast::StmtClassDef {
        name,
        mut body,
        arguments,
        ..
    }: ast::StmtClassDef,
    decorated: bool,
) -> Stmt {
    let class_name = name.id.as_str().to_string();

    let LoadBeforeStoreResult { captured_names, .. } =
        LoadBeforeStoreCollector::collect_from_body(&mut body);

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
                    let name = id.as_str().to_string();
                    let assign_stmt = py_stmt!(
                        "{name:id} = {value:expr}",
                        name = name.as_str(),
                        value = value,
                    );
                    ns_body.push(assign_stmt);
                    ns_body.push(py_stmt!(
                        r#"_dp_temp_ns[{name:literal}] = _ns[{name:literal}] = {name:id}"#,
                        name = name.as_str(),
                    ));
                }
            }
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(v),
                ..
            }) => {
                if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                    let name = id.as_str().to_string();
                    let assign_stmt =
                        py_stmt!("{name:id} = {value:expr}", name = name.as_str(), value = *v,);
                    ns_body.push(assign_stmt);
                    ns_body.push(py_stmt!(
                        r#"_dp_temp_ns[{name:literal}] = _ns[{name:literal}] = {name:id}"#,
                        name = name.as_str(),
                    ));
                }
            }
            Stmt::FunctionDef(mut func_def) => {
                rewrite_method(&mut func_def, &class_name);
                let fn_name = func_def.name.id.to_string();

                let decorators = take(&mut func_def.decorator_list);
                let mut decorator_names = Vec::new();
                for (index, decorator) in decorators.into_iter().enumerate() {
                    let decorator_name = format!("_dp_dec_{}_{index}", fn_name);
                    ns_body.push(py_stmt!(
                        "{decor_name:id} = {decor:expr}",
                        decor_name = decorator_name.as_str(),
                        decor = decorator.expression
                    ));
                    decorator_names.push(decorator_name);
                }

                let mk_func_def = py_stmt!(
                    r#"
def _dp_mk_{fn_name:id}():
    {fn_def:stmt}
    {fn_name:id}.__qualname__ = _ns["__qualname__"] + {suffix:literal}
    return {fn_name:id}
                "#,
                    fn_def = Stmt::FunctionDef(func_def),
                    fn_name = fn_name.as_str(),
                    suffix = format!(".{}", fn_name)
                );
                ns_body.push(mk_func_def);

                ns_body.push(py_stmt!(
                    "{fn_name:id} = _dp_mk_{fn_name:id}()",
                    fn_name = fn_name.as_str()
                ));

                for decorator_name in decorator_names.iter().rev() {
                    ns_body.push(py_stmt!(
                        "{fn_name:id} = {decor_name:id}({fn_name:id})",
                        fn_name = fn_name.as_str(),
                        decor_name = decorator_name.as_str()
                    ));
                }

                ns_body.push(py_stmt!(
                    "_dp_temp_ns[{fn_name:literal}] = _ns[{fn_name:literal}] = {fn_name:id}",
                    fn_name = fn_name.as_str()
                ));
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

    let mut ns_fn = py_stmt!(
        r#"
def _dp_ns_{class_name:id}(_ns):
    _dp_temp_ns = {}
    _dp_temp_ns["__module__"] = _ns["__module__"] = __name__
    _dp_temp_ns["__qualname__"] = _ns["__qualname__"] = {class_name:literal}

    {ns_body:stmt}
"#,
        class_name = class_name.as_str(),
        ns_body = ns_body,
    );

    if let Stmt::FunctionDef(ast::StmtFunctionDef { parameters, .. }) = &mut ns_fn {
        for name in captured_names {
            parameters.args.push(ast::ParameterWithDefault {
                range: TextRange::default(),
                node_index: ast::AtomicNodeIndex::default(),
                parameter: ast::Parameter {
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                    name: ast::Identifier::new(name.clone(), TextRange::default()),
                    annotation: None,
                },
                default: Some(Box::new(py_expr!("{name:id}", name = name.as_str()))),
            });
        }
    } else {
        unreachable!("expected function definition for namespace helper");
    }

    let make_fn = py_stmt!(
        r#"
def _dp_make_class_{class_name:id}():
    orig_bases = {bases:expr}
    bases = __dp__.resolve_bases(orig_bases)
    meta, ns, kwds = __dp__.prepare_class({class_name:literal}, bases, {prepare_dict:expr})
    _dp_ns_{class_name:id}(ns)
    if orig_bases is not bases and "__orig_bases__" not in ns:
        ns["__orig_bases__"] = orig_bases
    return meta({class_name:literal}, bases, ns, **kwds)
"#,
        bases = make_tuple(bases),
        class_name = class_name.as_str(),
        prepare_dict = prepare_dict,
    );

    let final_assignment = final_assignment;

    single_stmt(vec![ns_fn, make_fn, final_assignment])
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_class_def.txt");
}

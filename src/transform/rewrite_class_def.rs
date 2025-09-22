use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::make_tuple;
use crate::transform::driver::{ExprRewriter, Rewrite};
use crate::transform::rewrite_decorator;
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
                annotation,
                value,
                ..
            }) => {
                if let Some(value) = value.as_mut() {
                    self.visit_expr(value);
                    self.visit_expr(target);

                    if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                        let name = id.as_str();
                        if self.loads.contains(name) {
                            self.captured_names.insert(name.to_string());
                        }
                    }
                } else {
                    self.visit_expr(target);
                }

                self.visit_expr(annotation);
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
    class_expr: String,
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
        match expr {
            Expr::Call(call) => {
                let is_zero_arg_super =
                    if let Expr::Name(ast::ExprName { id, .. }) = call.func.as_ref() {
                        id == "super"
                            && call.arguments.args.is_empty()
                            && call.arguments.keywords.is_empty()
                    } else {
                        false
                    };

                if is_zero_arg_super {
                    walk_expr(self, expr);

                    let replacement = match &self.first_arg {
                        Some(arg) => py_expr!(
                            "super({cls:id}, {arg:id})",
                            cls = self.class_expr.as_str(),
                            arg = arg.as_str()
                        ),
                        None => py_expr!(
                            "super({cls:id}, None)",
                            cls = self.class_expr.as_str()
                        ),
                    };

                    *expr = replacement;
                } else {
                    walk_expr(self, expr);
                }
                return;
            }
            Expr::Name(ast::ExprName { id, ctx, .. }) => {
                if id == "__class__" && matches!(ctx, ExprContext::Load) {
                    *expr = py_expr!("{cls:id}", cls = self.class_expr.as_str());
                }
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
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
        class_expr: class_name.to_string(),
        first_arg,
    };
    for stmt in &mut func_def.body {
        walk_stmt(&mut transformer, stmt);
    }
}

fn extend_body_prepared(ns_body: &mut Vec<Stmt>, name: &str, value: Expr) {
    let assign_stmt = py_stmt!(
        r#"
{name:id} = {value:expr}
_dp_temp_ns[{name:literal}] = _dp_prepare_ns[{name:literal}] = {name:id}
"#,
        name = name,
        value = value,
    );
    ns_body.extend(assign_stmt);
}

fn extend_body_with_value(
    rewriter: &mut ExprRewriter,
    ns_body: &mut Vec<Stmt>,
    name: &str,
    value: Expr,
) {
    let (stmts, value_expr) = rewriter.maybe_placeholder_within(value);
    ns_body.extend(stmts);
    extend_body_prepared(ns_body, name, value_expr);
}

pub fn rewrite(
    ast::StmtClassDef {
        name,
        mut body,
        arguments,
        ..
    }: ast::StmtClassDef,
    decorators: Vec<ast::Decorator>,
    rewriter: &mut ExprRewriter,
) -> Rewrite {
    let class_name = name.id.as_str().to_string();

    let LoadBeforeStoreResult { captured_names, .. } =
        LoadBeforeStoreCollector::collect_from_body(&mut body);

    // Build namespace function body
    // TODO: correctly calculate the qualname of the class when nested
    let mut ns_body = Vec::new();

    let mut original_body = body;
    if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = original_body.first() {
        if let Expr::StringLiteral(_) = value.as_ref() {
            extend_body_prepared(&mut ns_body, "__doc__", *value.clone());
            original_body.remove(0);
        }
    }
    ns_body.extend(py_stmt!(
        r#"
_dp_class_annotations = _dp_temp_ns.get("__annotations__")
if _dp_class_annotations is None:
    _dp_class_annotations = __dp__.dict()
"#
    ));

    let mut has_class_annotations = false;
    let mut method_helpers: Vec<Stmt> = Vec::new();

    for stmt in original_body {
        match stmt {
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        let name = id.as_str().to_string();
                        extend_body_with_value(rewriter, &mut ns_body, name.as_str(), *value);
                    }
                } else {
                    let (mut stmts, shared_value) = rewriter.maybe_placeholder_within(*value);
                    ns_body.append(&mut stmts);
                    for target in targets {
                        if let Expr::Name(ast::ExprName { id, .. }) = target {
                            extend_body_prepared(&mut ns_body, id.as_str(), shared_value.clone());
                        }
                    }
                }
            }
            Stmt::AnnAssign(mut ann_assign) => {
                if ann_assign.simple {
                    if let Expr::Name(ast::ExprName { id, .. }) = ann_assign.target.as_ref() {
                        let name = id.as_str().to_string();

                        if let Some(value) = ann_assign.value.take() {
                            extend_body_with_value(rewriter, &mut ns_body, name.as_str(), *value);
                        }

                        if !has_class_annotations {
                            ns_body.extend(py_stmt!(
                                r#"
if _dp_temp_ns.get("__annotations__") is None:
    __dp__.setitem(_dp_temp_ns, "__annotations__", _dp_class_annotations)
__dp__.setitem(_dp_prepare_ns, "__annotations__", _dp_class_annotations)
"#
                            ));
                            has_class_annotations = true;
                        }

                        let (ann_stmts, annotation_expr) =
                            rewriter.maybe_placeholder_within(*ann_assign.annotation);
                        ns_body.extend(ann_stmts);

                        ns_body.extend(py_stmt!(
                            "_dp_class_annotations[{name:literal}] = {annotation:expr}",
                            name = name.as_str(),
                            annotation = annotation_expr,
                        ));

                        continue;
                    }
                }

                ns_body.push(Stmt::AnnAssign(ann_assign));
            }
            Stmt::FunctionDef(mut func_def) => {
                rewrite_method(&mut func_def, &class_name);
                let fn_name = func_def.name.id.to_string();
                let helper_name = format!("_dp_meth_{}_{}", class_name, fn_name);

                let decorators = take(&mut func_def.decorator_list);

                func_def.name.id = helper_name.clone().into();

                method_helpers.push(Stmt::FunctionDef(func_def));

                let mut method_stmts = Vec::new();

                method_stmts.extend(py_stmt!(
                    r#"
__dp__.setattr({helper_name:id}, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), {suffix:literal}))
{fn_name:id} = {helper_name:id}
"#,
                    helper_name = helper_name.as_str(),
                    fn_name = fn_name.as_str(),
                    suffix = format!(".{}", fn_name),
                ));

                let method_stmts = rewrite_decorator::rewrite(
                    decorators,
                    fn_name.as_str(),
                    method_stmts,
                    rewriter.context(),
                )
                .into_statements();

                ns_body.extend(method_stmts);
                ns_body.extend(py_stmt!(
                    r#"
_dp_temp_ns[{fn_name:literal}] = _dp_prepare_ns[{fn_name:literal}] = {fn_name:id}
"#,
                    fn_name = fn_name.as_str(),
                ));
            }
            Stmt::ClassDef(mut class_def) => {
                let nested_name = class_def.name.id.to_string();
                let decorators = take(&mut class_def.decorator_list);

                let nested_stmts = rewrite(class_def, decorators, rewriter).into_statements();
                ns_body.extend(nested_stmts);
                ns_body.extend(py_stmt!(
                    "_dp_temp_ns[{name:literal}] = _dp_prepare_ns[{name:literal}] = {name:id}",
                    name = nested_name.as_str(),
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

    let mut ns_fn = py_stmt!(
        r#"
def _dp_ns_{class_name:id}(_dp_prepare_ns):
    _dp_temp_ns = {}
    _dp_temp_ns["__module__"] = _dp_prepare_ns["__module__"] = __name__
    _dp_temp_ns["__qualname__"] = _dp_prepare_ns["__qualname__"] = {class_name:literal}

    {ns_body:stmt}
"#,
        class_name = class_name.as_str(),
        ns_body = ns_body,
    );

    if let Stmt::FunctionDef(ast::StmtFunctionDef { parameters, .. }) = &mut ns_fn[0] {
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

    ns_fn.extend(py_stmt!(
        r#"
def _dp_make_class_{class_name:id}():
    orig_bases = {bases:expr}
    bases = __dp__.resolve_bases(orig_bases)
    meta, ns, kwds = __dp__.prepare_class({class_name:literal}, bases, {prepare_dict:expr})
    _dp_ns_{class_name:id}(ns)
    if orig_bases is not bases and "__orig_bases__" not in ns:
        ns["__orig_bases__"] = orig_bases
    return meta({class_name:literal}, bases, ns, **kwds)

_dp_class_{class_name:id} = _dp_make_class_{class_name:id}()
{class_name:id} = _dp_class_{class_name:id}
"#,
        class_name = class_name.as_str(),
        bases = make_tuple(bases),
        prepare_dict = prepare_dict,
    ));

    let mut result = method_helpers;
    result.extend(
        rewrite_decorator::rewrite(decorators, class_name.as_str(), ns_fn, rewriter.context())
            .into_statements(),
    );

    Rewrite::Visit(result)
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_without_first_parameter_for_super() {
        assert_transform_eq(
            r#"
class C:
    def m():
        return super().m()
"#,
            r#"
def _dp_meth_C_m():
    return super(C, None).m()
def _dp_ns_C(_dp_prepare_ns):
    _dp_temp_ns = __dp__.dict()
    __dp__.setitem(_dp_temp_ns, "__module__", __name__)
    __dp__.setitem(_dp_prepare_ns, "__module__", __name__)
    _dp_tmp_1 = "C"
    __dp__.setitem(_dp_temp_ns, "__qualname__", _dp_tmp_1)
    __dp__.setitem(_dp_prepare_ns, "__qualname__", _dp_tmp_1)
    _dp_class_annotations = _dp_temp_ns.get("__annotations__")
    _dp_tmp_2 = __dp__.is_(_dp_class_annotations, None)
    if _dp_tmp_2:
        _dp_class_annotations = __dp__.dict()

    __dp__.setattr(_dp_meth_C_m, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".m"))
    m = _dp_meth_C_m
    __dp__.setitem(_dp_temp_ns, "m", m)
    __dp__.setitem(_dp_prepare_ns, "m", m)
def _dp_make_class_C():
    orig_bases = ()
    bases = __dp__.resolve_bases(orig_bases)
    _dp_tmp_3 = __dp__.prepare_class("C", bases, None)
    meta = __dp__.getitem(_dp_tmp_3, 0)
    ns = __dp__.getitem(_dp_tmp_3, 1)
    kwds = __dp__.getitem(_dp_tmp_3, 2)
    _dp_ns_C(ns)
    _dp_tmp_5 = __dp__.is_not(orig_bases, bases)
    _dp_tmp_4 = _dp_tmp_5
    if _dp_tmp_4:
        _dp_tmp_6 = __dp__.not_(__dp__.contains(ns, "__orig_bases__"))
        _dp_tmp_4 = _dp_tmp_6
    if _dp_tmp_4:
        __dp__.setitem(ns, "__orig_bases__", orig_bases)
    return meta("C", bases, ns, **kwds)
_dp_class_C = _dp_make_class_C()
C = _dp_class_C
"#,
        );
    }

    crate::transform_fixture_test!("tests_rewrite_class_def.txt");
}

use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::make_tuple;
use crate::transform::context::Context;
use crate::transform::driver::{ExprRewriter, Rewrite};
use crate::transform::rewrite_decorator;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};
use ruff_text_size::TextRange;
use std::collections::HashMap;
use std::mem::take;

struct ClassVarRenamer<'a> {
    ctx: &'a Context,
    replacements: HashMap<String, String>,
}

impl<'a> ClassVarRenamer<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            replacements: HashMap::new(),
        }
    }

    fn into_replacements(self) -> HashMap<String, String> {
        self.replacements
    }

    fn replacement_for(&mut self, name: &str) -> String {
        if let Some(existing) = self.replacements.get(name) {
            existing.clone()
        } else {
            let replacement = self.ctx.fresh(&format!("var_{name}"));
            self.replacements
                .insert(name.to_string(), replacement.clone());
            replacement
        }
    }
}

impl Transformer for ClassVarRenamer<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef {
                name,
                decorator_list,
                parameters,
                returns,
                type_params,
                ..
            }) => {
                let original_name = name.id.as_str().to_string();
                let replacement = self.replacement_for(original_name.as_str());
                name.id = replacement.into();

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
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            let name = id.as_str().to_string();
            match ctx {
                ExprContext::Store => {
                    let replacement = self.replacement_for(name.as_str());
                    *id = replacement.into();
                }
                ExprContext::Load | ExprContext::Del => {
                    if let Some(replacement) = self.replacements.get(name.as_str()) {
                        *id = replacement.clone().into();
                    }
                }
                _ => {}
            }
            return;
        }
        walk_expr(self, expr);
    }
}

fn lookup_original_name(mapping: &HashMap<String, String>, replacement: &str) -> String {
    mapping
        .get(replacement)
        .cloned()
        .unwrap_or_else(|| replacement.to_string())
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
                        None => py_expr!("super({cls:id}, None)", cls = self.class_expr.as_str()),
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

fn extend_body_assignment(
    ns_body: &mut Vec<Stmt>,
    ns_entries: &mut Vec<(String, Expr)>,
    original_name: &str,
    replacement_name: &str,
    value: Expr,
) {
    let assign_stmt = py_stmt!(
        "{replacement_name:id} = {value:expr}",
        replacement_name = replacement_name,
        value = value,
    );
    ns_body.extend(assign_stmt);
    ns_entries.push((
        original_name.to_string(),
        py_expr!("{replacement_name:id}", replacement_name = replacement_name),
    ));
}

fn extend_body_with_value(
    rewriter: &mut ExprRewriter,
    ns_body: &mut Vec<Stmt>,
    ns_entries: &mut Vec<(String, Expr)>,
    original_name: &str,
    replacement_name: &str,
    value: Expr,
) {
    let (stmts, value_expr) = rewriter.maybe_placeholder_within(value);
    ns_body.extend(stmts);
    extend_body_assignment(
        ns_body,
        ns_entries,
        original_name,
        replacement_name,
        value_expr,
    );
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

    let mut renamer = ClassVarRenamer::new(rewriter.context());
    renamer.visit_body(&mut body);
    let replacements = renamer.into_replacements();
    let mut replacement_to_original: HashMap<String, String> = HashMap::new();
    for (original, replacement) in replacements.iter() {
        replacement_to_original.insert(replacement.clone(), original.clone());
    }

    // Build namespace function body
    // TODO: correctly calculate the qualname of the class when nested
    let mut ns_body = Vec::new();
    let mut ns_entries: Vec<(String, Expr)> = Vec::new();

    ns_entries.push(("__module__".to_string(), py_expr!("__name__")));
    ns_entries.push((
        "__qualname__".to_string(),
        py_expr!("{class_name:literal}", class_name = class_name.as_str()),
    ));

    let mut original_body = body;
    if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = original_body.first() {
        if let Expr::StringLiteral(_) = value.as_ref() {
            extend_body_assignment(
                &mut ns_body,
                &mut ns_entries,
                "__doc__",
                "__doc__",
                *value.clone(),
            );
            original_body.remove(0);
        }
    }
    ns_body.extend(py_stmt!(
        r#"
_dp_class_annotations = _dp_prepare_ns.get("__annotations__")
if _dp_class_annotations is None:
    _dp_class_annotations = __dp__.dict()
"#
    ));

    let mut has_class_annotations = false;

    for stmt in original_body {
        match stmt {
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        let replacement_name = id.as_str().to_string();
                        let original_name = lookup_original_name(
                            &replacement_to_original,
                            replacement_name.as_str(),
                        );
                        extend_body_with_value(
                            rewriter,
                            &mut ns_body,
                            &mut ns_entries,
                            original_name.as_str(),
                            replacement_name.as_str(),
                            *value,
                        );
                    }
                } else {
                    let (mut stmts, shared_value) = rewriter.maybe_placeholder_within(*value);
                    ns_body.append(&mut stmts);
                    for target in targets {
                        if let Expr::Name(ast::ExprName { id, .. }) = target {
                            let replacement_name = id.as_str().to_string();
                            let original_name = lookup_original_name(
                                &replacement_to_original,
                                replacement_name.as_str(),
                            );
                            extend_body_assignment(
                                &mut ns_body,
                                &mut ns_entries,
                                original_name.as_str(),
                                replacement_name.as_str(),
                                shared_value.clone(),
                            );
                        }
                    }
                }
            }
            Stmt::AnnAssign(mut ann_assign) => {
                if ann_assign.simple {
                    if let Expr::Name(ast::ExprName { id, .. }) = ann_assign.target.as_ref() {
                        let replacement_name = id.as_str().to_string();
                        let original_name = lookup_original_name(
                            &replacement_to_original,
                            replacement_name.as_str(),
                        );

                        if let Some(value) = ann_assign.value.take() {
                            extend_body_with_value(
                                rewriter,
                                &mut ns_body,
                                &mut ns_entries,
                                original_name.as_str(),
                                replacement_name.as_str(),
                                *value,
                            );
                        }

                        if !has_class_annotations {
                            ns_entries.push((
                                "__annotations__".to_string(),
                                py_expr!("_dp_class_annotations"),
                            ));
                            has_class_annotations = true;
                        }

                        let (ann_stmts, annotation_expr) =
                            rewriter.maybe_placeholder_within(*ann_assign.annotation);
                        ns_body.extend(ann_stmts);

                        ns_body.extend(py_stmt!(
                            "_dp_class_annotations[{name:literal}] = {annotation:expr}",
                            name = original_name.as_str(),
                            annotation = annotation_expr,
                        ));

                        continue;
                    }
                }

                ns_body.push(Stmt::AnnAssign(ann_assign));
            }
            Stmt::FunctionDef(mut func_def) => {
                let fn_name = func_def.name.id.to_string();
                let original_fn_name =
                    lookup_original_name(&replacement_to_original, fn_name.as_str());

                rewrite_method(&mut func_def, &class_name);

                let decorators = take(&mut func_def.decorator_list);

                let mut method_stmts = Vec::new();
                method_stmts.push(Stmt::FunctionDef(func_def));

                let method_stmts = rewrite_decorator::rewrite(
                    decorators,
                    fn_name.as_str(),
                    method_stmts,
                    rewriter.context(),
                )
                .into_statements();

                ns_body.extend(method_stmts);
                ns_entries.push((
                    original_fn_name,
                    py_expr!("{fn_name:id}", fn_name = fn_name.as_str()),
                ));
            }
            Stmt::ClassDef(mut class_def) => {
                let nested_name = class_def.name.id.to_string();
                let decorators = take(&mut class_def.decorator_list);

                let nested_stmts = rewrite(class_def, decorators, rewriter).into_statements();
                ns_body.extend(nested_stmts);
                let nested_expr = py_expr!("{name:id}", name = nested_name.as_str());
                ns_entries.push((nested_name, nested_expr));
            }
            other => ns_body.push(other),
        }
    }

    let entry_exprs: Vec<Expr> = ns_entries
        .into_iter()
        .map(|(name, value)| {
            make_tuple(vec![
                py_expr!("{name:literal}", name = name.as_str()),
                value,
            ])
        })
        .collect();
    let entries_list = py_expr!(
        "__dp__.list({entries:expr})",
        entries = make_tuple(entry_exprs),
    );
    ns_body.extend(py_stmt!("return {entries:expr}", entries = entries_list));

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
    {ns_body:stmt}
"#,
        class_name = class_name.as_str(),
        ns_body = ns_body,
    );

    if !matches!(&ns_fn[0], Stmt::FunctionDef(_)) {
        unreachable!("expected function definition for namespace helper");
    }

    ns_fn.extend(py_stmt!(
        r#"
def _dp_make_class_{class_name:id}():
    orig_bases = {bases:expr}
    bases = __dp__.resolve_bases(orig_bases)
    meta, ns, kwds = __dp__.prepare_class({class_name:literal}, bases, {prepare_dict:expr})
    _dp_namespace_entries = _dp_ns_{class_name:id}(ns)
    _dp_temp_ns = __dp__.dict()
    for _dp_name, _dp_value in _dp_namespace_entries:
        __dp__.setitem(_dp_temp_ns, _dp_name, _dp_value)
        __dp__.setitem(ns, _dp_name, _dp_value)
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

    let mut result = Vec::new();
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
def _dp_ns_C(_dp_prepare_ns):
    _dp_class_annotations = _dp_prepare_ns.get("__annotations__")
    _dp_tmp_2 = __dp__.is_(_dp_class_annotations, None)
    if _dp_tmp_2:
        _dp_class_annotations = __dp__.dict()

    def _dp_var_m_1():
        return super(C, None).m()
    return __dp__.list((("__module__", __name__), ("__qualname__", "C"), ("m", _dp_var_m_1)))
def _dp_make_class_C():
    orig_bases = ()
    bases = __dp__.resolve_bases(orig_bases)
    _dp_tmp_3 = __dp__.prepare_class("C", bases, None)
    meta = __dp__.getitem(_dp_tmp_3, 0)
    ns = __dp__.getitem(_dp_tmp_3, 1)
    kwds = __dp__.getitem(_dp_tmp_3, 2)
    _dp_namespace_entries = _dp_ns_C(ns)
    _dp_temp_ns = __dp__.dict()
    _dp_iter_4 = __dp__.iter(_dp_namespace_entries)
    while True:
        try:
            _dp_tmp_5 = __dp__.next(_dp_iter_4)
            _dp_name = __dp__.getitem(_dp_tmp_5, 0)
            _dp_value = __dp__.getitem(_dp_tmp_5, 1)
        except:
            __dp__.check_stopiteration()
            break
        else:
            __dp__.setitem(_dp_temp_ns, _dp_name, _dp_value)
            __dp__.setitem(ns, _dp_name, _dp_value)
    _dp_tmp_7 = __dp__.is_not(orig_bases, bases)
    _dp_tmp_6 = _dp_tmp_7
    if _dp_tmp_6:
        _dp_tmp_8 = __dp__.not_(__dp__.contains(ns, "__orig_bases__"))
        _dp_tmp_6 = _dp_tmp_8
    if _dp_tmp_6:
        __dp__.setitem(ns, "__orig_bases__", orig_bases)
    return meta("C", bases, ns, **kwds)
_dp_class_C = _dp_make_class_C()
C = _dp_class_C
"#,
        );
    }

    crate::transform_fixture_test!("tests_rewrite_class_def.txt");
}

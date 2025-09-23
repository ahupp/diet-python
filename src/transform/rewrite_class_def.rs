use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::{make_tuple, py_stmt_single};
use crate::transform::class_def::AnnotationCollector;
use crate::transform::context::Context;
use crate::transform::driver::{ExprRewriter, Rewrite};
use crate::transform::rewrite_decorator;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};
use ruff_text_size::TextRange;
use std::collections::{HashMap, VecDeque};
use std::mem::take;

fn class_ident_from_qualname(qualname: &str) -> String {
    format!("_dp_class_{}", qualname.replace('.', "_"))
}

struct NestedClassCollector {
    class_qualname: String,
    nested: Vec<(String, ast::StmtClassDef)>,
}

impl NestedClassCollector {
    fn new(class_qualname: String) -> Self {
        Self {
            class_qualname,
            nested: Vec::new(),
        }
    }

    fn into_nested(self) -> Vec<(String, ast::StmtClassDef)> {
        self.nested
    }
}

impl Transformer for NestedClassCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::ClassDef(class_def) = stmt {
            let nested_name = class_def.name.id.to_string();
            let nested_qualname = format!("{}.{}", self.class_qualname, nested_name);
            let dp_name = class_ident_from_qualname(&nested_qualname);
            let class_ident = dp_name
                .strip_prefix("_dp_class_")
                .expect("dp class names are prefixed")
                .to_string();

            let mut nested_def = class_def.clone();
            let decorators = take(&mut nested_def.decorator_list);

            let (bases_tuple, prepare_dict) = class_call_arguments(nested_def.arguments.clone());

            let mut value = py_expr!(
                "__dp__.create_class({class_name:literal}, _dp_ns_{class_ident:id}, {bases:expr}, {prepare_dict:expr})",
                class_name = nested_name.as_str(),
                class_ident = class_ident.as_str(),
                bases = bases_tuple,
                prepare_dict = prepare_dict,
            );
            for decorator in decorators.into_iter().rev() {
                value = py_expr!(
                    "({decorator:expr})({value:expr})",
                    decorator = decorator.expression,
                    value = value,
                );
            }

            self.nested.push((dp_name.clone(), nested_def));

            *stmt = py_stmt_single(py_stmt!(
                "{name:id} = {value:expr}",
                name = nested_name.as_str(),
                value = value,
            ));

            return;
        }

        walk_stmt(self, stmt);
    }
}

fn class_call_arguments(arguments: Option<Box<ast::Arguments>>) -> (Expr, Expr) {
    let mut bases = Vec::new();
    let mut kw_keys = Vec::new();
    let mut kw_vals = Vec::new();
    if let Some(args) = arguments {
        let args = *args;
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

    (make_tuple(bases), prepare_dict)
}

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

pub fn rewrite(
    ast::StmtClassDef {
        name,
        mut body,
        arguments,
        ..
    }: ast::StmtClassDef,
    decorators: Vec<ast::Decorator>,
    rewriter: &mut ExprRewriter,
    qualname: Option<String>,
) -> Rewrite {
    let class_name = name.id.as_str().to_string();
    let class_qualname = qualname.unwrap_or_else(|| class_name.clone());
    let dp_class_name = class_ident_from_qualname(&class_qualname);
    let class_ident = dp_class_name
        .strip_prefix("_dp_class_")
        .expect("dp class names are prefixed")
        .to_string();
    let has_decorators = !decorators.is_empty();

    let mut nested_collector = NestedClassCollector::new(class_qualname.clone());
    nested_collector.visit_body(&mut body);
    let nested_classes = nested_collector.into_nested();

    let mut renamer = ClassVarRenamer::new(rewriter.context());
    renamer.visit_body(&mut body);
    let annotations = AnnotationCollector::collect(&mut body);
    let replacements = renamer.into_replacements();
    let mut replacement_to_original: HashMap<String, String> = HashMap::new();
    for (original, replacement) in replacements.iter() {
        replacement_to_original.insert(replacement.clone(), original.clone());
    }

    // Build namespace function body
    let add_class_binding = |ns_body: &mut Vec<Stmt>, replacement_name: &str, value: Expr| {
        let original_name = lookup_original_name(&replacement_to_original, replacement_name);
        ns_body.extend(py_stmt!(
            "{replacement_name:id} = _dp_add_binding({name:literal}, {value:expr})",
            replacement_name = replacement_name,
            name = original_name.as_str(),
            value = value,
        ));
    };

    let mut ns_body = Vec::new();

    ns_body.extend(py_stmt!(
        "_dp_add_binding({name:literal}, {value:expr})",
        name = "__module__",
        value = py_expr!("__name__"),
    ));
    ns_body.extend(py_stmt!(
        "_dp_add_binding({name:literal}, {value:expr})",
        name = "__qualname__",
        value = py_expr!(
            "{class_qualname:literal}",
            class_qualname = class_qualname.as_str()
        ),
    ));

    let mut original_body = body;
    let mut annotations = VecDeque::from(annotations);
    if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = original_body.first() {
        if let Expr::StringLiteral(_) = value.as_ref() {
            add_class_binding(&mut ns_body, "__doc__", *value.clone());
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

    for (index, stmt) in original_body.into_iter().enumerate() {
        let has_annotation = annotations
            .front()
            .map(|(ann_index, _, _)| *ann_index == index)
            .unwrap_or(false);

        let skip_stmt = has_annotation && matches!(stmt, Stmt::Pass(_));

        if !skip_stmt {
            match stmt {
                Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                    if targets.len() == 1 {
                        if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                            let replacement_name = id.as_str().to_string();
                            let (stmts, value_expr) = rewriter.maybe_placeholder_within(*value);
                            ns_body.extend(stmts);
                            add_class_binding(&mut ns_body, replacement_name.as_str(), value_expr);
                        }
                    } else {
                        let (mut stmts, shared_value) = rewriter.maybe_placeholder_within(*value);
                        ns_body.append(&mut stmts);
                        for target in targets {
                            if let Expr::Name(ast::ExprName { id, .. }) = target {
                                let replacement_name = id.as_str().to_string();
                                add_class_binding(
                                    &mut ns_body,
                                    replacement_name.as_str(),
                                    shared_value.clone(),
                                );
                            }
                        }
                    }
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
                    ns_body.extend(py_stmt!(
                        "{fn_name:id} = _dp_add_binding({name:literal}, {value:expr})",
                        fn_name = fn_name.as_str(),
                        name = original_fn_name.as_str(),
                        value = py_expr!("{fn_name:id}", fn_name = fn_name.as_str()),
                    ));
                }
                Stmt::ClassDef(_) => {
                    unreachable!("nested classes should be collected before rewriting")
                }
                other => ns_body.push(other),
            }
        }

        while annotations
            .front()
            .map(|(ann_index, _, _)| *ann_index == index)
            .unwrap_or(false)
        {
            let (_, replacement_name, annotation) = annotations.pop_front().unwrap();
            let original_name =
                lookup_original_name(&replacement_to_original, replacement_name.as_str());

            if !has_class_annotations {
                ns_body.extend(py_stmt!(
                    "_dp_add_binding({name:literal}, _dp_class_annotations)",
                    name = "__annotations__",
                ));
                has_class_annotations = true;
            }

            let (ann_stmts, annotation_expr) = rewriter.maybe_placeholder_within(annotation);
            ns_body.extend(ann_stmts);

            ns_body.extend(py_stmt!(
                "_dp_class_annotations[{name:literal}] = {annotation:expr}",
                name = original_name.as_str(),
                annotation = annotation_expr,
            ));
        }
    }

    // Build class helper function
    let mut ns_fn = py_stmt!(
        r#"
def _dp_ns_{class_ident:id}(_dp_prepare_ns, _dp_add_binding):
    {ns_body:stmt}
"#,
        class_ident = class_ident.as_str(),
        ns_body = ns_body,
    );

    if !matches!(&ns_fn[0], Stmt::FunctionDef(_)) {
        unreachable!("expected function definition for namespace helper");
    }

    let (bases_tuple, prepare_dict) = class_call_arguments(arguments);
    let create_call = py_expr!(
        "__dp__.create_class({class_name:literal}, _dp_ns_{class_ident:id}, {bases:expr}, {prepare_dict:expr})",
        class_name = class_name.as_str(),
        class_ident = class_ident.as_str(),
        bases = bases_tuple.clone(),
        prepare_dict = prepare_dict.clone(),
    );

    let assign_to_class_name = class_qualname == class_name;
    if assign_to_class_name || has_decorators {
        ns_fn.extend(py_stmt!(
            "{dp_class_name:id} = {create_call:expr}",
            dp_class_name = dp_class_name.as_str(),
            create_call = create_call,
        ));
    }

    let mut result = Vec::new();

    for (dp_name, nested_class_def) in nested_classes {
        let nested_name = nested_class_def.name.id.to_string();
        let nested_qualname = format!("{class_qualname}.{nested_name}");
        let nested_dp_name = class_ident_from_qualname(&nested_qualname);
        debug_assert_eq!(nested_dp_name, dp_name);

        result.extend(
            rewrite(
                nested_class_def,
                Vec::new(),
                rewriter,
                Some(nested_qualname),
            )
            .into_statements(),
        );
    }

    result.extend(
        rewrite_decorator::rewrite(
            decorators,
            dp_class_name.as_str(),
            ns_fn,
            rewriter.context(),
        )
        .into_statements(),
    );

    if assign_to_class_name {
        result.extend(py_stmt!(
            "{class_name:id} = {dp_class_name:id}",
            class_name = class_name.as_str(),
            dp_class_name = dp_class_name.as_str(),
        ));
    }

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
def _dp_ns_C(_dp_prepare_ns, _dp_add_binding):
    _dp_add_binding("__module__", __name__)
    _dp_add_binding("__qualname__", "C")
    _dp_class_annotations = _dp_prepare_ns.get("__annotations__")
    _dp_tmp_2 = __dp__.is_(_dp_class_annotations, None)
    if _dp_tmp_2:
        _dp_class_annotations = __dp__.dict()

    def _dp_var_m_1():
        return super(C, None).m()
    _dp_var_m_1 = _dp_add_binding("m", _dp_var_m_1)
_dp_class_C = __dp__.create_class("C", _dp_ns_C, (), None)
C = _dp_class_C
"#,
        );
    }

    crate::transform_fixture_test!("tests_rewrite_class_def.txt");
}

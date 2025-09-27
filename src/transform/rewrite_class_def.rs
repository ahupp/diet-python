use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::{make_tuple, py_stmt_single};
use crate::transform::class_def::AnnotationCollector;
use crate::transform::driver::{ExprRewriter, Rewrite};
use crate::transform::rewrite_decorator;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};
use ruff_text_size::TextRange;
use std::collections::HashSet;
use std::mem::take;

fn class_ident_from_qualname(qualname: &str) -> String {
    let sanitized: String = qualname
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("_dp_class_{}", sanitized)
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

        if matches!(stmt, Stmt::FunctionDef(_)) {
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

struct ClassVarRenamer {
    stored: HashSet<String>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
    pending: HashSet<String>,
    assignment_targets: HashSet<String>,
}

impl ClassVarRenamer {
    fn new() -> Self {
        Self {
            stored: HashSet::new(),
            globals: HashSet::new(),
            nonlocals: HashSet::new(),
            pending: HashSet::new(),
            assignment_targets: HashSet::new(),
        }
    }

    fn namespace_subscript(name: &str, ctx: ExprContext) -> Expr {
        Expr::Subscript(ast::ExprSubscript {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            value: Box::new(py_expr!("_dp_ns")),
            slice: Box::new(py_expr!("{name:literal}", name = name)),
            ctx,
        })
    }

    fn should_rewrite(&self, name: &str) -> bool {
        !self.globals.contains(name) && !self.nonlocals.contains(name) && !name.starts_with("_dp_")
    }

    fn collect_target_names(expr: &Expr, names: &mut Vec<String>) {
        match expr {
            Expr::Name(ast::ExprName { id, .. }) => names.push(id.as_str().to_string()),
            Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
                for elt in elts {
                    Self::collect_target_names(elt, names);
                }
            }
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                Self::collect_target_names(value, names);
            }
            _ => {}
        }
    }

    fn visit_store_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, .. }) => {
                let name = id.as_str().to_string();
                if self.should_rewrite(name.as_str()) {
                    *expr = Self::namespace_subscript(name.as_str(), ExprContext::Store);
                    self.pending.remove(name.as_str());
                    self.stored.insert(name);
                }
            }
            Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
                for elt in elts {
                    self.visit_store_expr(elt);
                }
            }
            Expr::Starred(ast::ExprStarred { value, .. }) => self.visit_store_expr(value),
            Expr::Attribute(ast::ExprAttribute { value, .. }) => {
                self.visit_expr(value);
            }
            Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
                self.visit_expr(value);
                self.visit_expr(slice);
            }
            _ => self.visit_expr(expr),
        }
    }

    fn visit_delete_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, .. }) => {
                let name = id.as_str().to_string();
                if self.should_rewrite(name.as_str()) {
                    *expr = Self::namespace_subscript(name.as_str(), ExprContext::Del);
                }
            }
            Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
                for elt in elts {
                    self.visit_delete_expr(elt);
                }
            }
            Expr::Starred(ast::ExprStarred { value, .. }) => self.visit_delete_expr(value),
            Expr::Attribute(ast::ExprAttribute { value, .. }) => {
                self.visit_expr(value);
            }
            Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
                self.visit_expr(value);
                self.visit_expr(slice);
            }
            _ => self.visit_expr(expr),
        }
    }
}

impl Transformer for ClassVarRenamer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                let mut tracked_targets = Vec::new();
                for target in targets.iter() {
                    Self::collect_target_names(target, &mut tracked_targets);
                }
                for name in &tracked_targets {
                    self.assignment_targets.insert(name.clone());
                }
                self.visit_expr(value);
                for target in targets.iter_mut() {
                    self.visit_store_expr(target);
                }
                for name in tracked_targets {
                    self.assignment_targets.remove(&name);
                }
            }
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                annotation,
                value,
                ..
            }) => {
                let mut tracked_targets = Vec::new();
                Self::collect_target_names(target, &mut tracked_targets);
                for name in &tracked_targets {
                    self.assignment_targets.insert(name.clone());
                }
                if let Some(expr) = value {
                    self.visit_expr(expr);
                }
                self.visit_annotation(annotation);
                self.visit_store_expr(target);
                for name in tracked_targets {
                    self.assignment_targets.remove(&name);
                }
            }
            Stmt::AugAssign(ast::StmtAugAssign {
                target, op, value, ..
            }) => {
                self.visit_expr(value);
                self.visit_operator(op);
                self.visit_store_expr(target);
            }
            Stmt::FunctionDef(ast::StmtFunctionDef {
                name,
                decorator_list,
                parameters,
                returns,
                type_params,
                ..
            }) => {
                let original_name = name.id.as_str().to_string();
                if self.should_rewrite(original_name.as_str()) {
                    self.stored.insert(original_name.clone());
                    self.pending.insert(original_name);
                }

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
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                for name in names {
                    self.globals.insert(name.id.to_string());
                }
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    self.nonlocals.insert(name.id.to_string());
                }
            }
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                for target in targets {
                    self.visit_delete_expr(target);
                }
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            if let ExprContext::Load = ctx {
                let name = id.as_str().to_string();
                let name_str = name.as_str();
                if self.should_rewrite(name_str) {
                    if self.stored.contains(name_str) && !self.pending.contains(name_str) {
                        *expr = Self::namespace_subscript(name_str, ExprContext::Load);
                    } else if self.pending.contains(name_str)
                        && !self.assignment_targets.contains(name_str)
                    {
                        *expr =
                            py_expr!("__dp__.global_(globals(), {name:literal})", name = name_str,);
                    }
                }
            }
            return;
        }
        walk_expr(self, expr);
    }
}

struct MethodTransformer {
    class_expr: String,
    first_arg: Option<String>,
    method_name: String,
    local_bindings: HashSet<String>,
}

impl MethodTransformer {
    fn collect_store_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, .. }) => {
                self.local_bindings.insert(id.to_string());
            }
            Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
                for elt in elts {
                    self.collect_store_expr(elt);
                }
            }
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                self.collect_store_expr(value);
            }
            Expr::Attribute(ast::ExprAttribute { value, .. }) => {
                self.collect_store_expr(value);
            }
            Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
                self.collect_store_expr(value);
                self.collect_store_expr(slice);
            }
            _ => {}
        }
    }
}

impl Transformer for MethodTransformer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) => return,
            Stmt::Assign(ast::StmtAssign { targets, .. }) => {
                for target in targets {
                    self.collect_store_expr(target);
                }
            }
            Stmt::AnnAssign(ast::StmtAnnAssign { target, .. }) => {
                self.collect_store_expr(target);
            }
            Stmt::AugAssign(ast::StmtAugAssign { target, .. }) => {
                self.collect_store_expr(target);
            }
            Stmt::For(ast::StmtFor { target, .. }) => {
                self.collect_store_expr(target);
            }
            Stmt::With(ast::StmtWith { items, .. }) => {
                for item in items {
                    if let Some(optional_vars) = &item.optional_vars {
                        self.collect_store_expr(optional_vars);
                    }
                }
            }
            _ => {}
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
                if matches!(ctx, ExprContext::Load) {
                    if id == "__class__" {
                        *expr = py_expr!("{cls:id}", cls = self.class_expr.as_str());
                    } else if id.as_str() == self.method_name
                        && !self.local_bindings.contains(id.as_str())
                    {
                        *expr = py_expr!(
                            "__dp__.global_(globals(), {name:literal})",
                            name = self.method_name.as_str()
                        );
                    }
                }
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}

fn rewrite_method(
    func_def: &mut ast::StmtFunctionDef,
    class_name: &str,
    class_qualname: &str,
    original_method_name: &str,
    rewriter: &mut ExprRewriter,
) {
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

    let mut local_bindings: HashSet<String> = HashSet::new();
    for param in &func_def.parameters.posonlyargs {
        local_bindings.insert(param.name().to_string());
    }
    for param in &func_def.parameters.args {
        local_bindings.insert(param.name().to_string());
    }
    for param in &func_def.parameters.kwonlyargs {
        local_bindings.insert(param.name().to_string());
    }
    if let Some(param) = &func_def.parameters.vararg {
        local_bindings.insert(param.name.to_string());
    }
    if let Some(param) = &func_def.parameters.kwarg {
        local_bindings.insert(param.name.to_string());
    }

    let mut transformer = MethodTransformer {
        class_expr: class_name.to_string(),
        first_arg,
        method_name: original_method_name.to_string(),
        local_bindings,
    };
    for stmt in &mut func_def.body {
        walk_stmt(&mut transformer, stmt);
    }

    let method_qualname = format!("{class_qualname}.{original_method_name}");
    let body = take(&mut func_def.body);
    func_def.body =
        rewriter.with_function_scope(method_qualname, |rewriter| rewriter.rewrite_block(body));
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

    let annotations = AnnotationCollector::collect(&mut body);

    // Build namespace function body
    let add_class_binding = |ns_body: &mut Vec<Stmt>, binding_name: &str, value: Expr| {
        ns_body.extend(py_stmt!(
            "{binding_name:id} = {value:expr}",
            binding_name = binding_name,
            value = value,
        ));
    };

    let mut ns_body = Vec::new();

    ns_body.extend(py_stmt!(
        "{binding_name:id} = {value:expr}",
        binding_name = "__module__",
        value = py_expr!("__name__"),
    ));
    ns_body.extend(py_stmt!(
        "{binding_name:id} = {value:expr}",
        binding_name = "__qualname__",
        value = py_expr!(
            "{class_qualname:literal}",
            class_qualname = class_qualname.as_str()
        ),
    ));

    let mut original_body = body;
    let has_class_annotations = !annotations.is_empty();
    if let Some(first_stmt) = original_body.first_mut() {
        if let Stmt::Expr(ast::StmtExpr { value, .. }) = first_stmt {
            if let Expr::StringLiteral(_) = value.as_ref() {
                let doc_expr = (*value).clone();
                *first_stmt = py_stmt_single(py_stmt!("__doc__ = {value:expr}", value = doc_expr));
            }
        }
    }

    for stmt in original_body.into_iter() {
        match stmt {
            Stmt::Assign(assign) => {
                if assign.targets.len() == 1 {
                    if let Some(Expr::Name(ast::ExprName { id, .. })) = assign.targets.first() {
                        let binding_name = id.as_str().to_string();
                        let value = *assign.value;
                        let (stmts, value_expr) = rewriter.maybe_placeholder_within(value);
                        ns_body.extend(stmts);
                        add_class_binding(&mut ns_body, binding_name.as_str(), value_expr);
                        continue;
                    }
                } else if assign
                    .targets
                    .iter()
                    .all(|target| matches!(target, Expr::Name(_)))
                {
                    let value = *assign.value;
                    let (mut stmts, shared_value) = rewriter.maybe_placeholder_within(value);
                    ns_body.append(&mut stmts);
                    for target in assign.targets.into_iter() {
                        if let Expr::Name(ast::ExprName { id, .. }) = target {
                            let binding_name = id.as_str().to_string();
                            add_class_binding(
                                &mut ns_body,
                                binding_name.as_str(),
                                shared_value.clone(),
                            );
                        }
                    }
                    continue;
                }

                ns_body.push(Stmt::Assign(assign));
            }
            Stmt::FunctionDef(mut func_def) => {
                let fn_name = func_def.name.id.to_string();

                rewrite_method(
                    &mut func_def,
                    &class_name,
                    &class_qualname,
                    fn_name.as_str(),
                    rewriter,
                );

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
                    "{fn_name:id} = {value:expr}",
                    fn_name = fn_name.as_str(),
                    value = py_expr!("{fn_name:id}", fn_name = fn_name.as_str()),
                ));
            }
            Stmt::ClassDef(_) => {
                unreachable!("nested classes should be collected before rewriting")
            }
            other => ns_body.push(other),
        }
    }

    if has_class_annotations {
        ns_body.extend(py_stmt!(
            r#"
_dp_class_annotations = _dp_ns.get("__annotations__")
if _dp_class_annotations is None:
    _dp_class_annotations = __dp__.dict()
"#
        ));

        ns_body.extend(py_stmt!(
            "{binding_name:id} = _dp_class_annotations",
            binding_name = "__annotations__",
        ));

        for (_, name, annotation) in annotations {
            let (ann_stmts, annotation_expr) = rewriter.maybe_placeholder_within(annotation);
            ns_body.extend(ann_stmts);

            ns_body.extend(py_stmt!(
                "_dp_class_annotations[{name:literal}] = {annotation:expr}",
                name = name.as_str(),
                annotation = annotation_expr,
            ));
        }
    }

    let mut renamer = ClassVarRenamer::new();
    for stmt in &ns_body {
        if let Stmt::FunctionDef(func_def) = stmt {
            renamer
                .pending
                .insert(func_def.name.id.as_str().to_string());
        }
    }
    renamer.visit_body(&mut ns_body);

    // Build class helper function
    let (bases_tuple, prepare_dict) = class_call_arguments(arguments);
    let create_call = py_expr!(
        "__dp__.create_class({class_name:literal}, _dp_ns_{class_ident:id}, {bases:expr}, {prepare_dict:expr})",
        class_name = class_name.as_str(),
        class_ident = class_ident.as_str(),
        bases = bases_tuple.clone(),
        prepare_dict = prepare_dict.clone(),
    );

    let assign_to_class_name = class_qualname == class_name;
    let needs_class_binding = assign_to_class_name || class_qualname.contains("<locals>");
    let decorator_count = decorators.len();
    let ns_helper_name = format!("_dp_ns_{}", class_ident);
    let remove_class_helper = needs_class_binding || has_decorators;

    let mut class_statements = Vec::new();
    if needs_class_binding || has_decorators {
        class_statements.extend(py_stmt!(
            "{dp_class_name:id} = {create_call:expr}",
            dp_class_name = dp_class_name.as_str(),
            create_call = create_call,
        ));
    }

    let mut ns_fn = py_stmt!(
        r#"
def _dp_ns_{class_ident:id}(_dp_ns):
    {ns_body:stmt}
"#,
        class_ident = class_ident.as_str(),
        ns_body = ns_body,
    );

    let ns_fn_stmt = match ns_fn.pop() {
        Some(Stmt::FunctionDef(func_def)) => Stmt::FunctionDef(func_def),
        _ => unreachable!("expected function definition for namespace helper"),
    };

    let mut decorated_statements = rewrite_decorator::rewrite(
        decorators,
        dp_class_name.as_str(),
        class_statements,
        rewriter.context(),
    )
    .into_statements();

    decorated_statements.insert(decorator_count, ns_fn_stmt);

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

    result.extend(decorated_statements);

    if needs_class_binding {
        result.extend(py_stmt!(
            "{class_name:id} = {dp_class_name:id}",
            class_name = class_name.as_str(),
            dp_class_name = dp_class_name.as_str(),
        ));
    }

    if remove_class_helper {
        result.extend(py_stmt!(
            "del {dp_class_name:id}",
            dp_class_name = dp_class_name.as_str(),
        ));
    }

    if needs_class_binding {
        result.extend(py_stmt!(
            "del {ns_helper:id}",
            ns_helper = ns_helper_name.as_str(),
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
def _dp_ns_C(_dp_ns):
    __dp__.setitem(_dp_ns, "__module__", __name__)
    __dp__.setitem(_dp_ns, "__qualname__", "C")

    def m():
        return super(C, None).m()
    __dp__.setitem(_dp_ns, "m", m)
_dp_class_C = __dp__.create_class("C", _dp_ns_C, (), None)
C = _dp_class_C
del _dp_class_C
del _dp_ns_C
"#,
        );
    }

    crate::transform_fixture_test!("tests_rewrite_class_def.txt");
}

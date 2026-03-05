use super::{collect_bound_names, collect_parameter_names, make_dp_tuple};
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;
use std::collections::HashSet;

pub(super) fn is_annotation_helper_name(name: &str) -> bool {
    name.contains("__annotate_func__") || name.contains("__annotate__")
}

pub(super) fn should_keep_non_lowered_for_annotationlib(func: &ast::StmtFunctionDef) -> bool {
    // annotationlib.call_annotate_function rebuilds callables via FunctionType(..., fake_globals).
    // BB-lowered wrappers delegate into pre-bound block function objects, so fake globals do not
    // apply to the annotation expression evaluation. Keep likely annotate callables in regular
    // function form so fake-globals execution can observe transformed expressions directly.
    let params = func.parameters.as_ref();
    let Some(first) = params.posonlyargs.first() else {
        return false;
    };
    first.parameter.name.id.as_str() == "format"
}

pub(super) fn ensure_dp_default_param(func: &mut ast::StmtFunctionDef) {
    if function_has_global_or_nonlocal_dp(func) {
        return;
    }
    let mut existing_params = collect_parameter_names(func.parameters.as_ref())
        .into_iter()
        .collect::<HashSet<_>>();
    if !existing_params.contains("__dp__") {
        func.parameters
            .kwonlyargs
            .push(build_kwonly_capture_default("__dp__"));
        existing_params.insert("__dp__".to_string());
    }

    // annotationlib.call_annotate_function rebuilds callables with fake globals.
    // Capture direct helper globals (`__dp_*`) as kw-only defaults so helper
    // resolution stays stable without relying on global lookups.
    for helper in collect_used_dp_helpers(func) {
        if existing_params.contains(helper.as_str()) {
            continue;
        }
        func.parameters
            .kwonlyargs
            .push(build_kwonly_capture_default(helper.as_str()));
        existing_params.insert(helper);
    }
}

pub(super) fn rewrite_annotation_helper_defs_as_exec_calls(
    body: Vec<Box<Stmt>>,
    outer_scope_names: &HashSet<String>,
) -> Vec<Box<Stmt>> {
    body.into_iter()
        .map(|stmt| match stmt.as_ref() {
            Stmt::FunctionDef(func) if is_annotation_helper_name(func.name.id.as_str()) => {
                Box::new(annotation_helper_exec_binding_stmt(
                    func.clone(),
                    func.name.id.as_str(),
                    Some(outer_scope_names),
                ))
            }
            _ => stmt,
        })
        .collect()
}

pub(super) fn annotation_helper_exec_binding_stmt(
    func: ast::StmtFunctionDef,
    bind_name: &str,
    outer_scope_names: Option<&HashSet<String>>,
) -> Stmt {
    let mut helper_fn = func;
    ensure_dp_default_param(&mut helper_fn);
    let capture_names = collect_capture_names(&helper_fn, outer_scope_names);
    ensure_capture_default_params(&mut helper_fn, &capture_names);
    let source = render_stmt_source(&Stmt::FunctionDef(helper_fn));
    let captures = make_dp_tuple(
        capture_names
            .iter()
            .map(|name| {
                py_expr!(
                    "({name:literal}, {value:id})",
                    name = name.as_str(),
                    value = name.as_str(),
                )
            })
            .collect(),
    );
    // TODO: Avoid source-string re-exec here by representing annotation helpers
    // as first-class BB/IR defs with explicit lexical captures that still satisfy
    // annotationlib fake-globals FunctionType semantics.
    py_stmt!(
        "{bind:id} = __dp_exec_function_def_source({source:literal}, __dp_globals(), {captures:expr}, {name:literal})",
        bind = bind_name,
        source = source.as_str(),
        captures = captures,
        name = bind_name,
    )
}

pub(super) fn ensure_capture_default_params(func: &mut ast::StmtFunctionDef, capture_names: &[String]) {
    let mut existing = collect_parameter_names(func.parameters.as_ref())
        .into_iter()
        .collect::<HashSet<_>>();
    for capture in capture_names {
        if existing.contains(capture.as_str()) {
            continue;
        }
        func.parameters
            .kwonlyargs
            .push(build_kwonly_capture_default(capture.as_str()));
        existing.insert(capture.clone());
    }
}

#[derive(Default)]
struct InternalCaptureCollector {
    names: HashSet<String>,
}

impl Transformer for InternalCaptureCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            if matches!(ctx, ExprContext::Load) {
                self.names.insert(id.to_string());
            }
        }
        walk_expr(self, expr);
    }
}

pub(super) fn collect_capture_names(
    func: &ast::StmtFunctionDef,
    outer_scope_names: Option<&HashSet<String>>,
) -> Vec<String> {
    let mut body = func.body.clone();
    let mut collector = InternalCaptureCollector::default();
    collector.visit_body(&mut body);
    let params = collect_parameter_names(func.parameters.as_ref())
        .into_iter()
        .collect::<HashSet<_>>();
    let bound = collect_bound_names(&func.body.body);
    let mut names = collector
        .names
        .into_iter()
        .filter(|name| !name.starts_with("__dp_"))
        .filter(|name| !params.contains(name.as_str()))
        .filter(|name| !bound.contains(name.as_str()))
        .filter(|name| !looks_like_generated_dp_temp(name.as_str()))
        .collect::<Vec<_>>();
    if let Some(outer_scope) = outer_scope_names {
        names.retain(|name| outer_scope.contains(name.as_str()));
    } else {
        names.retain(|name| is_internal_capture_name(name.as_str()));
    }
    names.sort();
    names
}

fn is_internal_capture_name(name: &str) -> bool {
    ((name.starts_with("_dp_") && !name.starts_with("__dp_")) || name == "__class__")
        && !looks_like_generated_dp_temp(name)
}

fn looks_like_generated_dp_temp(name: &str) -> bool {
    if !name.starts_with("_dp_") {
        return false;
    }
    let Some((_, suffix)) = name.rsplit_once('_') else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
}

pub(super) fn render_stmt_source(stmt: &Stmt) -> String {
    Generator::new(&Indentation::new("    ".to_string()), LineEnding::default()).stmt(stmt)
}

fn build_kwonly_capture_default(name: &str) -> ast::ParameterWithDefault {
    let template = py_stmt!(
        r#"
def _dp_template(*, {name:id}={name:id}):
    pass
"#,
        name = name,
    );
    match template {
        Stmt::FunctionDef(template_fn) => template_fn
            .parameters
            .kwonlyargs
            .first()
            .cloned()
            .expect("template kwonly param missing"),
        _ => unreachable!("template did not parse as function"),
    }
}

#[derive(Default)]
struct DpHelperCollector {
    names: HashSet<String>,
}

impl Transformer for DpHelperCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            if matches!(ctx, ExprContext::Load) {
                let name = id.as_str();
                if name.starts_with("__dp_") && name != "__dp__" {
                    self.names.insert(name.to_string());
                }
            }
        }
        walk_expr(self, expr);
    }
}

fn collect_used_dp_helpers(func: &ast::StmtFunctionDef) -> Vec<String> {
    let mut body = func.body.clone();
    let mut collector = DpHelperCollector::default();
    collector.visit_body(&mut body);
    let mut names = collector.names.into_iter().collect::<Vec<_>>();
    names.sort();
    names
}

fn function_has_global_or_nonlocal_dp(func: &ast::StmtFunctionDef) -> bool {
    func.body.body.iter().any(|stmt| match stmt.as_ref() {
        Stmt::Global(global_stmt) => global_stmt
            .names
            .iter()
            .any(|name| name.id.as_str() == "__dp__"),
        Stmt::Nonlocal(nonlocal_stmt) => nonlocal_stmt
            .names
            .iter()
            .any(|name| name.id.as_str() == "__dp__"),
        _ => false,
    })
}

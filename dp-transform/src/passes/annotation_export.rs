use crate::block_py::state::{collect_parameter_names, sync_target_cells_stmts};
use crate::passes::ast_symbol_analysis::collect_bound_names;
use crate::passes::ast_to_ast::ast_rewrite::{rewrite_with_pass, Rewrite, StmtRewritePass};
use crate::passes::ast_to_ast::body::{suite_mut, suite_ref};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::expr_utils::{make_dp_tuple, name_expr};
use crate::passes::ast_to_ast::rewrite_stmt;
use crate::passes::ast_to_ast::scope_helpers::cell_name;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;
use std::collections::HashSet;

pub(crate) fn is_annotation_helper_name(name: &str) -> bool {
    name.contains("__annotate_func__") || name.contains("__annotate__")
}

pub(crate) fn should_keep_non_lowered_for_annotationlib(func: &ast::StmtFunctionDef) -> bool {
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

pub(crate) struct AnnotationHelperForLoweringPass;

impl StmtRewritePass for AnnotationHelperForLoweringPass {
    fn lower_stmt(&self, _context: &Context, stmt: Stmt) -> Rewrite {
        Rewrite::Unmodified(stmt)
    }
}

pub(crate) fn prepare_non_lowered_annotationlib_function(
    context: &Context,
    func: &mut ast::StmtFunctionDef,
) {
    if !should_keep_non_lowered_for_annotationlib(func) {
        return;
    }
    rewrite_with_pass(
        context,
        Some(&AnnotationHelperForLoweringPass),
        None,
        suite_mut(&mut func.body),
    );
    ensure_dp_default_param(func);
}

pub(crate) fn ensure_dp_default_param(func: &mut ast::StmtFunctionDef) {
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

pub(crate) fn rewrite_annotation_helper_defs_as_exec_calls(
    body: Vec<Stmt>,
    outer_scope_names: &HashSet<String>,
) -> Vec<Stmt> {
    body.into_iter()
        .map(|stmt| match stmt {
            Stmt::FunctionDef(func) if is_annotation_helper_name(func.name.id.as_str()) => {
                annotation_helper_exec_binding_stmt(
                    func.clone(),
                    func.name.id.as_str(),
                    Some(outer_scope_names),
                )
            }
            other => other,
        })
        .collect()
}

pub(crate) fn build_lowered_annotation_helper_binding(
    func: &ast::StmtFunctionDef,
    bind_name: &str,
) -> Option<(Stmt, Expr)> {
    let annotation_entries = function_annotation_entries(func);
    if annotation_entries.is_empty() {
        return None;
    }
    // Keep helper name in __annotate__ family so BB lowering keeps it in lexical scope.
    let annotate_helper_name = format!("_dp_fn___annotate___{bind_name}");
    let helper_stmt = rewrite_stmt::annotation::build_annotate_fn(
        annotation_entries,
        annotate_helper_name.as_str(),
    );
    let helper_stmt = match helper_stmt {
        Stmt::FunctionDef(helper_fn) => {
            annotation_helper_exec_binding_stmt(helper_fn, annotate_helper_name.as_str(), None)
        }
        other => other,
    };
    Some((helper_stmt, name_expr(annotate_helper_name.as_str())?))
}

pub(crate) fn annotation_helper_exec_binding_stmt(
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

pub(crate) fn build_exec_function_def_binding_stmts(
    func_def: &ast::StmtFunctionDef,
    cell_slots: &HashSet<String>,
    outer_scope_names: &HashSet<String>,
) -> Vec<Stmt> {
    let mut source_fn = func_def.clone();
    let bind_name = source_fn.name.id.to_string();
    ensure_dp_default_param(&mut source_fn);
    let capture_names = collect_capture_names(&source_fn, Some(outer_scope_names));
    ensure_capture_default_params(&mut source_fn, &capture_names);
    let source = render_stmt_source(&Stmt::FunctionDef(source_fn));
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
    let base_value = py_expr!(
        "__dp_exec_function_def_source({source:literal}, __dp_globals(), {captures:expr}, {name:literal})",
        source = source.as_str(),
        captures = captures,
        name = bind_name.as_str(),
    );
    let mut out = vec![py_stmt!(
        "{name:id} = {value:expr}",
        name = bind_name.as_str(),
        value = base_value,
    )];
    let target_expr = py_expr!("{name:id}", name = bind_name.as_str());
    out.extend(sync_target_cells_stmts(&target_expr, cell_slots));
    out
}

pub(crate) fn ensure_capture_default_params(
    func: &mut ast::StmtFunctionDef,
    capture_names: &[String],
) {
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

pub(crate) fn collect_capture_names(
    func: &ast::StmtFunctionDef,
    outer_scope_names: Option<&HashSet<String>>,
) -> Vec<String> {
    let mut body = func.body.clone();
    let mut collector = InternalCaptureCollector::default();
    collector.visit_body(suite_mut(&mut body));
    let params = collect_parameter_names(func.parameters.as_ref())
        .into_iter()
        .collect::<HashSet<_>>();
    let bound = collect_bound_names(suite_ref(&func.body));
    let mut names = collector
        .names
        .into_iter()
        .filter(|name| !name.starts_with("__dp_"))
        .filter(|name| !params.contains(name.as_str()))
        .filter(|name| !bound.contains(name.as_str()))
        .filter(|name| !looks_like_generated_dp_temp(name.as_str()))
        .collect::<Vec<_>>();
    if let Some(outer_scope) = outer_scope_names {
        names.retain(|name| {
            outer_scope.contains(name.as_str()) || outer_scope.contains(cell_name(name).as_str())
        });
    } else {
        names.retain(|name| is_internal_capture_name(name.as_str()));
    }
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

pub(crate) fn render_stmt_source(stmt: &Stmt) -> String {
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

pub(crate) fn function_annotation_entries(
    func: &ast::StmtFunctionDef,
) -> Vec<(String, Expr, String)> {
    let mut entries = Vec::new();
    let parameters = func.parameters.as_ref();

    for param in &parameters.posonlyargs {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    for param in &parameters.args {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(vararg) = &parameters.vararg {
        if let Some(annotation) = vararg.annotation.as_ref() {
            entries.push((
                vararg.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    for param in &parameters.kwonlyargs {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(kwarg) = &parameters.kwarg {
        if let Some(annotation) = kwarg.annotation.as_ref() {
            entries.push((
                kwarg.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(returns) = func.returns.as_ref() {
        entries.push((
            "return".to_string(),
            *returns.clone(),
            annotation_expr_string(returns),
        ));
    }

    entries
}

fn annotation_expr_string(expr: &Expr) -> String {
    Generator::new(&Indentation::new("    ".to_string()), LineEnding::default()).expr(expr)
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
    collector.visit_body(suite_mut(&mut body));
    let mut names = collector.names.into_iter().collect::<Vec<_>>();
    names.sort();
    names
}

fn function_has_global_or_nonlocal_dp(func: &ast::StmtFunctionDef) -> bool {
    suite_ref(&func.body).iter().any(|stmt| match stmt {
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

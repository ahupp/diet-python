use super::super::bb_ir::BbFunctionKind;
use super::super::ruff_to_blockpy::LoweredBlockPyFunction;
use super::dataflow::analyze_blockpy_use_def;
use super::state::collect_parameter_names;
use crate::py_expr;
use ruff_python_ast::Expr;
use ruff_python_parser::parse_expression;
use std::collections::HashSet;

pub(crate) fn build_make_function_expr_from_lowered(
    lowered: &LoweredBlockPyFunction,
    doc_expr: Option<Expr>,
    annotate_fn_expr: Option<Expr>,
) -> Option<Expr> {
    let entry_label = lowered.function.entry_label();
    let entry_ref_expr = py_expr!("{entry:literal}", entry = entry_label);
    let param_names: HashSet<String> = collect_parameter_names(&lowered.function.params)
        .into_iter()
        .collect();
    let generator_lifted_state_names: HashSet<&str> = lowered
        .closure_layout
        .as_ref()
        .map(|layout| {
            layout
                .cellvars
                .iter()
                .chain(layout.runtime_cells.iter())
                .map(|slot| slot.logical_name.as_str())
                .collect()
        })
        .unwrap_or_default();
    let generator_closure_storage_names: HashSet<&str> = lowered
        .closure_layout
        .as_ref()
        .map(|layout| {
            layout
                .freevars
                .iter()
                .chain(layout.cellvars.iter())
                .chain(layout.runtime_cells.iter())
                .map(|slot| slot.storage_name.as_str())
                .collect()
        })
        .unwrap_or_default();
    let locally_assigned: HashSet<String> = lowered
        .function
        .blocks
        .iter()
        .flat_map(|block| analyze_blockpy_use_def(block).1.into_iter())
        .collect();
    let mut closure_items = Vec::new();
    for entry_name in &lowered.function.entry_liveins {
        if param_names.contains(entry_name) {
            closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
        } else if entry_name == "_dp_classcell"
            || (entry_name.starts_with("_dp_cell_")
                && !lowered.function.local_cell_slots.contains(entry_name))
        {
            let value = name_expr(entry_name.as_str())?;
            closure_items.push(make_dp_tuple(vec![
                py_expr!("{value:literal}", value = entry_name.as_str()),
                value,
            ]));
        } else if matches!(
            &lowered.bb_kind,
            BbFunctionKind::Generator {
                closure_state: true,
                ..
            } | BbFunctionKind::AsyncGenerator {
                closure_state: true,
                ..
            }
        ) && generator_closure_storage_names.contains(entry_name.as_str())
        {
            let value = name_expr(entry_name.as_str())?;
            closure_items.push(make_dp_tuple(vec![
                py_expr!("{value:literal}", value = entry_name.as_str()),
                value,
            ]));
        } else if matches!(
            &lowered.bb_kind,
            BbFunctionKind::Generator {
                closure_state: true,
                ..
            } | BbFunctionKind::AsyncGenerator {
                closure_state: true,
                ..
            }
        ) && generator_lifted_state_names.contains(entry_name.as_str())
        {
            closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
        } else if !entry_name.starts_with("_dp_") && !locally_assigned.contains(entry_name) {
            let value = name_expr(entry_name.as_str())?;
            closure_items.push(make_dp_tuple(vec![
                py_expr!("{value:literal}", value = entry_name.as_str()),
                value,
            ]));
        } else {
            closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
        }
    }
    let closure = make_dp_tuple(closure_items);
    let doc = doc_expr.unwrap_or_else(|| py_expr!("None"));
    let annotate_fn = annotate_fn_expr.unwrap_or_else(|| py_expr!("None"));
    let function_entry_expr = py_expr!(
        "__dp_make_function({entry:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, {module_globals:expr}, {module_name:expr}, {doc:expr}, {annotate_fn:expr})",
        entry = entry_ref_expr.clone(),
        name = lowered.function.display_name.as_str(),
        qualname = lowered.function.qualname.as_str(),
        closure = closure.clone(),
        params = lowered.param_specs.to_expr(),
        module_globals = py_expr!("__dp_globals()"),
        module_name = py_expr!("__name__"),
        doc = doc.clone(),
        annotate_fn = annotate_fn.clone(),
    );
    match &lowered.bb_kind {
        BbFunctionKind::Function => {
            if lowered.is_coroutine {
                Some(py_expr!(
                    "__dp_mark_coroutine_function({func:expr})",
                    func = function_entry_expr,
                ))
            } else {
                Some(function_entry_expr)
            }
        }
        BbFunctionKind::AsyncGenerator { closure_state, .. } => {
            if *closure_state {
                return Some(function_entry_expr);
            }
            Some(py_expr!(
                "__dp_def_async_gen({resume:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                resume = entry_ref_expr.clone(),
                name = lowered.function.display_name.as_str(),
                qualname = lowered.function.qualname.as_str(),
                closure = closure,
                params = lowered.param_specs.to_expr(),
                doc = doc.clone(),
                annotate_fn = annotate_fn.clone(),
            ))
        }
        BbFunctionKind::Generator { closure_state, .. } => {
            if *closure_state {
                if lowered.is_coroutine {
                    return Some(py_expr!(
                        "__dp_mark_coroutine_function({func:expr})",
                        func = function_entry_expr,
                    ));
                }
                return Some(function_entry_expr);
            }
            if lowered.is_coroutine {
                Some(py_expr!(
                    "__dp_def_coro_from_gen({resume:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                    resume = entry_ref_expr,
                    name = lowered.function.display_name.as_str(),
                    qualname = lowered.function.qualname.as_str(),
                    closure = closure,
                    params = lowered.param_specs.to_expr(),
                    doc = doc,
                    annotate_fn = annotate_fn,
                ))
            } else {
                panic!(
                    "non-closure-backed sync generator lowering is unreachable; \
                     generated comprehension helpers are async-only"
                )
            }
        }
    }
}

pub(crate) fn make_dp_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
        panic!("expected call expression for __dp_tuple");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

pub(crate) fn name_expr(name: &str) -> Option<Expr> {
    parse_expression(name)
        .ok()
        .map(|expr| *expr.into_syntax().body)
}

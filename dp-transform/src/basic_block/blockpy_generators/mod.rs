use crate::basic_block::ast_to_ast::scope::cell_name;
use crate::basic_block::block_py::{ClosureInit, ClosureLayout, ClosureSlot};
use std::collections::HashSet;

fn is_generator_dispatch_param(name: &str) -> bool {
    matches!(
        name,
        "_dp_self" | "_dp_send_value" | "_dp_resume_exc" | "_dp_transport_sent"
    )
}

fn generator_storage_name(name: &str) -> String {
    if name == "_dp_classcell" || name.starts_with("_dp_cell_") {
        return name.to_string();
    }
    cell_name(name)
}

fn logical_name_for_generator_state(name: &str) -> String {
    name.strip_prefix("_dp_cell_").unwrap_or(name).to_string()
}

fn runtime_init(name: &str) -> Option<ClosureInit> {
    match name {
        "_dp_pc" => Some(ClosureInit::RuntimePcUnstarted),
        "_dp_yieldfrom" => Some(ClosureInit::RuntimeNone),
        _ => None,
    }
}

pub(crate) fn build_blockpy_closure_layout(
    param_names: &[String],
    state_vars: &[String],
    capture_names: &[String],
    injected_exception_names: &HashSet<String>,
) -> ClosureLayout {
    let ordered_state = state_vars
        .iter()
        .filter(|name| !is_generator_dispatch_param(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let capture_names = capture_names.iter().cloned().collect::<HashSet<_>>();
    let mut seen_storage_names = HashSet::new();

    let mut freevars = Vec::new();
    let mut cellvars = Vec::new();
    let mut runtime_cells = Vec::new();

    for name in ordered_state {
        let logical_name = logical_name_for_generator_state(name.as_str());
        let storage_name = generator_storage_name(name.as_str());
        if !seen_storage_names.insert(storage_name.clone()) {
            continue;
        }
        if let Some(init) = runtime_init(logical_name.as_str()) {
            runtime_cells.push(ClosureSlot {
                logical_name,
                storage_name,
                init,
            });
            continue;
        }
        if name == "_dp_classcell"
            || capture_names.contains(name.as_str())
            || capture_names.contains(logical_name.as_str())
        {
            freevars.push(ClosureSlot {
                logical_name,
                storage_name,
                init: ClosureInit::InheritedCapture,
            });
            continue;
        }
        let init = if injected_exception_names.contains(logical_name.as_str()) {
            ClosureInit::DeletedSentinel
        } else if param_names.iter().any(|param| param == &logical_name) {
            ClosureInit::Parameter
        } else {
            ClosureInit::Deferred
        };
        cellvars.push(ClosureSlot {
            logical_name,
            storage_name,
            init,
        });
    }

    ClosureLayout {
        freevars,
        cellvars,
        runtime_cells,
    }
}

#[cfg(test)]
mod tests {
    use super::build_blockpy_closure_layout;
    use crate::basic_block::block_py::{
        BlockPyCfgBlockBuilder, BlockPyTerm, ClosureInit, ClosureLayout, ClosureSlot, FunctionId,
    };
    use crate::basic_block::ruff_to_blockpy::lower_stmts_to_blockpy_stmts;
    use crate::{py_expr, py_stmt};
    use ruff_python_ast::Expr;
    use std::collections::HashSet;

    fn blockpy_make_dp_tuple(items: Vec<Expr>) -> Expr {
        let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
            panic!("expected call expression for __dp_tuple");
        };
        call.arguments.args = items.into();
        Expr::Call(call)
    }

    fn closure_backed_generator_init_expr(slot: &ClosureSlot) -> Expr {
        match slot.init {
            ClosureInit::InheritedCapture => {
                panic!("inherited captures do not allocate new cells in outer factories")
            }
            ClosureInit::Parameter => {
                py_expr!("{name:id}", name = slot.logical_name.as_str())
            }
            ClosureInit::DeletedSentinel => py_expr!("__dp_DELETED"),
            ClosureInit::RuntimePcUnstarted => py_expr!("1"),
            ClosureInit::RuntimeNone => py_expr!("None"),
            ClosureInit::Deferred => py_expr!("None"),
        }
    }

    fn build_closure_backed_generator_factory_block(
        factory_label: &str,
        resume_label: &str,
        resume_function_id: FunctionId,
        resume_state_order: &[String],
        function_name: &str,
        qualname: &str,
        layout: &ClosureLayout,
        is_coroutine: bool,
        is_async_generator: bool,
    ) -> crate::basic_block::block_py::BlockPyBlock<Expr> {
        let hidden_name = "_dp_resume".to_string();
        let hidden_qualname = qualname.to_string();
        let mut body = Vec::new();

        for slot in layout.cellvars.iter().chain(layout.runtime_cells.iter()) {
            let stmt = py_stmt!(
                "{cell:id} = __dp_make_cell({init:expr})",
                cell = slot.storage_name.as_str(),
                init = closure_backed_generator_init_expr(slot),
            );
            let lowered = lower_stmts_to_blockpy_stmts::<Expr>(&[stmt])
                .unwrap_or_else(|err| panic!("failed to lower generator factory cell init: {err}"));
            assert!(lowered.term.is_none());
            body.extend(lowered.body);
        }

        let closure_names: Vec<String> = resume_state_order
            .iter()
            .filter(|state_name| {
                !matches!(
                    state_name.as_str(),
                    "_dp_self" | "_dp_send_value" | "_dp_resume_exc" | "_dp_transport_sent"
                )
            })
            .cloned()
            .collect();
        let closure_values = blockpy_make_dp_tuple(
            closure_names
                .iter()
                .map(|state_name| py_expr!("{name:id}", name = state_name.as_str()))
                .collect(),
        );

        let resume_entry = py_expr!(
            "__dp_def_hidden_resume_fn({resume:literal}, {function_id:literal}, {name:literal}, {qualname:literal}, {state_order:expr}, {closure_names:expr}, {closure_values:expr}, __dp_globals(), __name__, async_gen={async_gen:expr})",
            resume = resume_label,
            function_id = resume_function_id.0,
            name = hidden_name.as_str(),
            qualname = hidden_qualname.as_str(),
            state_order = blockpy_make_dp_tuple(
                resume_state_order
                    .iter()
                    .map(|state_name| py_expr!("{value:literal}", value = state_name.as_str()))
                    .collect(),
            ),
            closure_names = blockpy_make_dp_tuple(
                closure_names
                    .iter()
                    .map(|state_name| py_expr!("{value:literal}", value = state_name.as_str()))
                    .collect(),
            ),
            closure_values = closure_values,
            async_gen = if is_async_generator {
                py_expr!("True")
            } else {
                py_expr!("False")
            },
        );

        let generator_expr = if is_async_generator {
            py_expr!(
                "__dp_make_closure_async_generator({resume:expr}, {name:literal}, {qualname:literal})",
                resume = resume_entry,
                name = function_name,
                qualname = qualname,
            )
        } else {
            py_expr!(
                "__dp_make_closure_generator({resume:expr}, {name:literal}, {qualname:literal})",
                resume = resume_entry,
                name = function_name,
                qualname = qualname,
            )
        };

        let return_value = if is_coroutine {
            py_expr!(
                "__dp_make_coroutine_from_generator({gen:expr})",
                gen = generator_expr
            )
        } else {
            generator_expr
        };

        let mut block = BlockPyCfgBlockBuilder::new(factory_label.into());
        block.extend(body);
        block.set_term(BlockPyTerm::Return(Some(return_value.into())));
        block.finish(None)
    }

    #[test]
    fn build_blockpy_closure_layout_classifies_capture_local_and_runtime_cells() {
        let layout = build_blockpy_closure_layout(
            &["arg".to_string()],
            &[
                "_dp_self".to_string(),
                "arg".to_string(),
                "captured".to_string(),
                "_dp_yieldfrom".to_string(),
                "_dp_pc".to_string(),
                "_dp_try_exc_0".to_string(),
            ],
            &["captured".to_string()],
            &HashSet::from(["_dp_try_exc_0".to_string()]),
        );

        assert_eq!(
            layout
                .freevars
                .iter()
                .map(|slot| (slot.logical_name.as_str(), slot.storage_name.as_str()))
                .collect::<Vec<_>>(),
            vec![("captured", "_dp_cell_captured")]
        );
        assert_eq!(
            layout
                .cellvars
                .iter()
                .map(|slot| (
                    slot.logical_name.as_str(),
                    slot.storage_name.as_str(),
                    &slot.init
                ))
                .collect::<Vec<_>>(),
            vec![
                ("arg", "_dp_cell_arg", &ClosureInit::Parameter),
                (
                    "_dp_try_exc_0",
                    "_dp_cell__dp_try_exc_0",
                    &ClosureInit::DeletedSentinel
                ),
            ]
        );
        assert_eq!(
            layout
                .runtime_cells
                .iter()
                .map(|slot| (
                    slot.logical_name.as_str(),
                    slot.storage_name.as_str(),
                    &slot.init
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "_dp_yieldfrom",
                    "_dp_cell__dp_yieldfrom",
                    &ClosureInit::RuntimeNone
                ),
                (
                    "_dp_pc",
                    "_dp_cell__dp_pc",
                    &ClosureInit::RuntimePcUnstarted
                ),
            ]
        );
    }

    #[test]
    fn builds_closure_backed_generator_factory_block() {
        let layout = ClosureLayout {
            freevars: vec![ClosureSlot {
                logical_name: "captured".to_string(),
                storage_name: "_dp_cell_captured".to_string(),
                init: ClosureInit::InheritedCapture,
            }],
            cellvars: vec![ClosureSlot {
                logical_name: "x".to_string(),
                storage_name: "_dp_cell_x".to_string(),
                init: ClosureInit::Parameter,
            }],
            runtime_cells: vec![ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            }],
        };

        let block = build_closure_backed_generator_factory_block(
            "_dp_bb_demo_factory",
            "_dp_bb_demo_0",
            FunctionId(0),
            &[
                "_dp_self".to_string(),
                "_dp_send_value".to_string(),
                "_dp_resume_exc".to_string(),
                "_dp_cell_captured".to_string(),
                "_dp_cell_x".to_string(),
                "_dp_cell__dp_pc".to_string(),
            ],
            "demo",
            "demo",
            &layout,
            false,
            false,
        );

        assert_eq!(block.label.as_str(), "_dp_bb_demo_factory");
        assert!(matches!(block.term, BlockPyTerm::Return(Some(_))));
    }
}

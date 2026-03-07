use super::state_vars::sync_generator_state_order;
use crate::basic_block::bb_ir::{
    BbGeneratorClosureInit, BbGeneratorClosureLayout, BbGeneratorClosureSlot,
};
use crate::transform::scope::cell_name;
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

fn runtime_init(name: &str) -> Option<BbGeneratorClosureInit> {
    match name {
        "_dp_pc" => Some(BbGeneratorClosureInit::RuntimePcZero),
        _ => None,
    }
}

pub(super) fn build_generator_closure_layout(
    param_names: &[String],
    state_vars: &[String],
    capture_names: &[String],
    injected_exception_names: &HashSet<String>,
) -> BbGeneratorClosureLayout {
    let ordered_state = sync_generator_state_order(state_vars, injected_exception_names);
    let capture_names = capture_names.iter().cloned().collect::<HashSet<_>>();

    let mut inherited_captures = Vec::new();
    let mut lifted_locals = Vec::new();
    let mut runtime_cells = Vec::new();

    for name in ordered_state {
        if is_generator_dispatch_param(name.as_str()) {
            continue;
        }
        let logical_name = logical_name_for_generator_state(name.as_str());
        let storage_name = generator_storage_name(name.as_str());
        if let Some(init) = runtime_init(logical_name.as_str()) {
            runtime_cells.push(BbGeneratorClosureSlot {
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
            inherited_captures.push(BbGeneratorClosureSlot {
                logical_name,
                storage_name,
                init: BbGeneratorClosureInit::InheritedCapture,
            });
            continue;
        }
        let init = if injected_exception_names.contains(logical_name.as_str()) {
            BbGeneratorClosureInit::DeletedSentinel
        } else if param_names.iter().any(|param| param == &logical_name) {
            BbGeneratorClosureInit::Parameter
        } else {
            BbGeneratorClosureInit::Deferred
        };
        lifted_locals.push(BbGeneratorClosureSlot {
            logical_name,
            storage_name,
            init,
        });
    }

    BbGeneratorClosureLayout {
        inherited_captures,
        lifted_locals,
        runtime_cells,
    }
}

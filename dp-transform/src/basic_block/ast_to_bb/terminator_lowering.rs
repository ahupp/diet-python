use super::{BbFunctionKind, LoweredKind};

pub(super) fn bb_function_kind_from(kind: &LoweredKind) -> BbFunctionKind {
    match kind {
        LoweredKind::Function => BbFunctionKind::Function,
        LoweredKind::Generator {
            closure_state,
            resume_label,
            target_labels,
            resume_pcs,
        } => BbFunctionKind::Generator {
            closure_state: *closure_state,
            resume_label: resume_label.clone(),
            target_labels: target_labels.clone(),
            resume_pcs: resume_pcs.clone(),
        },
        LoweredKind::AsyncGenerator {
            closure_state,
            resume_label,
            target_labels,
            resume_pcs,
        } => BbFunctionKind::AsyncGenerator {
            closure_state: *closure_state,
            resume_label: resume_label.clone(),
            target_labels: target_labels.clone(),
            resume_pcs: resume_pcs.clone(),
        },
    }
}

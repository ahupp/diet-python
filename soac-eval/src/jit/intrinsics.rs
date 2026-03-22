use super::{DirectSimpleCallPart, DirectSimpleIntrinsicEmitState, ImportSpec, SigType};
use crate::jit::blockpy_intrinsics;
use cranelift_codegen::ir;

pub(super) trait JitIntrinsic: blockpy_intrinsics::Intrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> Option<ir::Value>;
}

static PYNUMBER_ADD_IMPORT: ImportSpec = ImportSpec::new(
    "PyNumber_Add",
    &[SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);

impl JitIntrinsic for blockpy_intrinsics::AddIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> Option<ir::Value> {
        let args = state.positional_args_for_intrinsic(self, parts);
        let add_ref = state.import_func(&PYNUMBER_ADD_IMPORT);
        Some(state.emit_owned_func_call(add_ref, &args))
    }
}

pub(super) fn jit_intrinsic_by_intrinsic(
    intrinsic: &'static dyn blockpy_intrinsics::Intrinsic,
) -> Option<&'static dyn JitIntrinsic> {
    intrinsic
        .as_any()
        .downcast_ref::<blockpy_intrinsics::AddIntrinsic>()
        .map(|value| value as &dyn JitIntrinsic)
}

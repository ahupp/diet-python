use super::{
    DirectSimpleCallPart, DirectSimpleIntrinsicEmitState, ImportSpec, SigType,
    emit_owned_bool_from_cond,
};
use crate::jit::blockpy_intrinsics;
use cranelift_codegen::ir;
use cranelift_codegen::ir::InstBuilder;
use pyo3::ffi;

pub(super) trait JitIntrinsic: blockpy_intrinsics::Intrinsic {
    fn emit_positional_owned_call(
        &self,
        spec: &'static ImportSpec,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value
    where
        Self: Sized,
    {
        let args = state.positional_args_for_intrinsic(self, parts);
        let func_ref = state.import_func(spec);
        state.emit_owned_func_call(func_ref, &args)
    }

    fn emit_positional_bool_call(
        &self,
        spec: &'static ImportSpec,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value
    where
        Self: Sized,
    {
        let args = state.positional_args_for_intrinsic(self, parts);
        let func_ref = state.import_func(spec);
        state.emit_bool_func_call(func_ref, &args)
    }

    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value;
}

macro_rules! define_owned_import_intrinsic {
    ($intrinsic_ty:path, $spec_name:ident, $symbol:literal, $params:expr) => {
        static $spec_name: ImportSpec = ImportSpec::new($symbol, $params, &[SigType::Pointer]);

        impl JitIntrinsic for $intrinsic_ty {
            fn emit_direct_simple(
                &self,
                state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
                parts: &[DirectSimpleCallPart],
            ) -> ir::Value {
                self.emit_positional_owned_call(&$spec_name, state, parts)
            }
        }
    };
}

macro_rules! define_bool_import_intrinsic {
    ($intrinsic_ty:path, $spec_name:ident, $symbol:literal, $params:expr) => {
        static $spec_name: ImportSpec = ImportSpec::new($symbol, $params, &[SigType::I32]);

        impl JitIntrinsic for $intrinsic_ty {
            fn emit_direct_simple(
                &self,
                state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
                parts: &[DirectSimpleCallPart],
            ) -> ir::Value {
                self.emit_positional_bool_call(&$spec_name, state, parts)
            }
        }
    };
}

define_owned_import_intrinsic!(
    blockpy_intrinsics::AddIntrinsic,
    PYNUMBER_ADD_IMPORT,
    "PyNumber_Add",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::SubIntrinsic,
    PYNUMBER_SUBTRACT_IMPORT,
    "PyNumber_Subtract",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::MulIntrinsic,
    PYNUMBER_MULTIPLY_IMPORT,
    "PyNumber_Multiply",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::MatMulIntrinsic,
    PYNUMBER_MATMUL_IMPORT,
    "PyNumber_MatrixMultiply",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::TrueDivIntrinsic,
    PYNUMBER_TRUE_DIVIDE_IMPORT,
    "PyNumber_TrueDivide",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::FloorDivIntrinsic,
    PYNUMBER_FLOOR_DIVIDE_IMPORT,
    "PyNumber_FloorDivide",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::ModIntrinsic,
    PYNUMBER_REMAINDER_IMPORT,
    "PyNumber_Remainder",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::LShiftIntrinsic,
    PYNUMBER_LSHIFT_IMPORT,
    "PyNumber_Lshift",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::RShiftIntrinsic,
    PYNUMBER_RSHIFT_IMPORT,
    "PyNumber_Rshift",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::OrIntrinsic,
    PYNUMBER_OR_IMPORT,
    "PyNumber_Or",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::XorIntrinsic,
    PYNUMBER_XOR_IMPORT,
    "PyNumber_Xor",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::AndIntrinsic,
    PYNUMBER_AND_IMPORT,
    "PyNumber_And",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceAddIntrinsic,
    PYNUMBER_INPLACE_ADD_IMPORT,
    "PyNumber_InPlaceAdd",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceSubIntrinsic,
    PYNUMBER_INPLACE_SUBTRACT_IMPORT,
    "PyNumber_InPlaceSubtract",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceMulIntrinsic,
    PYNUMBER_INPLACE_MULTIPLY_IMPORT,
    "PyNumber_InPlaceMultiply",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceMatMulIntrinsic,
    PYNUMBER_INPLACE_MATMUL_IMPORT,
    "PyNumber_InPlaceMatrixMultiply",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceTrueDivIntrinsic,
    PYNUMBER_INPLACE_TRUE_DIVIDE_IMPORT,
    "PyNumber_InPlaceTrueDivide",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceFloorDivIntrinsic,
    PYNUMBER_INPLACE_FLOOR_DIVIDE_IMPORT,
    "PyNumber_InPlaceFloorDivide",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceModIntrinsic,
    PYNUMBER_INPLACE_REMAINDER_IMPORT,
    "PyNumber_InPlaceRemainder",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceLShiftIntrinsic,
    PYNUMBER_INPLACE_LSHIFT_IMPORT,
    "PyNumber_InPlaceLshift",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceRShiftIntrinsic,
    PYNUMBER_INPLACE_RSHIFT_IMPORT,
    "PyNumber_InPlaceRshift",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceOrIntrinsic,
    PYNUMBER_INPLACE_OR_IMPORT,
    "PyNumber_InPlaceOr",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceXorIntrinsic,
    PYNUMBER_INPLACE_XOR_IMPORT,
    "PyNumber_InPlaceXor",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InPlaceAndIntrinsic,
    PYNUMBER_INPLACE_AND_IMPORT,
    "PyNumber_InPlaceAnd",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::PosIntrinsic,
    PYNUMBER_POSITIVE_IMPORT,
    "PyNumber_Positive",
    &[SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::NegIntrinsic,
    PYNUMBER_NEGATIVE_IMPORT,
    "PyNumber_Negative",
    &[SigType::Pointer]
);
define_owned_import_intrinsic!(
    blockpy_intrinsics::InvertIntrinsic,
    PYNUMBER_INVERT_IMPORT,
    "PyNumber_Invert",
    &[SigType::Pointer]
);
define_bool_import_intrinsic!(
    blockpy_intrinsics::NotIntrinsic,
    PYOBJECT_NOT_IMPORT,
    "PyObject_Not",
    &[SigType::Pointer]
);
define_bool_import_intrinsic!(
    blockpy_intrinsics::TruthIntrinsic,
    PYOBJECT_IS_TRUE_IMPORT,
    "PyObject_IsTrue",
    &[SigType::Pointer]
);
define_bool_import_intrinsic!(
    blockpy_intrinsics::ContainsIntrinsic,
    PYSEQUENCE_CONTAINS_IMPORT,
    "PySequence_Contains",
    &[SigType::Pointer, SigType::Pointer]
);

static PYOBJECT_RICHCOMPARE_IMPORT: ImportSpec = ImportSpec::new(
    "PyObject_RichCompare",
    &[SigType::Pointer, SigType::Pointer, SigType::I32],
    &[SigType::Pointer],
);
static PYNUMBER_POWER_IMPORT: ImportSpec = ImportSpec::new(
    "PyNumber_Power",
    &[SigType::Pointer, SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);
static PYNUMBER_INPLACE_POWER_IMPORT: ImportSpec = ImportSpec::new(
    "PyNumber_InPlacePower",
    &[SigType::Pointer, SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
);

fn emit_pow_like(
    intrinsic: &dyn blockpy_intrinsics::Intrinsic,
    spec: &'static ImportSpec,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_intrinsic(intrinsic, parts);
    let arg_values = state.emit_arg_values(&args);
    let func_ref = state.import_func(spec);
    let call_inst = match arg_values.as_slice() {
        [(left, _), (right, _)] => state
            .fb
            .ins()
            .call(func_ref, &[*left, *right, state.ctx.consts.none_const]),
        [(left, _), (right, _), (modulo, _)] => {
            state.fb.ins().call(func_ref, &[*left, *right, *modulo])
        }
        _ => panic!(
            "pow-like intrinsic {} received unsupported arity {}",
            intrinsic.name(),
            arg_values.len()
        ),
    };
    state.release_arg_values(&arg_values);
    state.finish_owned_result(state.fb.inst_results(call_inst)[0])
}

fn emit_richcompare(
    intrinsic: &dyn blockpy_intrinsics::Intrinsic,
    compare_op: i32,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_intrinsic(intrinsic, parts);
    let arg_values = state.emit_arg_values(&args);
    let func_ref = state.import_func(&PYOBJECT_RICHCOMPARE_IMPORT);
    let compare_op = state.fb.ins().iconst(ir::types::I32, compare_op as i64);
    let call_inst = state
        .fb
        .ins()
        .call(func_ref, &[arg_values[0].0, arg_values[1].0, compare_op]);
    state.release_arg_values(&arg_values);
    state.finish_owned_result(state.fb.inst_results(call_inst)[0])
}

fn emit_identity_compare(
    intrinsic: &dyn blockpy_intrinsics::Intrinsic,
    expect_equal: bool,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_intrinsic(intrinsic, parts);
    let arg_values = state.emit_arg_values(&args);
    let cond = state.fb.ins().icmp(
        if expect_equal {
            ir::condcodes::IntCC::Equal
        } else {
            ir::condcodes::IntCC::NotEqual
        },
        arg_values[0].0,
        arg_values[1].0,
    );
    state.release_arg_values(&arg_values);
    emit_owned_bool_from_cond(state.fb, cond, state.ctx)
}

impl JitIntrinsic for blockpy_intrinsics::GetAttrIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        let args = state.positional_args_for_intrinsic(self, parts);
        state.emit_owned_func_call(state.ctx.pyobject_getattr_ref, &args)
    }
}

impl JitIntrinsic for blockpy_intrinsics::SetAttrIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        let args = state.positional_args_for_intrinsic(self, parts);
        state.emit_owned_func_call(state.ctx.pyobject_setattr_ref, &args)
    }
}

impl JitIntrinsic for blockpy_intrinsics::MakeCellIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        let args = state.positional_args_for_intrinsic(self, parts);
        let arg_values = state.emit_arg_values(&args);
        let call_inst = state
            .fb
            .ins()
            .call(state.ctx.make_cell_ref, &[arg_values[0].0]);
        state.release_arg_values(&arg_values);
        state.finish_owned_result(state.fb.inst_results(call_inst)[0])
    }
}

impl JitIntrinsic for blockpy_intrinsics::GetItemIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        let args = state.positional_args_for_intrinsic(self, parts);
        state.emit_owned_func_call(state.ctx.pyobject_getitem_ref, &args)
    }
}

impl JitIntrinsic for blockpy_intrinsics::SetItemIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        let args = state.positional_args_for_intrinsic(self, parts);
        state.emit_owned_func_call(state.ctx.pyobject_setitem_ref, &args)
    }
}

impl JitIntrinsic for blockpy_intrinsics::PowIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_pow_like(self, &PYNUMBER_POWER_IMPORT, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::InPlacePowIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_pow_like(self, &PYNUMBER_INPLACE_POWER_IMPORT, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::EqIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_richcompare(self, ffi::Py_EQ, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::NeIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_richcompare(self, ffi::Py_NE, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::LtIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_richcompare(self, ffi::Py_LT, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::LeIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_richcompare(self, ffi::Py_LE, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::GtIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_richcompare(self, ffi::Py_GT, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::GeIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_richcompare(self, ffi::Py_GE, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::IsIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_identity_compare(self, true, state, parts)
    }
}

impl JitIntrinsic for blockpy_intrinsics::IsNotIntrinsic {
    fn emit_direct_simple(
        &self,
        state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
        parts: &[DirectSimpleCallPart],
    ) -> ir::Value {
        emit_identity_compare(self, false, state, parts)
    }
}

pub(super) fn jit_intrinsic_by_intrinsic(
    intrinsic: &'static dyn blockpy_intrinsics::Intrinsic,
) -> Option<&'static dyn JitIntrinsic> {
    match intrinsic.name() {
        "__dp_add" => Some(&blockpy_intrinsics::ADD_INTRINSIC),
        "__dp_getattr" => Some(&blockpy_intrinsics::GETATTR_INTRINSIC),
        "__dp_setattr" => Some(&blockpy_intrinsics::SETATTR_INTRINSIC),
        "__dp_make_cell" => Some(&blockpy_intrinsics::MAKE_CELL_INTRINSIC),
        "__dp_getitem" => Some(&blockpy_intrinsics::GETITEM_INTRINSIC),
        "__dp_setitem" => Some(&blockpy_intrinsics::SETITEM_INTRINSIC),
        "__dp_sub" => Some(&blockpy_intrinsics::SUB_INTRINSIC),
        "__dp_mul" => Some(&blockpy_intrinsics::MUL_INTRINSIC),
        "__dp_matmul" => Some(&blockpy_intrinsics::MATMUL_INTRINSIC),
        "__dp_truediv" => Some(&blockpy_intrinsics::TRUEDIV_INTRINSIC),
        "__dp_floordiv" => Some(&blockpy_intrinsics::FLOORDIV_INTRINSIC),
        "__dp_mod" => Some(&blockpy_intrinsics::MOD_INTRINSIC),
        "__dp_pow" => Some(&blockpy_intrinsics::POW_INTRINSIC),
        "__dp_lshift" => Some(&blockpy_intrinsics::LSHIFT_INTRINSIC),
        "__dp_rshift" => Some(&blockpy_intrinsics::RSHIFT_INTRINSIC),
        "__dp_or_" => Some(&blockpy_intrinsics::OR_INTRINSIC),
        "__dp_xor" => Some(&blockpy_intrinsics::XOR_INTRINSIC),
        "__dp_and_" => Some(&blockpy_intrinsics::AND_INTRINSIC),
        "__dp_iadd" => Some(&blockpy_intrinsics::IADD_INTRINSIC),
        "__dp_isub" => Some(&blockpy_intrinsics::ISUB_INTRINSIC),
        "__dp_imul" => Some(&blockpy_intrinsics::IMUL_INTRINSIC),
        "__dp_imatmul" => Some(&blockpy_intrinsics::IMATMUL_INTRINSIC),
        "__dp_itruediv" => Some(&blockpy_intrinsics::ITRUEDIV_INTRINSIC),
        "__dp_ifloordiv" => Some(&blockpy_intrinsics::IFLOORDIV_INTRINSIC),
        "__dp_imod" => Some(&blockpy_intrinsics::IMOD_INTRINSIC),
        "__dp_ipow" => Some(&blockpy_intrinsics::IPOW_INTRINSIC),
        "__dp_ilshift" => Some(&blockpy_intrinsics::ILSHIFT_INTRINSIC),
        "__dp_irshift" => Some(&blockpy_intrinsics::IRSHIFT_INTRINSIC),
        "__dp_ior" => Some(&blockpy_intrinsics::IOR_INTRINSIC),
        "__dp_ixor" => Some(&blockpy_intrinsics::IXOR_INTRINSIC),
        "__dp_iand" => Some(&blockpy_intrinsics::IAND_INTRINSIC),
        "__dp_pos" => Some(&blockpy_intrinsics::POS_INTRINSIC),
        "__dp_neg" => Some(&blockpy_intrinsics::NEG_INTRINSIC),
        "__dp_invert" => Some(&blockpy_intrinsics::INVERT_INTRINSIC),
        "__dp_not_" => Some(&blockpy_intrinsics::NOT_INTRINSIC),
        "__dp_truth" => Some(&blockpy_intrinsics::TRUTH_INTRINSIC),
        "__dp_eq" => Some(&blockpy_intrinsics::EQ_INTRINSIC),
        "__dp_ne" => Some(&blockpy_intrinsics::NE_INTRINSIC),
        "__dp_lt" => Some(&blockpy_intrinsics::LT_INTRINSIC),
        "__dp_le" => Some(&blockpy_intrinsics::LE_INTRINSIC),
        "__dp_gt" => Some(&blockpy_intrinsics::GT_INTRINSIC),
        "__dp_ge" => Some(&blockpy_intrinsics::GE_INTRINSIC),
        "__dp_contains" => Some(&blockpy_intrinsics::CONTAINS_INTRINSIC),
        "__dp_is_" => Some(&blockpy_intrinsics::IS_INTRINSIC),
        "__dp_is_not" => Some(&blockpy_intrinsics::IS_NOT_INTRINSIC),
        _ => None,
    }
}

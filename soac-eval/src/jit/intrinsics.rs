use super::{DirectSimpleCallPart, DirectSimpleIntrinsicEmitState, ImportSpec, SigType};
use crate::jit::blockpy_intrinsics;
use cranelift_codegen::ir;
use cranelift_codegen::ir::InstBuilder;
use pyo3::ffi;

macro_rules! define_owned_import_spec {
    ($spec_name:ident, $symbol:literal, $params:expr) => {
        static $spec_name: ImportSpec = ImportSpec::new($symbol, $params, &[SigType::Pointer]);
    };
}

macro_rules! define_bool_import_spec {
    ($spec_name:ident, $symbol:literal, $params:expr) => {
        static $spec_name: ImportSpec = ImportSpec::new($symbol, $params, &[SigType::I32]);
    };
}

define_owned_import_spec!(
    PYNUMBER_ADD_IMPORT,
    "PyNumber_Add",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_SUBTRACT_IMPORT,
    "PyNumber_Subtract",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_MULTIPLY_IMPORT,
    "PyNumber_Multiply",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_MATMUL_IMPORT,
    "PyNumber_MatrixMultiply",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_TRUE_DIVIDE_IMPORT,
    "PyNumber_TrueDivide",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_FLOOR_DIVIDE_IMPORT,
    "PyNumber_FloorDivide",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_REMAINDER_IMPORT,
    "PyNumber_Remainder",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_LSHIFT_IMPORT,
    "PyNumber_Lshift",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_RSHIFT_IMPORT,
    "PyNumber_Rshift",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_OR_IMPORT,
    "PyNumber_Or",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_XOR_IMPORT,
    "PyNumber_Xor",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_AND_IMPORT,
    "PyNumber_And",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_ADD_IMPORT,
    "PyNumber_InPlaceAdd",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_SUBTRACT_IMPORT,
    "PyNumber_InPlaceSubtract",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_MULTIPLY_IMPORT,
    "PyNumber_InPlaceMultiply",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_MATMUL_IMPORT,
    "PyNumber_InPlaceMatrixMultiply",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_TRUE_DIVIDE_IMPORT,
    "PyNumber_InPlaceTrueDivide",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_FLOOR_DIVIDE_IMPORT,
    "PyNumber_InPlaceFloorDivide",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_REMAINDER_IMPORT,
    "PyNumber_InPlaceRemainder",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_LSHIFT_IMPORT,
    "PyNumber_InPlaceLshift",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_RSHIFT_IMPORT,
    "PyNumber_InPlaceRshift",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_OR_IMPORT,
    "PyNumber_InPlaceOr",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_XOR_IMPORT,
    "PyNumber_InPlaceXor",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INPLACE_AND_IMPORT,
    "PyNumber_InPlaceAnd",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_POSITIVE_IMPORT,
    "PyNumber_Positive",
    &[SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_NEGATIVE_IMPORT,
    "PyNumber_Negative",
    &[SigType::Pointer]
);
define_owned_import_spec!(
    PYNUMBER_INVERT_IMPORT,
    "PyNumber_Invert",
    &[SigType::Pointer]
);
define_bool_import_spec!(PYOBJECT_NOT_IMPORT, "PyObject_Not", &[SigType::Pointer]);
define_bool_import_spec!(
    PYOBJECT_IS_TRUE_IMPORT,
    "PyObject_IsTrue",
    &[SigType::Pointer]
);
define_bool_import_spec!(
    PYSEQUENCE_CONTAINS_IMPORT,
    "PySequence_Contains",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    DP_JIT_PYOBJECT_DELITEM_IMPORT,
    "dp_jit_pyobject_delitem",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    DP_JIT_LOAD_GLOBAL_OBJ_IMPORT,
    "dp_jit_load_global_obj",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    DP_JIT_STORE_GLOBAL_IMPORT,
    "dp_jit_store_global",
    &[SigType::Pointer, SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    DP_JIT_DEL_QUIETLY_IMPORT,
    "dp_jit_del_quietly",
    &[SigType::Pointer, SigType::Pointer]
);
define_owned_import_spec!(
    DP_JIT_DEL_DEREF_QUIETLY_IMPORT,
    "dp_jit_del_deref_quietly",
    &[SigType::Pointer]
);
define_owned_import_spec!(
    DP_JIT_DEL_DEREF_IMPORT,
    "dp_jit_del_deref",
    &[SigType::Pointer]
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
fn emit_positional_owned_call(
    helper_name: &str,
    spec: &'static ImportSpec,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
    let func_ref = state.import_func(spec);
    state.emit_owned_func_call(func_ref, &args)
}

fn emit_positional_bool_call(
    helper_name: &str,
    spec: &'static ImportSpec,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
    let func_ref = state.import_func(spec);
    state.emit_bool_func_call(func_ref, &args)
}

fn emit_pow_like(
    helper_name: &str,
    spec: &'static ImportSpec,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
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
            "pow-like operation {helper_name} received unsupported arity {}",
            arg_values.len()
        ),
    };
    state.release_arg_values(&arg_values);
    state.finish_owned_result(state.fb.inst_results(call_inst)[0])
}

fn emit_richcompare(
    helper_name: &str,
    compare_op: i32,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
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
    helper_name: &str,
    expect_equal: bool,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
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
    super::emit_owned_bool_from_cond(state.fb, cond, state.ctx)
}

fn emit_getattr(
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
    state.emit_owned_func_call(state.ctx.pyobject_getattr_ref, &args)
}

fn emit_setattr(
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
    state.emit_owned_func_call(state.ctx.pyobject_setattr_ref, &args)
}

fn emit_make_cell(
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
    let arg_values = state.emit_arg_values(&args);
    let call_inst = state
        .fb
        .ins()
        .call(state.ctx.make_cell_ref, &[arg_values[0].0]);
    state.release_arg_values(&arg_values);
    state.finish_owned_result(state.fb.inst_results(call_inst)[0])
}

fn emit_getitem(
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
    state.emit_owned_func_call(state.ctx.pyobject_getitem_ref, &args)
}

fn emit_setitem(
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    let args = state.positional_args_for_operation(helper_name, parts);
    state.emit_owned_func_call(state.ctx.pyobject_setitem_ref, &args)
}

fn emit_binop(
    kind: blockpy_intrinsics::BinOpKind,
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    match kind {
        blockpy_intrinsics::BinOpKind::Add => {
            emit_positional_owned_call(helper_name, &PYNUMBER_ADD_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Sub => {
            emit_positional_owned_call(helper_name, &PYNUMBER_SUBTRACT_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Mul => {
            emit_positional_owned_call(helper_name, &PYNUMBER_MULTIPLY_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::MatMul => {
            emit_positional_owned_call(helper_name, &PYNUMBER_MATMUL_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::TrueDiv => {
            emit_positional_owned_call(helper_name, &PYNUMBER_TRUE_DIVIDE_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::FloorDiv => {
            emit_positional_owned_call(helper_name, &PYNUMBER_FLOOR_DIVIDE_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Mod => {
            emit_positional_owned_call(helper_name, &PYNUMBER_REMAINDER_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::LShift => {
            emit_positional_owned_call(helper_name, &PYNUMBER_LSHIFT_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::RShift => {
            emit_positional_owned_call(helper_name, &PYNUMBER_RSHIFT_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Or => {
            emit_positional_owned_call(helper_name, &PYNUMBER_OR_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Xor => {
            emit_positional_owned_call(helper_name, &PYNUMBER_XOR_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::And => {
            emit_positional_owned_call(helper_name, &PYNUMBER_AND_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Eq => {
            emit_richcompare(helper_name, ffi::Py_EQ, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Ne => {
            emit_richcompare(helper_name, ffi::Py_NE, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Lt => {
            emit_richcompare(helper_name, ffi::Py_LT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Le => {
            emit_richcompare(helper_name, ffi::Py_LE, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Gt => {
            emit_richcompare(helper_name, ffi::Py_GT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Ge => {
            emit_richcompare(helper_name, ffi::Py_GE, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Contains => {
            emit_positional_bool_call(helper_name, &PYSEQUENCE_CONTAINS_IMPORT, state, parts)
        }
        blockpy_intrinsics::BinOpKind::Is => emit_identity_compare(helper_name, true, state, parts),
        blockpy_intrinsics::BinOpKind::IsNot => {
            emit_identity_compare(helper_name, false, state, parts)
        }
    }
}

fn emit_unary_op(
    kind: blockpy_intrinsics::UnaryOpKind,
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    match kind {
        blockpy_intrinsics::UnaryOpKind::Pos => {
            emit_positional_owned_call(helper_name, &PYNUMBER_POSITIVE_IMPORT, state, parts)
        }
        blockpy_intrinsics::UnaryOpKind::Neg => {
            emit_positional_owned_call(helper_name, &PYNUMBER_NEGATIVE_IMPORT, state, parts)
        }
        blockpy_intrinsics::UnaryOpKind::Invert => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INVERT_IMPORT, state, parts)
        }
        blockpy_intrinsics::UnaryOpKind::Not => {
            emit_positional_bool_call(helper_name, &PYOBJECT_NOT_IMPORT, state, parts)
        }
        blockpy_intrinsics::UnaryOpKind::Truth => {
            emit_positional_bool_call(helper_name, &PYOBJECT_IS_TRUE_IMPORT, state, parts)
        }
    }
}

fn emit_ternary_op(
    kind: blockpy_intrinsics::TernaryOpKind,
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    match kind {
        blockpy_intrinsics::TernaryOpKind::Pow => {
            emit_pow_like(helper_name, &PYNUMBER_POWER_IMPORT, state, parts)
        }
    }
}

fn emit_inplace_binop(
    kind: blockpy_intrinsics::InplaceBinOpKind,
    helper_name: &str,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> ir::Value {
    match kind {
        blockpy_intrinsics::InplaceBinOpKind::Add => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_ADD_IMPORT, state, parts)
        }
        blockpy_intrinsics::InplaceBinOpKind::Sub => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_SUBTRACT_IMPORT, state, parts)
        }
        blockpy_intrinsics::InplaceBinOpKind::Mul => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_MULTIPLY_IMPORT, state, parts)
        }
        blockpy_intrinsics::InplaceBinOpKind::MatMul => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_MATMUL_IMPORT, state, parts)
        }
        blockpy_intrinsics::InplaceBinOpKind::TrueDiv => emit_positional_owned_call(
            helper_name,
            &PYNUMBER_INPLACE_TRUE_DIVIDE_IMPORT,
            state,
            parts,
        ),
        blockpy_intrinsics::InplaceBinOpKind::FloorDiv => emit_positional_owned_call(
            helper_name,
            &PYNUMBER_INPLACE_FLOOR_DIVIDE_IMPORT,
            state,
            parts,
        ),
        blockpy_intrinsics::InplaceBinOpKind::Mod => emit_positional_owned_call(
            helper_name,
            &PYNUMBER_INPLACE_REMAINDER_IMPORT,
            state,
            parts,
        ),
        blockpy_intrinsics::InplaceBinOpKind::LShift => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_LSHIFT_IMPORT, state, parts)
        }
        blockpy_intrinsics::InplaceBinOpKind::RShift => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_RSHIFT_IMPORT, state, parts)
        }
        blockpy_intrinsics::InplaceBinOpKind::Or => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_OR_IMPORT, state, parts)
        }
        blockpy_intrinsics::InplaceBinOpKind::Xor => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_XOR_IMPORT, state, parts)
        }
        blockpy_intrinsics::InplaceBinOpKind::And => {
            emit_positional_owned_call(helper_name, &PYNUMBER_INPLACE_AND_IMPORT, state, parts)
        }
    }
}

pub(super) fn emit_operation_direct_simple<E>(
    operation: &blockpy_intrinsics::Operation<E>,
    state: &mut DirectSimpleIntrinsicEmitState<'_, '_, '_, '_>,
    parts: &[DirectSimpleCallPart],
) -> Option<ir::Value> {
    let helper_name = operation.helper_name();
    match operation {
        blockpy_intrinsics::Operation::BinOp { kind, .. } => {
            Some(emit_binop(*kind, helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::UnaryOp { kind, .. } => {
            Some(emit_unary_op(*kind, helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::InplaceBinOp { kind, .. } => {
            Some(emit_inplace_binop(*kind, helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::TernaryOp { kind, .. } => {
            Some(emit_ternary_op(*kind, helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::GetAttr { .. } => {
            Some(emit_getattr(helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::SetAttr { .. } => {
            Some(emit_setattr(helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::GetItem { .. } => {
            Some(emit_getitem(helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::SetItem { .. } => {
            Some(emit_setitem(helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::DelItem { .. } => Some(emit_positional_owned_call(
            helper_name,
            &DP_JIT_PYOBJECT_DELITEM_IMPORT,
            state,
            parts,
        )),
        blockpy_intrinsics::Operation::LoadGlobal { .. } => Some(emit_positional_owned_call(
            helper_name,
            &DP_JIT_LOAD_GLOBAL_OBJ_IMPORT,
            state,
            parts,
        )),
        blockpy_intrinsics::Operation::StoreGlobal { .. } => Some(emit_positional_owned_call(
            helper_name,
            &DP_JIT_STORE_GLOBAL_IMPORT,
            state,
            parts,
        )),
        blockpy_intrinsics::Operation::LoadCell { .. } => None,
        blockpy_intrinsics::Operation::MakeCell { .. } => {
            Some(emit_make_cell(helper_name, state, parts))
        }
        blockpy_intrinsics::Operation::CellRef { .. } => None,
        blockpy_intrinsics::Operation::StoreCell { .. } => None,
        blockpy_intrinsics::Operation::DelQuietly { .. } => Some(emit_positional_owned_call(
            helper_name,
            &DP_JIT_DEL_QUIETLY_IMPORT,
            state,
            parts,
        )),
        blockpy_intrinsics::Operation::DelDerefQuietly { .. } => Some(emit_positional_owned_call(
            helper_name,
            &DP_JIT_DEL_DEREF_QUIETLY_IMPORT,
            state,
            parts,
        )),
        blockpy_intrinsics::Operation::DelDeref { .. } => Some(emit_positional_owned_call(
            helper_name,
            &DP_JIT_DEL_DEREF_IMPORT,
            state,
            parts,
        )),
    }
}

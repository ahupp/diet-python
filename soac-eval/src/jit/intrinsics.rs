use super::{ImportSpec, JitEmitCtx, SigType, emit_owned_module_constant_from_parts};
use crate::jit::blockpy_intrinsics;
use cranelift_codegen::ir;
use cranelift_codegen::ir::InstBuilder;
use cranelift_frontend::FunctionBuilder;
use pyo3::ffi;

pub(super) trait OperationEmitState<'fb, E> {
    fn ctx(&self) -> &JitEmitCtx<'_>;
    fn fb(&mut self) -> &mut FunctionBuilder<'fb>;
    fn import_func(&mut self, spec: &'static ImportSpec) -> ir::FuncRef;
    fn emit_arg_values(&mut self, args: &[&E]) -> Vec<(ir::Value, bool)>;
    fn release_arg_values(&mut self, arg_values: &[(ir::Value, bool)]);
    fn finish_owned_result(&mut self, value: ir::Value) -> ir::Value;
    fn emit_owned_bool_from_i32_result(&mut self, result: ir::Value) -> ir::Value;
    fn emit_owned_bool_from_cond(&mut self, cond: ir::Value) -> ir::Value;

    fn emit_owned_string_constant(&mut self, value: &str) -> ir::Value {
        let constant_id = self
            .ctx()
            .module_constants
            .require_unicode_constant_id(value);
        self.emit_owned_module_constant(constant_id)
    }

    fn emit_owned_module_constant(
        &mut self,
        constant_id: crate::module_constants::ModuleConstantId,
    ) -> ir::Value {
        let load_module_constant_ref = self.ctx().load_module_constant_ref;
        let vmctx_value = self.ctx().consts.vmctx_value;
        let step_null_block = self.ctx().consts.step_null_block;
        let step_null_args = self.ctx().consts.step_null_args.clone();
        let ptr_ty = self.ctx().consts.ptr_ty;
        let i64_ty = self.ctx().consts.i64_ty;
        emit_owned_module_constant_from_parts(
            self.fb(),
            constant_id,
            load_module_constant_ref,
            vmctx_value,
            step_null_block,
            &step_null_args,
            ptr_ty,
            i64_ty,
        )
    }

    fn emit_owned_func_call(&mut self, func_ref: ir::FuncRef, args: &[&E]) -> ir::Value {
        let arg_values = self.emit_arg_values(args);
        let values = arg_values
            .iter()
            .map(|(value, _)| *value)
            .collect::<Vec<_>>();
        let call_inst = self.fb().ins().call(func_ref, &values);
        self.release_arg_values(&arg_values);
        let result = self.fb().inst_results(call_inst)[0];
        self.finish_owned_result(result)
    }

    fn emit_bool_func_call(&mut self, func_ref: ir::FuncRef, args: &[&E]) -> ir::Value {
        let arg_values = self.emit_arg_values(args);
        let values = arg_values
            .iter()
            .map(|(value, _)| *value)
            .collect::<Vec<_>>();
        let call_inst = self.fb().ins().call(func_ref, &values);
        self.release_arg_values(&arg_values);
        let result = self.fb().inst_results(call_inst)[0];
        self.emit_owned_bool_from_i32_result(result)
    }
}

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
static PYNUMBER_INPLACE_POWER_IMPORT: ImportSpec = ImportSpec::new(
    "PyNumber_InPlacePower",
    &[SigType::Pointer, SigType::Pointer, SigType::Pointer],
    &[SigType::Pointer],
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
    DP_JIT_LOAD_RUNTIME_OBJ_IMPORT,
    "dp_jit_load_runtime_obj",
    &[SigType::Pointer]
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
fn emit_positional_owned_call<'fb, E>(
    spec: &'static ImportSpec,
    state: &mut impl OperationEmitState<'fb, E>,
    args: &[&E],
) -> ir::Value {
    let func_ref = state.import_func(spec);
    state.emit_owned_func_call(func_ref, &args)
}

fn emit_positional_bool_call<'fb, E>(
    spec: &'static ImportSpec,
    state: &mut impl OperationEmitState<'fb, E>,
    args: &[&E],
) -> ir::Value {
    let func_ref = state.import_func(spec);
    state.emit_bool_func_call(func_ref, &args)
}

fn emit_pow_like<'fb, E>(
    spec: &'static ImportSpec,
    state: &mut impl OperationEmitState<'fb, E>,
    args: &[&E],
) -> ir::Value {
    let arg_values = state.emit_arg_values(&args);
    let func_ref = state.import_func(spec);
    let none_const = state.ctx().consts.none_const;
    let call_inst = match arg_values {
        ref arg_values => match arg_values.as_slice() {
            [(left, _), (right, _)] => state
                .fb()
                .ins()
                .call(func_ref, &[*left, *right, none_const]),
            [(left, _), (right, _), (modulo, _)] => {
                state.fb().ins().call(func_ref, &[*left, *right, *modulo])
            }
            _ => panic!(
                "pow-like operation received unsupported arity {}",
                arg_values.len()
            ),
        },
    };
    state.release_arg_values(&arg_values);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

fn emit_richcompare<'fb, E>(
    compare_op: i32,
    state: &mut impl OperationEmitState<'fb, E>,
    args: &[&E],
) -> ir::Value {
    let arg_values = state.emit_arg_values(&args);
    let func_ref = state.import_func(&PYOBJECT_RICHCOMPARE_IMPORT);
    let compare_op = state.fb().ins().iconst(ir::types::I32, compare_op as i64);
    let call_inst = state
        .fb()
        .ins()
        .call(func_ref, &[arg_values[0].0, arg_values[1].0, compare_op]);
    state.release_arg_values(&arg_values);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

fn emit_identity_compare<'fb, E>(
    state: &mut impl OperationEmitState<'fb, E>,
    args: &[&E],
) -> ir::Value {
    let arg_values = state.emit_arg_values(&args);
    let cond = state.fb().ins().icmp(
        ir::condcodes::IntCC::Equal,
        arg_values[0].0,
        arg_values[1].0,
    );
    state.release_arg_values(&arg_values);
    state.emit_owned_bool_from_cond(cond)
}

fn emit_getattr<'fb, E>(
    op: &blockpy_intrinsics::GetAttr<E>,
    state: &mut impl OperationEmitState<'fb, E>,
) -> ir::Value {
    let arg_values = state.emit_arg_values(&[&op.value, &op.attr]);
    let pyobject_getattr_ref = state.ctx().pyobject_getattr_ref;
    let call_inst = state
        .fb()
        .ins()
        .call(pyobject_getattr_ref, &[arg_values[0].0, arg_values[1].0]);
    state.release_arg_values(&arg_values);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

fn emit_setattr<'fb, E>(
    op: &blockpy_intrinsics::SetAttr<E>,
    state: &mut impl OperationEmitState<'fb, E>,
) -> ir::Value {
    let arg_values = state.emit_arg_values(&[&op.value, &op.attr, &op.replacement]);
    let pyobject_setattr_ref = state.ctx().pyobject_setattr_ref;
    let call_inst = state.fb().ins().call(
        pyobject_setattr_ref,
        &[arg_values[0].0, arg_values[1].0, arg_values[2].0],
    );
    state.release_arg_values(&arg_values);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

fn emit_make_cell<'fb, E>(state: &mut impl OperationEmitState<'fb, E>, args: &[&E]) -> ir::Value {
    let arg_values = state.emit_arg_values(&args);
    let make_cell_ref = state.ctx().make_cell_ref;
    let call_inst = state.fb().ins().call(make_cell_ref, &[arg_values[0].0]);
    state.release_arg_values(&arg_values);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

fn emit_getitem<'fb, E>(state: &mut impl OperationEmitState<'fb, E>, args: &[&E]) -> ir::Value {
    state.emit_owned_func_call(state.ctx().pyobject_getitem_ref, &args)
}

fn emit_setitem<'fb, E>(state: &mut impl OperationEmitState<'fb, E>, args: &[&E]) -> ir::Value {
    state.emit_owned_func_call(state.ctx().pyobject_setitem_ref, &args)
}

fn emit_binop<'fb, E>(
    kind: blockpy_intrinsics::BinOpKind,
    state: &mut impl OperationEmitState<'fb, E>,
    args: &[&E],
) -> ir::Value {
    match kind {
        blockpy_intrinsics::BinOpKind::Add => {
            emit_positional_owned_call(&PYNUMBER_ADD_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::Sub => {
            emit_positional_owned_call(&PYNUMBER_SUBTRACT_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::Mul => {
            emit_positional_owned_call(&PYNUMBER_MULTIPLY_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::MatMul => {
            emit_positional_owned_call(&PYNUMBER_MATMUL_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::TrueDiv => {
            emit_positional_owned_call(&PYNUMBER_TRUE_DIVIDE_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::FloorDiv => {
            emit_positional_owned_call(&PYNUMBER_FLOOR_DIVIDE_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::Mod => {
            emit_positional_owned_call(&PYNUMBER_REMAINDER_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::Pow => emit_pow_like(&PYNUMBER_POWER_IMPORT, state, args),
        blockpy_intrinsics::BinOpKind::LShift => {
            emit_positional_owned_call(&PYNUMBER_LSHIFT_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::RShift => {
            emit_positional_owned_call(&PYNUMBER_RSHIFT_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::Or => {
            emit_positional_owned_call(&PYNUMBER_OR_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::Xor => {
            emit_positional_owned_call(&PYNUMBER_XOR_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::And => {
            emit_positional_owned_call(&PYNUMBER_AND_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::Eq => emit_richcompare(ffi::Py_EQ, state, args),
        blockpy_intrinsics::BinOpKind::Ne => emit_richcompare(ffi::Py_NE, state, args),
        blockpy_intrinsics::BinOpKind::Lt => emit_richcompare(ffi::Py_LT, state, args),
        blockpy_intrinsics::BinOpKind::Le => emit_richcompare(ffi::Py_LE, state, args),
        blockpy_intrinsics::BinOpKind::Gt => emit_richcompare(ffi::Py_GT, state, args),
        blockpy_intrinsics::BinOpKind::Ge => emit_richcompare(ffi::Py_GE, state, args),
        blockpy_intrinsics::BinOpKind::Contains => {
            emit_positional_bool_call(&PYSEQUENCE_CONTAINS_IMPORT, state, args)
        }
        blockpy_intrinsics::BinOpKind::Is => emit_identity_compare(state, args),
    }
}

fn emit_unary_op<'fb, E>(
    kind: blockpy_intrinsics::UnaryOpKind,
    state: &mut impl OperationEmitState<'fb, E>,
    args: &[&E],
) -> ir::Value {
    match kind {
        blockpy_intrinsics::UnaryOpKind::Pos => {
            emit_positional_owned_call(&PYNUMBER_POSITIVE_IMPORT, state, args)
        }
        blockpy_intrinsics::UnaryOpKind::Neg => {
            emit_positional_owned_call(&PYNUMBER_NEGATIVE_IMPORT, state, args)
        }
        blockpy_intrinsics::UnaryOpKind::Invert => {
            emit_positional_owned_call(&PYNUMBER_INVERT_IMPORT, state, args)
        }
        blockpy_intrinsics::UnaryOpKind::Not => {
            emit_positional_bool_call(&PYOBJECT_NOT_IMPORT, state, args)
        }
        blockpy_intrinsics::UnaryOpKind::Truth => {
            emit_positional_bool_call(&PYOBJECT_IS_TRUE_IMPORT, state, args)
        }
    }
}

fn emit_inplace_binop<'fb, E>(
    kind: blockpy_intrinsics::InplaceBinOpKind,
    state: &mut impl OperationEmitState<'fb, E>,
    args: &[&E],
) -> ir::Value {
    match kind {
        blockpy_intrinsics::InplaceBinOpKind::Add => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_ADD_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::Sub => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_SUBTRACT_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::Mul => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_MULTIPLY_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::MatMul => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_MATMUL_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::TrueDiv => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_TRUE_DIVIDE_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::FloorDiv => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_FLOOR_DIVIDE_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::Mod => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_REMAINDER_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::Pow => {
            emit_pow_like(&PYNUMBER_INPLACE_POWER_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::LShift => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_LSHIFT_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::RShift => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_RSHIFT_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::Or => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_OR_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::Xor => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_XOR_IMPORT, state, args)
        }
        blockpy_intrinsics::InplaceBinOpKind::And => {
            emit_positional_owned_call(&PYNUMBER_INPLACE_AND_IMPORT, state, args)
        }
    }
}

fn emit_load_name<'fb, E>(
    op: &blockpy_intrinsics::LoadName,
    state: &mut impl OperationEmitState<'fb, E>,
) -> ir::Value {
    let name_obj = state.emit_owned_string_constant(op.name.as_str());
    let func_ref = state.import_func(&DP_JIT_LOAD_GLOBAL_OBJ_IMPORT);
    let decref_ref = state.ctx().decref_ref;
    let globals_obj = state.ctx().consts.block_const;
    let call_inst = state.fb().ins().call(func_ref, &[globals_obj, name_obj]);
    state.fb().ins().call(decref_ref, &[name_obj]);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

fn emit_load_runtime<'fb, E>(
    op: &blockpy_intrinsics::LoadRuntime,
    state: &mut impl OperationEmitState<'fb, E>,
) -> ir::Value {
    let name_obj = state.emit_owned_string_constant(op.name.as_str());
    let func_ref = state.import_func(&DP_JIT_LOAD_RUNTIME_OBJ_IMPORT);
    let decref_ref = state.ctx().decref_ref;
    let call_inst = state.fb().ins().call(func_ref, &[name_obj]);
    state.fb().ins().call(decref_ref, &[name_obj]);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

fn emit_store_name<'fb, E>(
    op: &blockpy_intrinsics::StoreName<E>,
    state: &mut impl OperationEmitState<'fb, E>,
) -> ir::Value {
    let arg_values = state.emit_arg_values(&[&op.value]);
    let name_obj = state.emit_owned_string_constant(op.name.as_str());
    let func_ref = state.import_func(&DP_JIT_STORE_GLOBAL_IMPORT);
    let decref_ref = state.ctx().decref_ref;
    let globals_obj = state.ctx().consts.block_const;
    let call_inst = state
        .fb()
        .ins()
        .call(func_ref, &[globals_obj, name_obj, arg_values[0].0]);
    state.release_arg_values(&arg_values);
    state.fb().ins().call(decref_ref, &[name_obj]);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

fn emit_del_name<'fb, E>(
    op: &blockpy_intrinsics::DelName,
    state: &mut impl OperationEmitState<'fb, E>,
) -> ir::Value {
    let name_obj = state.emit_owned_string_constant(op.name.as_str());
    let func_ref = if op.quietly {
        state.import_func(&DP_JIT_DEL_QUIETLY_IMPORT)
    } else {
        state.import_func(&DP_JIT_PYOBJECT_DELITEM_IMPORT)
    };
    let decref_ref = state.ctx().decref_ref;
    let globals_obj = state.ctx().consts.block_const;
    let call_inst = state.fb().ins().call(func_ref, &[globals_obj, name_obj]);
    state.fb().ins().call(decref_ref, &[name_obj]);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

pub(super) fn emit_del_deref_raw_cell<'fb, E>(
    cell_obj: ir::Value,
    quietly: bool,
    state: &mut impl OperationEmitState<'fb, E>,
) -> ir::Value {
    let func_ref = state.import_func(if quietly {
        &DP_JIT_DEL_DEREF_QUIETLY_IMPORT
    } else {
        &DP_JIT_DEL_DEREF_IMPORT
    });
    let decref_ref = state.ctx().decref_ref;
    let call_inst = state.fb().ins().call(func_ref, &[cell_obj]);
    state.fb().ins().call(decref_ref, &[cell_obj]);
    let result = state.fb().inst_results(call_inst)[0];
    state.finish_owned_result(result)
}

pub(super) fn emit_operation<'fb, E>(
    operation: &blockpy_intrinsics::OperationDetail<E>,
    state: &mut impl OperationEmitState<'fb, E>,
) -> Option<ir::Value> {
    match operation {
        blockpy_intrinsics::OperationDetail::BinOp(op) => Some(emit_binop(
            op.kind,
            state,
            &[op.left.as_ref(), op.right.as_ref()],
        )),
        blockpy_intrinsics::OperationDetail::UnaryOp(op) => {
            Some(emit_unary_op(op.kind, state, &[op.operand.as_ref()]))
        }
        blockpy_intrinsics::OperationDetail::InplaceBinOp(op) => Some(emit_inplace_binop(
            op.kind,
            state,
            &[op.left.as_ref(), op.right.as_ref()],
        )),
        blockpy_intrinsics::OperationDetail::Call(_) => None,
        blockpy_intrinsics::OperationDetail::GetAttr(op) => Some(emit_getattr(op, state)),
        blockpy_intrinsics::OperationDetail::SetAttr(op) => Some(emit_setattr(op, state)),
        blockpy_intrinsics::OperationDetail::GetItem(op) => {
            Some(emit_getitem(state, &[op.value.as_ref(), op.index.as_ref()]))
        }
        blockpy_intrinsics::OperationDetail::SetItem(op) => Some(emit_setitem(
            state,
            &[
                op.value.as_ref(),
                op.index.as_ref(),
                op.replacement.as_ref(),
            ],
        )),
        blockpy_intrinsics::OperationDetail::DelItem(op) => Some(emit_positional_owned_call(
            &DP_JIT_PYOBJECT_DELITEM_IMPORT,
            state,
            &[op.value.as_ref(), op.index.as_ref()],
        )),
        blockpy_intrinsics::OperationDetail::LoadRuntime(op) => Some(emit_load_runtime(op, state)),
        blockpy_intrinsics::OperationDetail::LoadName(op) => Some(emit_load_name(op, state)),
        blockpy_intrinsics::OperationDetail::StoreName(op) => Some(emit_store_name(op, state)),
        blockpy_intrinsics::OperationDetail::DelName(op) => Some(emit_del_name(op, state)),
        blockpy_intrinsics::OperationDetail::LoadLocation(_) => None,
        blockpy_intrinsics::OperationDetail::MakeCell(op) => {
            Some(emit_make_cell(state, &[op.initial_value.as_ref()]))
        }
        blockpy_intrinsics::OperationDetail::CellRefForName(_) => None,
        blockpy_intrinsics::OperationDetail::CellRef(_) => None,
        blockpy_intrinsics::OperationDetail::MakeFunction(_) => None,
        blockpy_intrinsics::OperationDetail::StoreLocation(_) => None,
        blockpy_intrinsics::OperationDetail::DelLocation(_) => None,
    }
}

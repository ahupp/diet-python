use cranelift_jit::JITBuilder;
use std::ffi::c_void;
use std::ptr;

pub type ObjPtr = *mut c_void;
pub type IncrefFn = unsafe extern "C" fn(ObjPtr);
pub type DecrefFn = unsafe extern "C" fn(ObjPtr);
pub type CallOneArgFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
pub type CallTwoArgsFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
pub type CallVarArgsFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
pub type CallObjectFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
pub type CallWithKwFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
pub type GetRaisedExceptionFn = unsafe extern "C" fn() -> ObjPtr;
pub type GetArgItemFn = unsafe extern "C" fn(ObjPtr, i64) -> ObjPtr;
pub type MakeIntFn = unsafe extern "C" fn(i64) -> ObjPtr;
pub type MakeFloatFn = unsafe extern "C" fn(f64) -> ObjPtr;
pub type MakeBytesFn = unsafe extern "C" fn(*const u8, i64) -> ObjPtr;
pub type LoadNameFn = unsafe extern "C" fn(ObjPtr, *const u8, i64) -> ObjPtr;
pub type LoadLocalRawByNameFn = unsafe extern "C" fn(ObjPtr, *const u8, i64) -> ObjPtr;
pub type PyObjectGetAttrFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
pub type PyObjectSetAttrFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
pub type PyObjectGetItemFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
pub type PyObjectSetItemFn = unsafe extern "C" fn(ObjPtr, ObjPtr, ObjPtr) -> ObjPtr;
pub type PyObjectToI64Fn = unsafe extern "C" fn(ObjPtr) -> i64;
pub type DecodeLiteralBytesFn = unsafe extern "C" fn(*const u8, i64) -> ObjPtr;
pub type TupleNewFn = unsafe extern "C" fn(i64) -> ObjPtr;
pub type TupleSetItemFn = unsafe extern "C" fn(ObjPtr, i64, ObjPtr) -> i32;
pub type IsTrueFn = unsafe extern "C" fn(ObjPtr) -> i32;
pub type CompareEqFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> i32;
pub type CompareObjFn = unsafe extern "C" fn(ObjPtr, ObjPtr) -> ObjPtr;
pub type RaiseFromExcFn = unsafe extern "C" fn(ObjPtr) -> i32;

#[derive(Clone, Copy)]
pub struct SpecializedJitHooks {
    pub incref: IncrefFn,
    pub decref: DecrefFn,
    pub py_call_three: CallVarArgsFn,
    pub py_call_object: CallObjectFn,
    pub py_call_with_kw: CallWithKwFn,
    pub py_get_raised_exception: GetRaisedExceptionFn,
    pub get_arg_item: GetArgItemFn,
    pub make_int: MakeIntFn,
    pub make_float: MakeFloatFn,
    pub make_bytes: MakeBytesFn,
    pub load_name: LoadNameFn,
    pub load_local_raw_by_name: LoadLocalRawByNameFn,
    pub pyobject_getattr: PyObjectGetAttrFn,
    pub pyobject_setattr: PyObjectSetAttrFn,
    pub pyobject_getitem: PyObjectGetItemFn,
    pub pyobject_setitem: PyObjectSetItemFn,
    pub pyobject_to_i64: PyObjectToI64Fn,
    pub decode_literal_bytes: DecodeLiteralBytesFn,
    pub tuple_new: TupleNewFn,
    pub tuple_set_item: TupleSetItemFn,
    pub is_true: IsTrueFn,
    pub compare_eq_obj: CompareObjFn,
    pub compare_lt_obj: CompareObjFn,
    pub raise_from_exc: RaiseFromExcFn,
}

static mut DP_JIT_INCREF_FN: Option<IncrefFn> = None;
static mut DP_JIT_DECREF_FN: Option<DecrefFn> = None;
static mut DP_JIT_CALL_ONE_ARG_FN: Option<CallOneArgFn> = None;
static mut DP_JIT_CALL_TWO_ARGS_FN: Option<CallTwoArgsFn> = None;
static mut DP_JIT_CALL_VAR_ARGS_FN: Option<CallVarArgsFn> = None;
static mut DP_JIT_CALL_OBJECT_FN: Option<CallObjectFn> = None;
static mut DP_JIT_CALL_WITH_KW_FN: Option<CallWithKwFn> = None;
static mut DP_JIT_GET_RAISED_EXCEPTION_FN: Option<GetRaisedExceptionFn> = None;
static mut DP_JIT_GET_ARG_ITEM_FN: Option<GetArgItemFn> = None;
static mut DP_JIT_MAKE_INT_FN: Option<MakeIntFn> = None;
static mut DP_JIT_MAKE_FLOAT_FN: Option<MakeFloatFn> = None;
static mut DP_JIT_MAKE_BYTES_FN: Option<MakeBytesFn> = None;
static mut DP_JIT_LOAD_NAME_FN: Option<LoadNameFn> = None;
static mut DP_JIT_LOAD_LOCAL_RAW_BY_NAME_FN: Option<LoadLocalRawByNameFn> = None;
static mut DP_JIT_PYOBJECT_GETATTR_FN: Option<PyObjectGetAttrFn> = None;
static mut DP_JIT_PYOBJECT_SETATTR_FN: Option<PyObjectSetAttrFn> = None;
static mut DP_JIT_PYOBJECT_GETITEM_FN: Option<PyObjectGetItemFn> = None;
static mut DP_JIT_PYOBJECT_SETITEM_FN: Option<PyObjectSetItemFn> = None;
static mut DP_JIT_PYOBJECT_TO_I64_FN: Option<PyObjectToI64Fn> = None;
static mut DP_JIT_DECODE_LITERAL_BYTES_FN: Option<DecodeLiteralBytesFn> = None;
static mut DP_JIT_TUPLE_NEW_FN: Option<TupleNewFn> = None;
static mut DP_JIT_TUPLE_SET_ITEM_FN: Option<TupleSetItemFn> = None;
static mut DP_JIT_IS_TRUE_FN: Option<IsTrueFn> = None;
static mut DP_JIT_COMPARE_EQ_OBJ_FN: Option<CompareObjFn> = None;
static mut DP_JIT_COMPARE_LT_OBJ_FN: Option<CompareObjFn> = None;
static mut DP_JIT_RAISE_FROM_EXC_FN: Option<RaiseFromExcFn> = None;

pub unsafe fn set_smoke_call_one_hook(call_one_arg_fn: CallOneArgFn) {
    DP_JIT_CALL_ONE_ARG_FN = Some(call_one_arg_fn);
}

pub unsafe fn set_smoke_call_two_hook(call_two_args_fn: CallTwoArgsFn) {
    DP_JIT_CALL_TWO_ARGS_FN = Some(call_two_args_fn);
}

pub unsafe fn set_smoke_refcount_hooks(incref_fn: IncrefFn, decref_fn: DecrefFn) {
    DP_JIT_INCREF_FN = Some(incref_fn);
    DP_JIT_DECREF_FN = Some(decref_fn);
}

pub unsafe fn install_specialized_hooks(hooks: &SpecializedJitHooks) {
    DP_JIT_INCREF_FN = Some(hooks.incref);
    DP_JIT_DECREF_FN = Some(hooks.decref);
    DP_JIT_CALL_VAR_ARGS_FN = Some(hooks.py_call_three);
    DP_JIT_CALL_OBJECT_FN = Some(hooks.py_call_object);
    DP_JIT_CALL_WITH_KW_FN = Some(hooks.py_call_with_kw);
    DP_JIT_GET_RAISED_EXCEPTION_FN = Some(hooks.py_get_raised_exception);
    DP_JIT_GET_ARG_ITEM_FN = Some(hooks.get_arg_item);
    DP_JIT_MAKE_INT_FN = Some(hooks.make_int);
    DP_JIT_MAKE_FLOAT_FN = Some(hooks.make_float);
    DP_JIT_MAKE_BYTES_FN = Some(hooks.make_bytes);
    DP_JIT_LOAD_NAME_FN = Some(hooks.load_name);
    DP_JIT_LOAD_LOCAL_RAW_BY_NAME_FN = Some(hooks.load_local_raw_by_name);
    DP_JIT_PYOBJECT_GETATTR_FN = Some(hooks.pyobject_getattr);
    DP_JIT_PYOBJECT_SETATTR_FN = Some(hooks.pyobject_setattr);
    DP_JIT_PYOBJECT_GETITEM_FN = Some(hooks.pyobject_getitem);
    DP_JIT_PYOBJECT_SETITEM_FN = Some(hooks.pyobject_setitem);
    DP_JIT_PYOBJECT_TO_I64_FN = Some(hooks.pyobject_to_i64);
    DP_JIT_DECODE_LITERAL_BYTES_FN = Some(hooks.decode_literal_bytes);
    DP_JIT_TUPLE_NEW_FN = Some(hooks.tuple_new);
    DP_JIT_TUPLE_SET_ITEM_FN = Some(hooks.tuple_set_item);
    DP_JIT_IS_TRUE_FN = Some(hooks.is_true);
    DP_JIT_COMPARE_EQ_OBJ_FN = Some(hooks.compare_eq_obj);
    DP_JIT_COMPARE_LT_OBJ_FN = Some(hooks.compare_lt_obj);
    DP_JIT_RAISE_FROM_EXC_FN = Some(hooks.raise_from_exc);
}

pub unsafe extern "C" fn dp_jit_incref(obj: ObjPtr) {
    if let Some(func) = DP_JIT_INCREF_FN {
        func(obj);
    }
}

pub unsafe extern "C" fn dp_jit_decref(obj: ObjPtr) {
    if let Some(func) = DP_JIT_DECREF_FN {
        func(obj);
    }
}

pub unsafe extern "C" fn dp_jit_call_one_arg(callable: ObjPtr, arg: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_ONE_ARG_FN {
        return func(callable, arg);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_call_two_args(
    callable: ObjPtr,
    arg1: ObjPtr,
    arg2: ObjPtr,
) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_TWO_ARGS_FN {
        return func(callable, arg1, arg2);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_raise_from_exc(exc: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_RAISE_FROM_EXC_FN {
        return func(exc);
    }
    -1
}

pub unsafe extern "C" fn dp_jit_py_call_three(
    callable: ObjPtr,
    arg1: ObjPtr,
    arg2: ObjPtr,
    arg3: ObjPtr,
    _sentinel: ObjPtr,
) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_VAR_ARGS_FN {
        return func(callable, arg1, arg2, arg3);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_py_call_object(callable: ObjPtr, args: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_OBJECT_FN {
        return func(callable, args);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_py_call_with_kw(
    callable: ObjPtr,
    args: ObjPtr,
    kw: ObjPtr,
) -> ObjPtr {
    if let Some(func) = DP_JIT_CALL_WITH_KW_FN {
        return func(callable, args, kw);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_get_raised_exception() -> ObjPtr {
    if let Some(func) = DP_JIT_GET_RAISED_EXCEPTION_FN {
        return func();
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_get_arg_item(args: ObjPtr, index: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_GET_ARG_ITEM_FN {
        return func(args, index);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_make_int(value: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_MAKE_INT_FN {
        return func(value);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_make_float(value: f64) -> ObjPtr {
    if let Some(func) = DP_JIT_MAKE_FLOAT_FN {
        return func(value);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_make_bytes(data_ptr: *const u8, data_len: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_MAKE_BYTES_FN {
        return func(data_ptr, data_len);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_load_name(
    block: ObjPtr,
    name_ptr: *const u8,
    name_len: i64,
) -> ObjPtr {
    if let Some(func) = DP_JIT_LOAD_NAME_FN {
        return func(block, name_ptr, name_len);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_load_local_raw_by_name(
    frame_obj: ObjPtr,
    name_ptr: *const u8,
    name_len: i64,
) -> ObjPtr {
    if let Some(func) = DP_JIT_LOAD_LOCAL_RAW_BY_NAME_FN {
        return func(frame_obj, name_ptr, name_len);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_pyobject_getattr(obj: ObjPtr, attr: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_PYOBJECT_GETATTR_FN {
        return func(obj, attr);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_pyobject_setattr(
    obj: ObjPtr,
    attr: ObjPtr,
    value: ObjPtr,
) -> ObjPtr {
    if let Some(func) = DP_JIT_PYOBJECT_SETATTR_FN {
        return func(obj, attr, value);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_pyobject_getitem(obj: ObjPtr, key: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_PYOBJECT_GETITEM_FN {
        return func(obj, key);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_pyobject_setitem(
    obj: ObjPtr,
    key: ObjPtr,
    value: ObjPtr,
) -> ObjPtr {
    if let Some(func) = DP_JIT_PYOBJECT_SETITEM_FN {
        return func(obj, key, value);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_pyobject_to_i64(value: ObjPtr) -> i64 {
    if let Some(func) = DP_JIT_PYOBJECT_TO_I64_FN {
        return func(value);
    }
    i64::MIN
}

pub unsafe extern "C" fn dp_jit_decode_literal_bytes(data_ptr: *const u8, data_len: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_DECODE_LITERAL_BYTES_FN {
        return func(data_ptr, data_len);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_tuple_new(size: i64) -> ObjPtr {
    if let Some(func) = DP_JIT_TUPLE_NEW_FN {
        return func(size);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_tuple_set_item(tuple_obj: ObjPtr, index: i64, item: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_TUPLE_SET_ITEM_FN {
        return func(tuple_obj, index, item);
    }
    -1
}

pub unsafe extern "C" fn dp_jit_is_true(value: ObjPtr) -> i32 {
    if let Some(func) = DP_JIT_IS_TRUE_FN {
        return func(value);
    }
    -1
}

pub unsafe extern "C" fn dp_jit_compare_eq_obj(lhs: ObjPtr, rhs: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_COMPARE_EQ_OBJ_FN {
        return func(lhs, rhs);
    }
    ptr::null_mut()
}

pub unsafe extern "C" fn dp_jit_compare_lt_obj(lhs: ObjPtr, rhs: ObjPtr) -> ObjPtr {
    if let Some(func) = DP_JIT_COMPARE_LT_OBJ_FN {
        return func(lhs, rhs);
    }
    ptr::null_mut()
}

pub fn register_specialized_jit_symbols(builder: &mut JITBuilder) {
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol(
        "PyObject_CallFunctionObjArgs",
        dp_jit_py_call_three as *const u8,
    );
    builder.symbol("PyObject_CallObject", dp_jit_py_call_object as *const u8);
    builder.symbol(
        "dp_jit_py_call_with_kw",
        dp_jit_py_call_with_kw as *const u8,
    );
    builder.symbol(
        "PyErr_GetRaisedException",
        dp_jit_get_raised_exception as *const u8,
    );
    builder.symbol("dp_jit_get_arg_item", dp_jit_get_arg_item as *const u8);
    builder.symbol("dp_jit_make_int", dp_jit_make_int as *const u8);
    builder.symbol("dp_jit_make_float", dp_jit_make_float as *const u8);
    builder.symbol("dp_jit_make_bytes", dp_jit_make_bytes as *const u8);
    builder.symbol("dp_jit_load_name", dp_jit_load_name as *const u8);
    builder.symbol(
        "dp_jit_load_local_raw_by_name",
        dp_jit_load_local_raw_by_name as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_getattr",
        dp_jit_pyobject_getattr as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_setattr",
        dp_jit_pyobject_setattr as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_getitem",
        dp_jit_pyobject_getitem as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_setitem",
        dp_jit_pyobject_setitem as *const u8,
    );
    builder.symbol(
        "dp_jit_pyobject_to_i64",
        dp_jit_pyobject_to_i64 as *const u8,
    );
    builder.symbol(
        "dp_jit_decode_literal_bytes",
        dp_jit_decode_literal_bytes as *const u8,
    );
    builder.symbol("dp_jit_tuple_new", dp_jit_tuple_new as *const u8);
    builder.symbol("dp_jit_tuple_set_item", dp_jit_tuple_set_item as *const u8);
    builder.symbol("dp_jit_is_true", dp_jit_is_true as *const u8);
    builder.symbol("dp_jit_compare_eq_obj", dp_jit_compare_eq_obj as *const u8);
    builder.symbol("dp_jit_compare_lt_obj", dp_jit_compare_lt_obj as *const u8);
    builder.symbol("dp_jit_raise_from_exc", dp_jit_raise_from_exc as *const u8);
}

pub fn register_smoke_call_one_symbols(builder: &mut JITBuilder) {
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol("dp_jit_call_one_arg", dp_jit_call_one_arg as *const u8);
}

pub fn register_smoke_call_two_symbols(builder: &mut JITBuilder) {
    builder.symbol("dp_jit_incref", dp_jit_incref as *const u8);
    builder.symbol("dp_jit_decref", dp_jit_decref as *const u8);
    builder.symbol("dp_jit_call_two_args", dp_jit_call_two_args as *const u8);
}

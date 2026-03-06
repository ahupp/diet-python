use cranelift_jit::JITBuilder;
use libc;
use pyo3::ffi;
use std::ffi::{c_void, CString};
use std::ptr;
use std::sync::OnceLock;

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
    DP_JIT_RAISE_FROM_EXC_FN = Some(hooks.raise_from_exc);
}

unsafe extern "C" fn jit_incref_hook(obj: ObjPtr) {
    if !obj.is_null() {
        ffi::Py_INCREF(obj as *mut ffi::PyObject);
    }
}

unsafe extern "C" fn jit_decref_hook(obj: ObjPtr) {
    if !obj.is_null() {
        ffi::Py_DECREF(obj as *mut ffi::PyObject);
    }
}

unsafe extern "C" fn py_call_three_hook(
    callable: ObjPtr,
    arg1: ObjPtr,
    arg2: ObjPtr,
    arg3: ObjPtr,
) -> ObjPtr {
    ffi::PyObject_CallFunctionObjArgs(
        callable as *mut ffi::PyObject,
        arg1 as *mut ffi::PyObject,
        arg2 as *mut ffi::PyObject,
        arg3 as *mut ffi::PyObject,
        ptr::null_mut::<ffi::PyObject>(),
    ) as ObjPtr
}

unsafe extern "C" fn py_call_object_hook(callable: ObjPtr, args: ObjPtr) -> ObjPtr {
    ffi::PyObject_CallObject(callable as *mut ffi::PyObject, args as *mut ffi::PyObject) as ObjPtr
}

unsafe extern "C" fn py_call_with_kw_hook(callable: ObjPtr, args: ObjPtr, kwargs: ObjPtr) -> ObjPtr {
    ffi::PyObject_Call(
        callable as *mut ffi::PyObject,
        args as *mut ffi::PyObject,
        kwargs as *mut ffi::PyObject,
    ) as ObjPtr
}

unsafe extern "C" fn py_get_raised_exception_hook() -> ObjPtr {
    ffi::PyErr_GetRaisedException() as ObjPtr
}

unsafe extern "C" fn get_arg_item_hook(args: ObjPtr, index: i64) -> ObjPtr {
    if args.is_null() {
        return ptr::null_mut();
    }
    ffi::PySequence_GetItem(args as *mut ffi::PyObject, index as ffi::Py_ssize_t) as ObjPtr
}

unsafe extern "C" fn make_int_hook(value: i64) -> ObjPtr {
    ffi::PyLong_FromLongLong(value as std::ffi::c_longlong) as ObjPtr
}

unsafe extern "C" fn make_float_hook(value: f64) -> ObjPtr {
    ffi::PyFloat_FromDouble(value) as ObjPtr
}

unsafe extern "C" fn make_bytes_hook(data_ptr: *const u8, data_len: i64) -> ObjPtr {
    if data_ptr.is_null() || data_len < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_make_bytes\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyBytes_FromStringAndSize(data_ptr as *const i8, data_len as ffi::Py_ssize_t) as ObjPtr
}

unsafe extern "C" fn load_name_hook(
    globals_obj: ObjPtr,
    name_ptr: *const u8,
    name_len: i64,
) -> ObjPtr {
    if globals_obj.is_null() || name_ptr.is_null() || name_len < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_load_name\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    let name_obj = ffi::PyUnicode_DecodeUTF8(
        name_ptr as *const i8,
        name_len as ffi::Py_ssize_t,
        b"strict\0".as_ptr() as *const i8,
    );
    if name_obj.is_null() {
        return ptr::null_mut();
    }
    ffi::Py_INCREF(globals_obj as *mut ffi::PyObject);
    let builtins_dict = ffi::PyEval_GetBuiltins();
    if builtins_dict.is_null() {
        ffi::Py_DECREF(globals_obj as *mut ffi::PyObject);
        ffi::Py_DECREF(name_obj);
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"PyEval_GetBuiltins returned null\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    let load_global = ffi::PyDict_GetItemString(
        builtins_dict as *mut ffi::PyObject,
        b"__dp_load_global\0".as_ptr() as *const i8,
    );
    if load_global.is_null() {
        ffi::Py_DECREF(globals_obj as *mut ffi::PyObject);
        ffi::Py_DECREF(name_obj);
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"missing builtins.__dp_load_global\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::Py_INCREF(load_global);
    let result = ffi::PyObject_CallFunctionObjArgs(
        load_global,
        globals_obj as *mut ffi::PyObject,
        name_obj,
        ptr::null_mut::<ffi::PyObject>(),
    );
    ffi::Py_DECREF(load_global);
    ffi::Py_DECREF(globals_obj as *mut ffi::PyObject);
    ffi::Py_DECREF(name_obj);
    result as ObjPtr
}

unsafe extern "C" fn load_local_raw_by_name_hook(
    owner: ObjPtr,
    name_ptr: *const u8,
    name_len: i64,
) -> ObjPtr {
    if owner.is_null() || name_ptr.is_null() || name_len < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_load_local_raw_by_name\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    let name_obj = ffi::PyUnicode_DecodeUTF8(
        name_ptr as *const i8,
        name_len as ffi::Py_ssize_t,
        b"strict\0".as_ptr() as *const i8,
    );
    if name_obj.is_null() {
        return ptr::null_mut();
    }
    let builtins_dict = ffi::PyEval_GetBuiltins();
    if builtins_dict.is_null() {
        ffi::Py_DECREF(name_obj);
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"PyEval_GetBuiltins returned null\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    let load_local_raw = ffi::PyDict_GetItemString(
        builtins_dict as *mut ffi::PyObject,
        b"__dp_load_local_raw\0".as_ptr() as *const i8,
    );
    if load_local_raw.is_null() {
        ffi::Py_DECREF(name_obj);
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"missing builtins.__dp_load_local_raw\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::Py_INCREF(load_local_raw);
    let result = ffi::PyObject_CallFunctionObjArgs(
        load_local_raw,
        owner as *mut ffi::PyObject,
        name_obj,
        ptr::null_mut::<ffi::PyObject>(),
    );
    ffi::Py_DECREF(load_local_raw);
    ffi::Py_DECREF(name_obj);
    result as ObjPtr
}

unsafe extern "C" fn pyobject_getattr_hook(obj: ObjPtr, attr: ObjPtr) -> ObjPtr {
    if obj.is_null() || attr.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_pyobject_getattr\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyObject_GetAttr(obj as *mut ffi::PyObject, attr as *mut ffi::PyObject) as ObjPtr
}

unsafe extern "C" fn pyobject_setattr_hook(obj: ObjPtr, attr: ObjPtr, value: ObjPtr) -> ObjPtr {
    if obj.is_null() || attr.is_null() || value.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_pyobject_setattr\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    let rc = ffi::PyObject_SetAttr(
        obj as *mut ffi::PyObject,
        attr as *mut ffi::PyObject,
        value as *mut ffi::PyObject,
    );
    if rc == 0 {
        let none = ffi::Py_None();
        ffi::Py_INCREF(none);
        none as ObjPtr
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn pyobject_getitem_hook(obj: ObjPtr, key: ObjPtr) -> ObjPtr {
    if obj.is_null() || key.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_pyobject_getitem\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyObject_GetItem(obj as *mut ffi::PyObject, key as *mut ffi::PyObject) as ObjPtr
}

unsafe extern "C" fn pyobject_setitem_hook(obj: ObjPtr, key: ObjPtr, value: ObjPtr) -> ObjPtr {
    if obj.is_null() || key.is_null() || value.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_pyobject_setitem\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    let rc = ffi::PyObject_SetItem(
        obj as *mut ffi::PyObject,
        key as *mut ffi::PyObject,
        value as *mut ffi::PyObject,
    );
    if rc == 0 {
        let none = ffi::Py_None();
        ffi::Py_INCREF(none);
        none as ObjPtr
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn pyobject_to_i64_hook(value: ObjPtr) -> i64 {
    if value.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid null value for dp_jit_pyobject_to_i64\0".as_ptr() as *const i8,
        );
        return i64::MIN;
    }
    let idx_obj = ffi::PyNumber_Index(value as *mut ffi::PyObject);
    if idx_obj.is_null() {
        return i64::MIN;
    }
    let out = ffi::PyLong_AsLongLong(idx_obj);
    ffi::Py_DECREF(idx_obj);
    if out == -1 && !ffi::PyErr_Occurred().is_null() {
        i64::MIN
    } else {
        out as i64
    }
}

unsafe extern "C" fn decode_literal_bytes_hook(data_ptr: *const u8, data_len: i64) -> ObjPtr {
    if data_ptr.is_null() || data_len < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_decode_literal_bytes\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyUnicode_DecodeUTF8(
        data_ptr as *const i8,
        data_len as ffi::Py_ssize_t,
        b"surrogatepass\0".as_ptr() as *const i8,
    ) as ObjPtr
}

unsafe extern "C" fn tuple_new_hook(size: i64) -> ObjPtr {
    if size < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid tuple size in JIT\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyTuple_New(size as ffi::Py_ssize_t) as ObjPtr
}

unsafe extern "C" fn tuple_set_item_hook(tuple_obj: ObjPtr, index: i64, value: ObjPtr) -> i32 {
    if tuple_obj.is_null() || value.is_null() || index < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid tuple_set_item arguments in JIT\0".as_ptr() as *const i8,
        );
        return -1;
    }
    ffi::PyTuple_SetItem(
        tuple_obj as *mut ffi::PyObject,
        index as ffi::Py_ssize_t,
        value as *mut ffi::PyObject,
    )
}

unsafe extern "C" fn is_true_hook(value: ObjPtr) -> i32 {
    if value.is_null() {
        return -1;
    }
    ffi::PyObject_IsTrue(value as *mut ffi::PyObject)
}

unsafe extern "C" fn raise_from_exc_hook(exc: ObjPtr) -> i32 {
    if exc.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"missing exception for dp_jit_raise_from_exc\0".as_ptr() as *const i8,
        );
        return -1;
    }
    let exc_obj = exc as *mut ffi::PyObject;
    let typ = ffi::PyExceptionInstance_Class(exc_obj);
    if typ.is_null() {
        return -1;
    }
    ffi::PyErr_SetObject(typ, exc_obj);
    ffi::Py_DECREF(typ);
    0
}

pub fn default_specialized_hooks() -> SpecializedJitHooks {
    SpecializedJitHooks {
        incref: jit_incref_hook,
        decref: jit_decref_hook,
        py_call_three: py_call_three_hook,
        py_call_object: py_call_object_hook,
        py_call_with_kw: py_call_with_kw_hook,
        py_get_raised_exception: py_get_raised_exception_hook,
        get_arg_item: get_arg_item_hook,
        make_int: make_int_hook,
        make_float: make_float_hook,
        make_bytes: make_bytes_hook,
        load_name: load_name_hook,
        load_local_raw_by_name: load_local_raw_by_name_hook,
        pyobject_getattr: pyobject_getattr_hook,
        pyobject_setattr: pyobject_setattr_hook,
        pyobject_getitem: pyobject_getitem_hook,
        pyobject_setitem: pyobject_setitem_hook,
        pyobject_to_i64: pyobject_to_i64_hook,
        decode_literal_bytes: decode_literal_bytes_hook,
        tuple_new: tuple_new_hook,
        tuple_set_item: tuple_set_item_hook,
        is_true: is_true_hook,
        raise_from_exc: raise_from_exc_hook,
    }
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

unsafe extern "C" fn pyobject_richcompare_wrapper(lhs: ObjPtr, rhs: ObjPtr, op: i32) -> ObjPtr {
    if lhs.is_null() || rhs.is_null() {
        return ptr::null_mut();
    }
    type Func = unsafe extern "C" fn(*mut ffi::PyObject, *mut ffi::PyObject, i32) -> *mut ffi::PyObject;
    static SYMBOL: OnceLock<usize> = OnceLock::new();
    let symbol = *SYMBOL.get_or_init(|| unsafe {
        load_python_capi_symbol(b"PyObject_RichCompare\0")
    });
    if symbol == 0 {
        return ptr::null_mut();
    }
    let func: Func = unsafe { std::mem::transmute(symbol) };
    func(lhs as *mut ffi::PyObject, rhs as *mut ffi::PyObject, op) as ObjPtr
}

unsafe fn load_python_capi_symbol(name: &'static [u8]) -> usize {
    libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr() as *const i8) as usize
}

macro_rules! define_unary_obj_wrapper {
    ($fn_name:ident, $symbol:literal) => {
        unsafe extern "C" fn $fn_name(value: ObjPtr) -> ObjPtr {
            if value.is_null() {
                return ptr::null_mut();
            }
            type Func = unsafe extern "C" fn(*mut ffi::PyObject) -> *mut ffi::PyObject;
            static SYMBOL: OnceLock<usize> = OnceLock::new();
            let symbol = *SYMBOL.get_or_init(|| unsafe {
                load_python_capi_symbol(concat!($symbol, "\0").as_bytes())
            });
            if symbol == 0 {
                return ptr::null_mut();
            }
            let func: Func = unsafe { std::mem::transmute(symbol) };
            func(value as *mut ffi::PyObject) as ObjPtr
        }
    };
}

macro_rules! define_unary_i32_wrapper {
    ($fn_name:ident, $symbol:literal) => {
        unsafe extern "C" fn $fn_name(value: ObjPtr) -> i32 {
            if value.is_null() {
                return -1;
            }
            type Func = unsafe extern "C" fn(*mut ffi::PyObject) -> i32;
            static SYMBOL: OnceLock<usize> = OnceLock::new();
            let symbol = *SYMBOL.get_or_init(|| unsafe {
                load_python_capi_symbol(concat!($symbol, "\0").as_bytes())
            });
            if symbol == 0 {
                return -1;
            }
            let func: Func = unsafe { std::mem::transmute(symbol) };
            func(value as *mut ffi::PyObject)
        }
    };
}

macro_rules! define_binary_obj_wrapper {
    ($fn_name:ident, $symbol:literal) => {
        unsafe extern "C" fn $fn_name(lhs: ObjPtr, rhs: ObjPtr) -> ObjPtr {
            if lhs.is_null() || rhs.is_null() {
                return ptr::null_mut();
            }
            type Func =
                unsafe extern "C" fn(*mut ffi::PyObject, *mut ffi::PyObject) -> *mut ffi::PyObject;
            static SYMBOL: OnceLock<usize> = OnceLock::new();
            let symbol = *SYMBOL.get_or_init(|| unsafe {
                load_python_capi_symbol(concat!($symbol, "\0").as_bytes())
            });
            if symbol == 0 {
                return ptr::null_mut();
            }
            let func: Func = unsafe { std::mem::transmute(symbol) };
            func(lhs as *mut ffi::PyObject, rhs as *mut ffi::PyObject) as ObjPtr
        }
    };
}

macro_rules! define_binary_i32_wrapper {
    ($fn_name:ident, $symbol:literal) => {
        unsafe extern "C" fn $fn_name(lhs: ObjPtr, rhs: ObjPtr) -> i32 {
            if lhs.is_null() || rhs.is_null() {
                return -1;
            }
            type Func = unsafe extern "C" fn(*mut ffi::PyObject, *mut ffi::PyObject) -> i32;
            static SYMBOL: OnceLock<usize> = OnceLock::new();
            let symbol = *SYMBOL.get_or_init(|| unsafe {
                load_python_capi_symbol(concat!($symbol, "\0").as_bytes())
            });
            if symbol == 0 {
                return -1;
            }
            let func: Func = unsafe { std::mem::transmute(symbol) };
            func(lhs as *mut ffi::PyObject, rhs as *mut ffi::PyObject)
        }
    };
}

macro_rules! define_ternary_obj_wrapper {
    ($fn_name:ident, $symbol:literal) => {
        unsafe extern "C" fn $fn_name(lhs: ObjPtr, rhs: ObjPtr, third: ObjPtr) -> ObjPtr {
            if lhs.is_null() || rhs.is_null() || third.is_null() {
                return ptr::null_mut();
            }
            type Func = unsafe extern "C" fn(
                *mut ffi::PyObject,
                *mut ffi::PyObject,
                *mut ffi::PyObject,
            ) -> *mut ffi::PyObject;
            static SYMBOL: OnceLock<usize> = OnceLock::new();
            let symbol = *SYMBOL.get_or_init(|| unsafe {
                load_python_capi_symbol(concat!($symbol, "\0").as_bytes())
            });
            if symbol == 0 {
                return ptr::null_mut();
            }
            let func: Func = unsafe { std::mem::transmute(symbol) };
            func(
                lhs as *mut ffi::PyObject,
                rhs as *mut ffi::PyObject,
                third as *mut ffi::PyObject,
            ) as ObjPtr
        }
    };
}

define_binary_i32_wrapper!(pysequence_contains_wrapper, "PySequence_Contains");
define_unary_i32_wrapper!(pyobject_not_wrapper, "PyObject_Not");
define_unary_i32_wrapper!(pyobject_is_true_wrapper, "PyObject_IsTrue");
define_binary_obj_wrapper!(pynumber_add_wrapper, "PyNumber_Add");
define_binary_obj_wrapper!(pynumber_subtract_wrapper, "PyNumber_Subtract");
define_binary_obj_wrapper!(pynumber_multiply_wrapper, "PyNumber_Multiply");
define_binary_obj_wrapper!(pynumber_matrix_multiply_wrapper, "PyNumber_MatrixMultiply");
define_binary_obj_wrapper!(pynumber_true_divide_wrapper, "PyNumber_TrueDivide");
define_binary_obj_wrapper!(pynumber_floor_divide_wrapper, "PyNumber_FloorDivide");
define_binary_obj_wrapper!(pynumber_remainder_wrapper, "PyNumber_Remainder");
define_ternary_obj_wrapper!(pynumber_power_wrapper, "PyNumber_Power");
define_binary_obj_wrapper!(pynumber_lshift_wrapper, "PyNumber_Lshift");
define_binary_obj_wrapper!(pynumber_rshift_wrapper, "PyNumber_Rshift");
define_binary_obj_wrapper!(pynumber_or_wrapper, "PyNumber_Or");
define_binary_obj_wrapper!(pynumber_xor_wrapper, "PyNumber_Xor");
define_binary_obj_wrapper!(pynumber_and_wrapper, "PyNumber_And");
define_binary_obj_wrapper!(pynumber_inplace_add_wrapper, "PyNumber_InPlaceAdd");
define_binary_obj_wrapper!(pynumber_inplace_subtract_wrapper, "PyNumber_InPlaceSubtract");
define_binary_obj_wrapper!(pynumber_inplace_multiply_wrapper, "PyNumber_InPlaceMultiply");
define_binary_obj_wrapper!(
    pynumber_inplace_matrix_multiply_wrapper,
    "PyNumber_InPlaceMatrixMultiply"
);
define_binary_obj_wrapper!(
    pynumber_inplace_true_divide_wrapper,
    "PyNumber_InPlaceTrueDivide"
);
define_binary_obj_wrapper!(
    pynumber_inplace_floor_divide_wrapper,
    "PyNumber_InPlaceFloorDivide"
);
define_binary_obj_wrapper!(
    pynumber_inplace_remainder_wrapper,
    "PyNumber_InPlaceRemainder"
);
define_ternary_obj_wrapper!(pynumber_inplace_power_wrapper, "PyNumber_InPlacePower");
define_binary_obj_wrapper!(pynumber_inplace_lshift_wrapper, "PyNumber_InPlaceLshift");
define_binary_obj_wrapper!(pynumber_inplace_rshift_wrapper, "PyNumber_InPlaceRshift");
define_binary_obj_wrapper!(pynumber_inplace_or_wrapper, "PyNumber_InPlaceOr");
define_binary_obj_wrapper!(pynumber_inplace_xor_wrapper, "PyNumber_InPlaceXor");
define_binary_obj_wrapper!(pynumber_inplace_and_wrapper, "PyNumber_InPlaceAnd");
define_unary_obj_wrapper!(pynumber_positive_wrapper, "PyNumber_Positive");
define_unary_obj_wrapper!(pynumber_negative_wrapper, "PyNumber_Negative");
define_unary_obj_wrapper!(pynumber_invert_wrapper, "PyNumber_Invert");

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
    builder.symbol("dp_jit_raise_from_exc", dp_jit_raise_from_exc as *const u8);
    builder.symbol("PyObject_RichCompare", pyobject_richcompare_wrapper as *const u8);
    builder.symbol("PySequence_Contains", pysequence_contains_wrapper as *const u8);
    builder.symbol("PyObject_Not", pyobject_not_wrapper as *const u8);
    builder.symbol("PyObject_IsTrue", pyobject_is_true_wrapper as *const u8);
    builder.symbol("PyNumber_Add", pynumber_add_wrapper as *const u8);
    builder.symbol("PyNumber_Subtract", pynumber_subtract_wrapper as *const u8);
    builder.symbol("PyNumber_Multiply", pynumber_multiply_wrapper as *const u8);
    builder.symbol("PyNumber_MatrixMultiply", pynumber_matrix_multiply_wrapper as *const u8);
    builder.symbol("PyNumber_TrueDivide", pynumber_true_divide_wrapper as *const u8);
    builder.symbol("PyNumber_FloorDivide", pynumber_floor_divide_wrapper as *const u8);
    builder.symbol("PyNumber_Remainder", pynumber_remainder_wrapper as *const u8);
    builder.symbol("PyNumber_Power", pynumber_power_wrapper as *const u8);
    builder.symbol("PyNumber_Lshift", pynumber_lshift_wrapper as *const u8);
    builder.symbol("PyNumber_Rshift", pynumber_rshift_wrapper as *const u8);
    builder.symbol("PyNumber_Or", pynumber_or_wrapper as *const u8);
    builder.symbol("PyNumber_Xor", pynumber_xor_wrapper as *const u8);
    builder.symbol("PyNumber_And", pynumber_and_wrapper as *const u8);
    builder.symbol("PyNumber_InPlaceAdd", pynumber_inplace_add_wrapper as *const u8);
    builder.symbol(
        "PyNumber_InPlaceSubtract",
        pynumber_inplace_subtract_wrapper as *const u8,
    );
    builder.symbol(
        "PyNumber_InPlaceMultiply",
        pynumber_inplace_multiply_wrapper as *const u8,
    );
    builder.symbol(
        "PyNumber_InPlaceMatrixMultiply",
        pynumber_inplace_matrix_multiply_wrapper as *const u8,
    );
    builder.symbol(
        "PyNumber_InPlaceTrueDivide",
        pynumber_inplace_true_divide_wrapper as *const u8,
    );
    builder.symbol(
        "PyNumber_InPlaceFloorDivide",
        pynumber_inplace_floor_divide_wrapper as *const u8,
    );
    builder.symbol(
        "PyNumber_InPlaceRemainder",
        pynumber_inplace_remainder_wrapper as *const u8,
    );
    builder.symbol("PyNumber_InPlacePower", pynumber_inplace_power_wrapper as *const u8);
    builder.symbol(
        "PyNumber_InPlaceLshift",
        pynumber_inplace_lshift_wrapper as *const u8,
    );
    builder.symbol(
        "PyNumber_InPlaceRshift",
        pynumber_inplace_rshift_wrapper as *const u8,
    );
    builder.symbol("PyNumber_InPlaceOr", pynumber_inplace_or_wrapper as *const u8);
    builder.symbol("PyNumber_InPlaceXor", pynumber_inplace_xor_wrapper as *const u8);
    builder.symbol("PyNumber_InPlaceAnd", pynumber_inplace_and_wrapper as *const u8);
    builder.symbol("PyNumber_Positive", pynumber_positive_wrapper as *const u8);
    builder.symbol("PyNumber_Negative", pynumber_negative_wrapper as *const u8);
    builder.symbol("PyNumber_Invert", pynumber_invert_wrapper as *const u8);
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

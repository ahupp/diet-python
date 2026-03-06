use log::info;
use super::*;
use crate::jit::{self, ClifPlan};
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::time::Instant;

unsafe extern "C" {
    fn PyFunction_SetVectorcall(
        func: *mut ffi::PyFunctionObject,
        vectorcall: ffi::vectorcallfunc,
    );
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn set_runtime_error<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(ffi::PyExc_RuntimeError, CString::new(msg).unwrap().as_ptr());
    }
    Err(())
}

pub(crate) fn set_name_error<T>(name: &str) -> Result<T, ()> {
    unsafe {
        let msg = CString::new(format!("name '{name}' is not defined")).unwrap();
        ffi::PyErr_SetString(ffi::PyExc_NameError, msg.as_ptr());
    }
    Err(())
}

const CLIF_VECTORCALL_CAPSULE_NAME: &[u8] = b"soac.clif_vectorcall_data\0";
const CLIF_VECTORCALL_ATTR: &[u8] = b"__dp_clif_vectorcall_data\0";

struct ClifFunctionData {
    plan: ClifPlan,
    module_name: String,
    qualname: String,
    true_obj: *mut ffi::PyObject,
    false_obj: *mut ffi::PyObject,
    build_entry_args_obj: *mut ffi::PyObject,
    materialize_entry_obj: *mut ffi::PyObject,
    compiled_handle: *mut c_void,
    compiled_vectorcall_handle: *mut c_void,
    compiled_vectorcall_entry: Option<jit::VectorcallEntryFn>,
}

unsafe fn free_clif_function_data(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let data = unsafe { Box::from_raw(ptr as *mut ClifFunctionData) };
    if !data.true_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.true_obj) };
    }
    if !data.false_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.false_obj) };
    }
    if !data.build_entry_args_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.build_entry_args_obj) };
    }
    if !data.materialize_entry_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.materialize_entry_obj) };
    }
    unsafe { jit::free_cranelift_run_bb_specialized_cached(data.compiled_handle) };
    unsafe { jit::free_cranelift_vectorcall_trampoline(data.compiled_vectorcall_handle) };
}

unsafe extern "C" fn free_clif_vectorcall_capsule(capsule: *mut ffi::PyObject) {
    if capsule.is_null() {
        return;
    }
    let ptr = unsafe {
        ffi::PyCapsule_GetPointer(
            capsule,
            CLIF_VECTORCALL_CAPSULE_NAME.as_ptr() as *const c_char,
        )
    };
    if !ptr.is_null() {
        unsafe { free_clif_function_data(ptr) };
    } else if !unsafe { ffi::PyErr_Occurred() }.is_null() {
        unsafe { ffi::PyErr_Clear() };
    }
}

unsafe fn owned_ptr_to_bound<'py>(
    py: Python<'py>,
    ptr: *mut ffi::PyObject,
) -> PyResult<Bound<'py, PyAny>> {
    Bound::from_owned_ptr_or_opt(py, ptr).ok_or_else(|| PyErr::fetch(py))
}

unsafe extern "C" fn jit_incref_hook(obj: *mut c_void) {
    if !obj.is_null() {
        ffi::Py_INCREF(obj as *mut ffi::PyObject);
    }
}

unsafe extern "C" fn jit_decref_hook(obj: *mut c_void) {
    if !obj.is_null() {
        ffi::Py_DECREF(obj as *mut ffi::PyObject);
    }
}

unsafe extern "C" fn py_call_three_hook(
    callable: *mut c_void,
    arg1: *mut c_void,
    arg2: *mut c_void,
    arg3: *mut c_void,
) -> *mut c_void {
    ffi::PyObject_CallFunctionObjArgs(
        callable as *mut ffi::PyObject,
        arg1 as *mut ffi::PyObject,
        arg2 as *mut ffi::PyObject,
        arg3 as *mut ffi::PyObject,
        ptr::null_mut::<ffi::PyObject>(),
    ) as *mut c_void
}

unsafe extern "C" fn py_call_object_hook(callable: *mut c_void, args: *mut c_void) -> *mut c_void {
    ffi::PyObject_CallObject(callable as *mut ffi::PyObject, args as *mut ffi::PyObject)
        as *mut c_void
}

unsafe extern "C" fn py_call_with_kw_hook(
    callable: *mut c_void,
    args: *mut c_void,
    kwargs: *mut c_void,
) -> *mut c_void {
    ffi::PyObject_Call(
        callable as *mut ffi::PyObject,
        args as *mut ffi::PyObject,
        kwargs as *mut ffi::PyObject,
    ) as *mut c_void
}

unsafe extern "C" fn py_get_raised_exception_hook() -> *mut c_void {
    ffi::PyErr_GetRaisedException() as *mut c_void
}

unsafe extern "C" fn get_arg_item_hook(args: *mut c_void, index: i64) -> *mut c_void {
    if args.is_null() {
        return ptr::null_mut();
    }
    ffi::PySequence_GetItem(args as *mut ffi::PyObject, index as ffi::Py_ssize_t) as *mut c_void
}

unsafe extern "C" fn make_int_hook(value: i64) -> *mut c_void {
    ffi::PyLong_FromLongLong(value as std::ffi::c_longlong) as *mut c_void
}

unsafe extern "C" fn make_float_hook(value: f64) -> *mut c_void {
    ffi::PyFloat_FromDouble(value) as *mut c_void
}

unsafe extern "C" fn make_bytes_hook(data_ptr: *const u8, data_len: i64) -> *mut c_void {
    if data_ptr.is_null() || data_len < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_make_bytes\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyBytes_FromStringAndSize(data_ptr as *const i8, data_len as ffi::Py_ssize_t)
        as *mut c_void
}

unsafe extern "C" fn load_name_hook(
    globals_obj: *mut c_void,
    name_ptr: *const u8,
    name_len: i64,
) -> *mut c_void {
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
    result as *mut c_void
}

unsafe extern "C" fn load_local_raw_by_name_hook(
    owner: *mut c_void,
    name_ptr: *const u8,
    name_len: i64,
) -> *mut c_void {
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
    result as *mut c_void
}

unsafe extern "C" fn pyobject_getattr_hook(obj: *mut c_void, attr: *mut c_void) -> *mut c_void {
    if obj.is_null() || attr.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_pyobject_getattr\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyObject_GetAttr(obj as *mut ffi::PyObject, attr as *mut ffi::PyObject) as *mut c_void
}

unsafe extern "C" fn pyobject_setattr_hook(
    obj: *mut c_void,
    attr: *mut c_void,
    value: *mut c_void,
) -> *mut c_void {
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
        none as *mut c_void
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn pyobject_getitem_hook(obj: *mut c_void, key: *mut c_void) -> *mut c_void {
    if obj.is_null() || key.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid arguments to dp_jit_pyobject_getitem\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyObject_GetItem(obj as *mut ffi::PyObject, key as *mut ffi::PyObject) as *mut c_void
}

unsafe extern "C" fn pyobject_setitem_hook(
    obj: *mut c_void,
    key: *mut c_void,
    value: *mut c_void,
) -> *mut c_void {
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
        none as *mut c_void
    } else {
        ptr::null_mut()
    }
}

unsafe extern "C" fn pyobject_to_i64_hook(value: *mut c_void) -> i64 {
    if value.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid null object for dp_jit_pyobject_to_i64\0".as_ptr() as *const i8,
        );
        return i64::MIN;
    }
    let result = ffi::PyLong_AsLongLong(value as *mut ffi::PyObject);
    if result == -1 && !ffi::PyErr_Occurred().is_null() {
        return i64::MIN;
    }
    result as i64
}

unsafe extern "C" fn decode_literal_bytes_hook(data_ptr: *const u8, data_len: i64) -> *mut c_void {
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
    ) as *mut c_void
}

unsafe extern "C" fn tuple_new_hook(size: i64) -> *mut c_void {
    if size < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid tuple size\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }
    ffi::PyTuple_New(size as ffi::Py_ssize_t) as *mut c_void
}

unsafe extern "C" fn tuple_set_item_hook(
    tuple_obj: *mut c_void,
    index: i64,
    item: *mut c_void,
) -> i32 {
    if tuple_obj.is_null() || item.is_null() || index < 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid tuple_set_item arguments\0".as_ptr() as *const i8,
        );
        return -1;
    }
    ffi::PyTuple_SetItem(
        tuple_obj as *mut ffi::PyObject,
        index as ffi::Py_ssize_t,
        item as *mut ffi::PyObject,
    )
}

unsafe extern "C" fn is_true_hook(value: *mut c_void) -> i32 {
    if value.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid null object for dp_jit_is_true\0".as_ptr() as *const i8,
        );
        return -1;
    }
    ffi::PyObject_IsTrue(value as *mut ffi::PyObject)
}

unsafe extern "C" fn raise_from_exc_hook(exc: *mut c_void) -> i32 {
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

fn default_specialized_hooks() -> jit::SpecializedJitHooks {
    jit::SpecializedJitHooks {
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

unsafe fn make_clif_function_data(
    module_name: &str,
    qualname: &str,
    build_entry_args_obj: *mut ffi::PyObject,
    materialize_entry_obj: *mut ffi::PyObject,
) -> Result<*mut c_void, ()> {
    if build_entry_args_obj.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"CLIF function registration expects a non-null build-entry-args helper\0".as_ptr()
                as *const c_char,
        );
        return Err(());
    }
    let plan = jit::lookup_clif_plan(module_name, qualname);
    let Some(plan) = plan else {
        let msg = format!(
            "no specialized JIT plan found: module={module_name:?} qualname={qualname:?}"
        );
        if let Ok(c_msg) = CString::new(msg) {
            ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
        } else {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"no specialized JIT plan found\0".as_ptr() as *const c_char,
            );
        }
        return Err(());
    };
    if let Some((index, _)) = plan
        .block_fast_paths
        .iter()
        .enumerate()
        .find(|(_, path)| matches!(path, jit::BlockFastPath::None))
    {
        let label = plan
            .block_labels
            .get(index)
            .map(String::as_str)
            .unwrap_or("<unknown>");
        let msg = format!(
            "CLIF function requires full fast-path plan; unsupported block at index {index} label {label:?} for module={module_name:?} qualname={qualname:?}"
        );
        if let Ok(c_msg) = CString::new(msg) {
            ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
        } else {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"CLIF function requires full fast-path plan\0".as_ptr() as *const c_char,
            );
        }
        return Err(());
    }

    let true_obj = ffi::PyBool_FromLong(1);
    if true_obj.is_null() {
        return Err(());
    }
    let false_obj = ffi::PyBool_FromLong(0);
    if false_obj.is_null() {
        ffi::Py_DECREF(true_obj);
        return Err(());
    }

    let clif_data = Box::new(ClifFunctionData {
        plan,
        module_name: module_name.to_string(),
        qualname: qualname.to_string(),
        true_obj,
        false_obj,
        build_entry_args_obj: {
            let ptr = build_entry_args_obj;
            ffi::Py_INCREF(ptr);
            ptr
        },
        materialize_entry_obj: {
            let ptr = materialize_entry_obj;
            if !ptr.is_null() {
                ffi::Py_INCREF(ptr);
            }
            ptr
        },
        compiled_handle: ptr::null_mut(),
        compiled_vectorcall_handle: ptr::null_mut(),
        compiled_vectorcall_entry: None,
    });
    Ok(Box::into_raw(clif_data) as *mut c_void)
}

unsafe fn clif_vectorcall_data(function: *mut ffi::PyObject) -> Result<&'static mut ClifFunctionData, ()> {
    if ffi::PyFunction_Check(function) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"expected Python function for CLIF vectorcall data lookup\0".as_ptr() as *const i8,
        );
        return Err(());
    }
    let dict = (*(function as *mut ffi::PyFunctionObject)).func_dict;
    if dict.is_null() {
        return set_runtime_error("missing CLIF vectorcall metadata dictionary");
    }
    let capsule =
        ffi::PyDict_GetItemString(dict, CLIF_VECTORCALL_ATTR.as_ptr() as *const c_char);
    if capsule.is_null() {
        return set_runtime_error("missing CLIF vectorcall metadata capsule");
    }
    let ptr = ffi::PyCapsule_GetPointer(
        capsule,
        CLIF_VECTORCALL_CAPSULE_NAME.as_ptr() as *const c_char,
    );
    if ptr.is_null() {
        return Err(());
    }
    Ok(&mut *(ptr as *mut ClifFunctionData))
}

unsafe fn ensure_clif_vectorcall_compiled(
    py: Python<'_>,
    callable: *mut ffi::PyObject,
    data: &mut ClifFunctionData,
) -> Result<(), ()> {
    if !data.materialize_entry_obj.is_null() {
        return Ok(());
    }
    if data.compiled_handle.is_null() {
        let globals_obj = ffi::PyFunction_GetGlobals(callable);
        if globals_obj.is_null() {
            return Err(());
        }
        let empty_tuple_obj = PyTuple::empty(py);
        let compile_start = Instant::now();
        let block_ptrs = vec![ptr::null_mut::<c_void>(); data.plan.block_labels.len()];
        data.compiled_handle = match jit::compile_cranelift_run_bb_specialized_cached(
            block_ptrs.as_slice(),
            &data.plan,
            globals_obj as *mut c_void,
            data.true_obj as *mut c_void,
            data.false_obj as *mut c_void,
            py.None().as_ptr() as *mut c_void,
            empty_tuple_obj.as_ptr() as *mut c_void,
        ) {
            Ok(handle) => handle,
            Err(err) => {
                if let Ok(c_msg) = CString::new(err) {
                    ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
                } else {
                    ffi::PyErr_SetString(
                        ffi::PyExc_RuntimeError,
                        b"failed to compile CLIF function body\0".as_ptr() as *const i8,
                    );
                }
                return Err(());
            }
        };
        let elapsed_ms = compile_start.elapsed().as_secs_f64() * 1000.0;
        info!(
            "soac_jit_precompile module={} qualname={} blocks={} elapsed_ms={elapsed_ms:.3}",
            data.module_name,
            data.qualname,
            data.plan.block_labels.len(),
        );
    }
    if data.compiled_vectorcall_handle.is_null() {
        let (handle, entry) = match jit::compile_cranelift_vectorcall_trampoline(
            build_bb_args_from_vectorcall,
            run_clif_vectorcall_compiled,
            data as *mut ClifFunctionData as *mut c_void,
            data.compiled_handle,
        ) {
            Ok(value) => value,
            Err(err) => {
                if let Ok(c_msg) = CString::new(err) {
                    ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
                } else {
                    ffi::PyErr_SetString(
                        ffi::PyExc_RuntimeError,
                        b"failed to compile CLIF vectorcall trampoline\0".as_ptr()
                            as *const i8,
                    );
                }
                return Err(());
            }
        };
        data.compiled_vectorcall_handle = handle;
        data.compiled_vectorcall_entry = Some(entry);
        let vectorcall_entry: ffi::vectorcallfunc = std::mem::transmute(entry);
        PyFunction_SetVectorcall(callable as *mut ffi::PyFunctionObject, vectorcall_entry);
    }
    Ok(())
}

unsafe fn vectorcall_args_tuple(
    args: *const *mut ffi::PyObject,
    count: ffi::Py_ssize_t,
) -> *mut ffi::PyObject {
    let tuple = ffi::PyTuple_New(count);
    if tuple.is_null() {
        return ptr::null_mut();
    }
    for index in 0..count {
        let item = if count == 0 {
            ptr::null_mut()
        } else {
            *args.add(index as usize)
        };
        if item.is_null() {
            ffi::Py_DECREF(tuple);
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"null vectorcall positional argument\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        ffi::Py_INCREF(item);
        if ffi::PyTuple_SetItem(tuple, index, item) != 0 {
            ffi::Py_DECREF(tuple);
            return ptr::null_mut();
        }
    }
    tuple
}

unsafe fn vectorcall_kwargs_dict(
    args: *const *mut ffi::PyObject,
    positional_count: ffi::Py_ssize_t,
    keyword_count: ffi::Py_ssize_t,
    kwnames: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    if keyword_count == 0 {
        return ptr::null_mut();
    }
    let kwargs = ffi::PyDict_New();
    if kwargs.is_null() {
        return ptr::null_mut();
    }
    for index in 0..keyword_count {
        let key = ffi::PyTuple_GetItem(kwnames, index);
        if key.is_null() {
            ffi::Py_DECREF(kwargs);
            return ptr::null_mut();
        }
        let value = *args.add((positional_count + index) as usize);
        if value.is_null() {
            ffi::Py_DECREF(kwargs);
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"null vectorcall keyword argument\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        if ffi::PyDict_SetItem(kwargs, key, value) != 0 {
            ffi::Py_DECREF(kwargs);
            return ptr::null_mut();
        }
    }
    kwargs
}

unsafe extern "C" fn build_bb_args_from_vectorcall(
    callable: *mut c_void,
    args: *const *mut c_void,
    nargsf: usize,
    kwnames: *mut c_void,
    data_ptr: *mut c_void,
) -> *mut c_void {
    match panic::catch_unwind(AssertUnwindSafe(|| {
        if callable.is_null() || data_ptr.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid vectorcall build args input\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        let py = Python::assume_attached();
        let data = &mut *(data_ptr as *mut ClifFunctionData);
        let nargs = ffi::PyVectorcall_NARGS(nargsf) as ffi::Py_ssize_t;
        let kwcount = if kwnames.is_null() {
            0
        } else {
            ffi::PyTuple_GET_SIZE(kwnames as *mut ffi::PyObject)
        };
        let poscount = nargs;
        let args_tuple = vectorcall_args_tuple(args as *const *mut ffi::PyObject, poscount);
        if args_tuple.is_null() {
            return ptr::null_mut();
        }
        let kwargs_dict = vectorcall_kwargs_dict(
            args as *const *mut ffi::PyObject,
            poscount,
            kwcount,
            kwnames as *mut ffi::PyObject,
        );
        if kwcount > 0 && kwargs_dict.is_null() {
            ffi::Py_DECREF(args_tuple);
            return ptr::null_mut();
        }

        let kwargs_arg = if kwargs_dict.is_null() {
            py.None().as_ptr()
        } else {
            kwargs_dict
        };
        let bb_args = match owned_ptr_to_bound(
            py,
            ffi::PyObject_CallFunctionObjArgs(
                data.build_entry_args_obj,
                args_tuple,
                kwargs_arg,
                ptr::null_mut::<ffi::PyObject>(),
            ),
        ) {
            Ok(value) => value,
            Err(err) => {
                err.restore(py);
                ffi::Py_DECREF(args_tuple);
                if !kwargs_dict.is_null() {
                    ffi::Py_DECREF(kwargs_dict);
                }
                return ptr::null_mut();
            }
        };
        ffi::Py_DECREF(args_tuple);
        if !kwargs_dict.is_null() {
            ffi::Py_DECREF(kwargs_dict);
        }
        bb_args.into_ptr() as *mut c_void
    })) {
        Ok(value) => value,
        Err(payload) => {
            let message = format!(
                "panic in build_bb_args_from_vectorcall: {}",
                panic_payload_to_string(payload)
            );
            if let Ok(c_msg) = CString::new(message) {
                ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
            } else {
                ffi::PyErr_SetString(
                    ffi::PyExc_RuntimeError,
                    b"panic in build_bb_args_from_vectorcall\0".as_ptr() as *const i8,
                );
            }
            ptr::null_mut()
        }
    }
}

unsafe extern "C" fn run_clif_vectorcall_compiled(
    compiled_handle: *mut c_void,
    bb_args: *mut c_void,
) -> *mut c_void {
    match panic::catch_unwind(AssertUnwindSafe(|| {
        if compiled_handle.is_null() || bb_args.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid CLIF vectorcall compiled input\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        let hooks = default_specialized_hooks();
        match jit::run_cranelift_run_bb_specialized_cached(compiled_handle, bb_args, &hooks) {
            Ok(value) => value,
            Err(err) => {
                if let Ok(c_msg) = CString::new(err) {
                    ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
                } else {
                    ffi::PyErr_SetString(
                        ffi::PyExc_RuntimeError,
                        b"failed to execute CLIF vectorcall entry\0".as_ptr() as *const i8,
                    );
                }
                ptr::null_mut()
            }
        }
    })) {
        Ok(value) => value,
        Err(payload) => {
            let message = format!(
                "panic in run_clif_vectorcall_compiled: {}",
                panic_payload_to_string(payload)
            );
            if let Ok(c_msg) = CString::new(message) {
                ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
            } else {
                ffi::PyErr_SetString(
                    ffi::PyExc_RuntimeError,
                    b"panic in run_clif_vectorcall_compiled\0".as_ptr() as *const i8,
                );
            }
            ptr::null_mut()
        }
    }
}

unsafe extern "C" fn lazy_clif_vectorcall(
    callable: *mut ffi::PyObject,
    args: *const *mut ffi::PyObject,
    nargsf: usize,
    kwnames: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    match panic::catch_unwind(AssertUnwindSafe(|| {
        let py = Python::assume_attached();
        let data = match clif_vectorcall_data(callable) {
            Ok(value) => value,
            Err(()) => return ptr::null_mut(),
        };
        if !data.materialize_entry_obj.is_null() {
            let bb_args = build_bb_args_from_vectorcall(
                callable as *mut c_void,
                args as *const *mut c_void,
                nargsf,
                kwnames as *mut c_void,
                data as *mut ClifFunctionData as *mut c_void,
            );
            if bb_args.is_null() {
                return ptr::null_mut();
            }
            let result = ffi::PyObject_CallFunctionObjArgs(
                data.materialize_entry_obj,
                bb_args as *mut ffi::PyObject,
                ptr::null_mut::<ffi::PyObject>(),
            );
            ffi::Py_DECREF(bb_args as *mut ffi::PyObject);
            return result;
        }
        if ensure_clif_vectorcall_compiled(py, callable, data).is_err() {
            return ptr::null_mut();
        }
        let Some(entry) = data.compiled_vectorcall_entry else {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"missing compiled CLIF vectorcall entry\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        };
        entry(
            callable as *mut c_void,
            args as *const *mut c_void,
            nargsf,
            kwnames as *mut c_void,
        ) as *mut ffi::PyObject
    })) {
        Ok(value) => value,
        Err(payload) => {
            let message = format!(
                "panic in lazy_clif_vectorcall: {}",
                panic_payload_to_string(payload)
            );
            if let Ok(c_msg) = CString::new(message) {
                ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
            } else {
                ffi::PyErr_SetString(
                    ffi::PyExc_RuntimeError,
                    b"panic in lazy_clif_vectorcall\0".as_ptr() as *const i8,
                );
            }
            ptr::null_mut()
        }
    }
}

pub unsafe fn register_clif_vectorcall(
    function: *mut ffi::PyObject,
    module_name: &str,
    qualname: &str,
    build_entry_args_obj: *mut ffi::PyObject,
    materialize_entry_obj: *mut ffi::PyObject,
) -> Result<(), ()> {
    if ffi::PyFunction_Check(function) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"register_clif_vectorcall expects a Python function\0".as_ptr() as *const c_char,
        );
        return Err(());
    }
    let func = function as *mut ffi::PyFunctionObject;
    if !(*func).func_dict.is_null()
        && !ffi::PyDict_GetItemString(
            (*func).func_dict,
            CLIF_VECTORCALL_ATTR.as_ptr() as *const c_char,
        )
        .is_null()
    {
        PyFunction_SetVectorcall(func, lazy_clif_vectorcall);
        return Ok(());
    }

    let data_ptr = make_clif_function_data(
        module_name,
        qualname,
        build_entry_args_obj,
        materialize_entry_obj,
    )?;
    let capsule = ffi::PyCapsule_New(
        data_ptr,
        CLIF_VECTORCALL_CAPSULE_NAME.as_ptr() as *const c_char,
        Some(free_clif_vectorcall_capsule),
    );
    if capsule.is_null() {
        free_clif_function_data(data_ptr);
        return Err(());
    }
    if (*func).func_dict.is_null() {
        (*func).func_dict = ffi::PyDict_New();
        if (*func).func_dict.is_null() {
            ffi::Py_DECREF(capsule);
            return Err(());
        }
    }
    if ffi::PyDict_SetItemString(
        (*func).func_dict,
        CLIF_VECTORCALL_ATTR.as_ptr() as *const c_char,
        capsule,
    ) != 0
    {
        ffi::Py_DECREF(capsule);
        return Err(());
    }
    ffi::Py_DECREF(capsule);
    PyFunction_SetVectorcall(func, lazy_clif_vectorcall);
    Ok(())
}

pub unsafe fn compile_clif_vectorcall(function: *mut ffi::PyObject) -> Result<(), ()> {
    if ffi::PyFunction_Check(function) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"compile_clif_vectorcall expects a Python function\0".as_ptr() as *const c_char,
        );
        return Err(());
    }
    let py = Python::assume_attached();
    let data = clif_vectorcall_data(function)?;
    if !data.materialize_entry_obj.is_null() {
        return Ok(());
    }
    ensure_clif_vectorcall_compiled(py, function, data)
}

use super::*;
use crate::code_extra::{
    SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER, code_extra_index, get_code_extra, set_code_extra,
};
use crate::jit::{self, ClifPlan};
use log::info;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use std::any::Any;
use std::time::Instant;

unsafe extern "C" {
    fn PyUnstable_InterpreterFrame_GetCode(
        frame: *mut ffi::_PyInterpreterFrame,
    ) -> *mut ffi::PyObject;
    fn _PyInterpreterState_SetEvalFrameFunc(
        interp: *mut ffi::PyInterpreterState,
        eval_frame: extern "C" fn(
            tstate: *mut ffi::PyThreadState,
            frame: *mut ffi::_PyInterpreterFrame,
            throwflag: c_int,
        ) -> *mut ffi::PyObject,
    );
    fn _PyEval_EvalFrameDefault(
        tstate: *mut ffi::PyThreadState,
        frame: *mut ffi::_PyInterpreterFrame,
        throwflag: c_int,
    ) -> *mut ffi::PyObject;
    fn _PyEval_FrameClearAndPop(
        tstate: *mut ffi::PyThreadState,
        frame: *mut ffi::_PyInterpreterFrame,
    );
    fn _PyFrame_MakeAndSetFrameObject(
        frame: *mut ffi::_PyInterpreterFrame,
    ) -> *mut ffi::PyFrameObject;
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

static INIT_EVAL_FRAME_HOOK: Once = Once::new();

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

struct ClifWrapperData {
    plan: ClifPlan,
    module_name: String,
    qualname: String,
    true_obj: *mut ffi::PyObject,
    false_obj: *mut ffi::PyObject,
    sig_obj: *mut ffi::PyObject,
    state_order_obj: *mut ffi::PyObject,
    closure_obj: *mut ffi::PyObject,
    build_entry_args_obj: *mut ffi::PyObject,
    compiled_handle: *mut c_void,
}

unsafe fn is_module_init_entry(plan: &ClifPlan) -> bool {
    plan.block_labels
        .get(plan.entry_index)
        .is_some_and(|label| label.contains("_dp_module_init"))
}

unsafe fn ensure_clif_wrapper_compiled(
    py: Python<'_>,
    clif_data: &mut ClifWrapperData,
    globals_obj: *mut ffi::PyObject,
) -> PyResult<()> {
    if globals_obj.is_null() {
        return Err(PyRuntimeError::new_err(
            "invalid null globals while compiling CLIF wrapper",
        ));
    }
    if is_module_init_entry(&clif_data.plan) || !clif_data.compiled_handle.is_null() {
        return Ok(());
    }
    let compile_start = Instant::now();
    let empty_tuple_obj = PyTuple::empty(py);
    let block_ptrs = vec![ptr::null_mut::<c_void>(); clif_data.plan.block_labels.len()];
    clif_data.compiled_handle = unsafe {
        jit::compile_cranelift_run_bb_specialized_cached(
            block_ptrs.as_slice(),
            &clif_data.plan,
            globals_obj as *mut c_void,
            clif_data.true_obj as *mut c_void,
            clif_data.false_obj as *mut c_void,
            py.None().as_ptr() as *mut c_void,
            empty_tuple_obj.as_ptr() as *mut c_void,
        )
    }
    .map_err(PyRuntimeError::new_err)?;
    let elapsed_ms = compile_start.elapsed().as_secs_f64() * 1000.0;
    info!(
        "soac_jit_precompile module={} qualname={} blocks={} elapsed_ms={elapsed_ms:.3}",
        clif_data.module_name,
        clif_data.qualname,
        clif_data.plan.block_labels.len(),
    );
    Ok(())
}

unsafe extern "C" fn free_clif_wrapper_data(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let data = unsafe { Box::from_raw(ptr as *mut ClifWrapperData) };
    if !data.true_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.true_obj) };
    }
    if !data.false_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.false_obj) };
    }
    if !data.sig_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.sig_obj) };
    }
    if !data.state_order_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.state_order_obj) };
    }
    if !data.closure_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.closure_obj) };
    }
    if !data.build_entry_args_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.build_entry_args_obj) };
    }
    unsafe { jit::free_cranelift_run_bb_specialized_cached(data.compiled_handle) };
}

unsafe fn frame_var_get_required(
    frame_obj: *mut ffi::PyFrameObject,
    name: &str,
) -> Result<*mut ffi::PyObject, ()> {
    let value = frame_var_get_optional(frame_obj, name)?;
    if value.is_null() {
        return set_runtime_error(&format!("missing frame variable {name}"));
    }
    Ok(value)
}

unsafe fn owned_ptr_to_bound<'py>(
    py: Python<'py>,
    ptr: *mut ffi::PyObject,
) -> PyResult<Bound<'py, PyAny>> {
    Bound::from_owned_ptr_or_opt(py, ptr).ok_or_else(|| PyErr::fetch(py))
}

unsafe fn frame_var_get_required_bound<'py>(
    py: Python<'py>,
    frame_obj: *mut ffi::PyFrameObject,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let value = frame_var_get_required(frame_obj, name).map_err(|_| PyErr::fetch(py))?;
    Ok(Bound::from_owned_ptr(py, value))
}

unsafe fn frame_var_get_optional_bound<'py>(
    py: Python<'py>,
    frame_obj: *mut ffi::PyFrameObject,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let value = frame_var_get_optional(frame_obj, name).map_err(|_| PyErr::fetch(py))?;
    Ok(Bound::from_owned_ptr_or_opt(py, value))
}

unsafe fn eval_clif_wrapper_frame(frame_obj: *mut ffi::PyFrameObject) -> *mut ffi::PyObject {
    unsafe extern "C" fn jit_incref(obj: *mut c_void) {
        if !obj.is_null() {
            ffi::Py_INCREF(obj as *mut ffi::PyObject);
        }
    }

    unsafe extern "C" fn jit_decref(obj: *mut c_void) {
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

    unsafe extern "C" fn py_call_object_hook(
        callable: *mut c_void,
        args: *mut c_void,
    ) -> *mut c_void {
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

    unsafe extern "C" fn decode_literal_bytes_hook(
        data_ptr: *const u8,
        data_len: i64,
    ) -> *mut c_void {
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
                b"invalid tuple size in JIT\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        ffi::PyTuple_New(size as ffi::Py_ssize_t) as *mut c_void
    }

    unsafe extern "C" fn tuple_set_item_hook(
        tuple_obj: *mut c_void,
        index: i64,
        value: *mut c_void,
    ) -> i32 {
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

    unsafe extern "C" fn is_true_hook(value: *mut c_void) -> i32 {
        if value.is_null() {
            return -1;
        }
        ffi::PyObject_IsTrue(value as *mut ffi::PyObject)
    }

    unsafe extern "C" fn compare_eq_obj_hook(lhs: *mut c_void, rhs: *mut c_void) -> *mut c_void {
        if lhs.is_null() || rhs.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid arguments to dp_jit_compare_eq_obj\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        ffi::PyObject_RichCompare(
            lhs as *mut ffi::PyObject,
            rhs as *mut ffi::PyObject,
            ffi::Py_EQ,
        ) as *mut c_void
    }

    unsafe extern "C" fn compare_lt_obj_hook(lhs: *mut c_void, rhs: *mut c_void) -> *mut c_void {
        if lhs.is_null() || rhs.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid arguments to dp_jit_compare_lt_obj\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        ffi::PyObject_RichCompare(
            lhs as *mut ffi::PyObject,
            rhs as *mut ffi::PyObject,
            ffi::Py_LT,
        ) as *mut c_void
    }

    unsafe extern "C" fn raise_from_exc_hook(exc: *mut c_void) -> i32 {
        if exc.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid null exception instance in JIT raise\0".as_ptr() as *const i8,
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

    let py = Python::assume_attached();
    let result: PyResult<Py<PyAny>> = (|| -> PyResult<Py<PyAny>> {
        let code_obj = unsafe { ffi::PyFrame_GetCode(frame_obj) };
        if code_obj.is_null() {
            return Err(PyErr::fetch(py));
        }
        let code_extra = unsafe { get_code_extra(code_obj as *mut ffi::PyObject) }
            .ok_or_else(|| PyRuntimeError::new_err("missing CLIF code extra in wrapper frame"))?;
        unsafe { ffi::Py_DECREF(code_obj as *mut ffi::PyObject) };
        if code_extra.kind != SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER || code_extra.data.is_null() {
            return Err(PyRuntimeError::new_err(
                "invalid CLIF wrapper code extra payload",
            ));
        }
        let clif_data = unsafe { &mut *(code_extra.data as *mut ClifWrapperData) };
        if clif_data.sig_obj.is_null()
            || clif_data.state_order_obj.is_null()
            || clif_data.closure_obj.is_null()
            || clif_data.build_entry_args_obj.is_null()
        {
            return Err(PyRuntimeError::new_err(
                "invalid CLIF wrapper data: missing wrapper metadata",
            ));
        }

        let args_obj = unsafe { frame_var_get_required_bound(py, frame_obj, "args") }?;
        let kwargs_obj = unsafe { frame_var_get_optional_bound(py, frame_obj, "kwargs") }?;
        let sig_obj = unsafe { Bound::<PyAny>::from_borrowed_ptr(py, clif_data.sig_obj) };
        let state_order_obj =
            unsafe { Bound::<PyAny>::from_borrowed_ptr(py, clif_data.state_order_obj) };
        let closure_obj = unsafe { Bound::<PyAny>::from_borrowed_ptr(py, clif_data.closure_obj) };
        let build_entry_args_obj =
            unsafe { Bound::<PyAny>::from_borrowed_ptr(py, clif_data.build_entry_args_obj) };

        let bind_method = sig_obj.getattr("bind")?;
        let bound = unsafe {
            owned_ptr_to_bound(
                py,
                ffi::PyObject_Call(
                    bind_method.as_ptr(),
                    args_obj.as_ptr(),
                    kwargs_obj
                        .as_ref()
                        .map_or(ptr::null_mut(), |kwargs| kwargs.as_ptr()),
                ),
            )
        }?;

        let _ = bound.call_method0("apply_defaults")?;
        let bound_arguments = bound.getattr("arguments")?;

        let bb_args = unsafe {
            owned_ptr_to_bound(
                py,
                ffi::PyObject_CallFunctionObjArgs(
                    build_entry_args_obj.as_ptr(),
                    bound_arguments.as_ptr(),
                    state_order_obj.as_ptr(),
                    closure_obj.as_ptr(),
                    ptr::null_mut::<ffi::PyObject>(),
                ),
            )
        }?;

        let globals_obj = unsafe { ffi::PyFrame_GetGlobals(frame_obj) };
        if globals_obj.is_null() {
            return Err(PyErr::fetch(py));
        }
        let is_module_init_entry = unsafe { is_module_init_entry(&clif_data.plan) };
        if let Err(err) = unsafe { ensure_clif_wrapper_compiled(py, clif_data, globals_obj) } {
            unsafe { ffi::Py_DECREF(globals_obj) };
            return Err(err);
        }
        let empty_tuple_obj = PyTuple::empty(py);
        let hooks = jit::SpecializedJitHooks {
            incref: jit_incref,
            decref: jit_decref,
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
            compare_eq_obj: compare_eq_obj_hook,
            compare_lt_obj: compare_lt_obj_hook,
            raise_from_exc: raise_from_exc_hook,
        };
        let run_result = unsafe {
            if is_module_init_entry {
                let block_ptrs = vec![ptr::null_mut::<c_void>(); clif_data.plan.block_labels.len()];
                jit::run_cranelift_run_bb_specialized(
                    block_ptrs.as_slice(),
                    &clif_data.plan,
                    globals_obj as *mut c_void,
                    clif_data.true_obj as *mut c_void,
                    clif_data.false_obj as *mut c_void,
                    bb_args.as_ptr() as *mut c_void,
                    &hooks,
                    py.None().as_ptr() as *mut c_void,
                    empty_tuple_obj.as_ptr() as *mut c_void,
                )
            } else {
                jit::run_cranelift_run_bb_specialized_cached(
                    clif_data.compiled_handle,
                    bb_args.as_ptr() as *mut c_void,
                    &hooks,
                )
            }
        };
        let result_ptr = match run_result {
            Ok(ptr) => ptr,
            Err(err) => {
                unsafe { ffi::Py_DECREF(globals_obj) };
                return Err(PyRuntimeError::new_err(err));
            }
        };
        unsafe { ffi::Py_DECREF(globals_obj) };
        if result_ptr.is_null() {
            if unsafe { ffi::PyErr_Occurred() }.is_null() {
                return Err(PyRuntimeError::new_err(
                    "Cranelift JIT run_bb returned null result without exception",
                ));
            }
            return Err(PyErr::fetch(py));
        }
        let result = unsafe {
            Bound::<PyAny>::from_owned_ptr_or_opt(py, result_ptr as *mut ffi::PyObject)
                .ok_or_else(|| PyErr::fetch(py))?
        };
        Ok(result.unbind())
    })();

    match result {
        Ok(value) => value.into_ptr(),
        Err(err) => {
            err.restore(py);
            ptr::null_mut()
        }
    }
}

unsafe fn frame_var_get_optional(
    frame_obj: *mut ffi::PyFrameObject,
    name: &str,
) -> Result<*mut ffi::PyObject, ()> {
    unsafe fn mapping_lookup_optional(
        mapping: *mut ffi::PyObject,
        name: &str,
    ) -> Result<*mut ffi::PyObject, ()> {
        if mapping.is_null() {
            return Ok(ptr::null_mut());
        }
        let key = ffi::PyUnicode_FromStringAndSize(name.as_ptr() as *const c_char, name.len() as _);
        if key.is_null() {
            return Err(());
        }
        let value = ffi::PyObject_GetItem(mapping, key);
        ffi::Py_DECREF(key);
        if value.is_null() {
            if ffi::PyErr_ExceptionMatches(ffi::PyExc_KeyError) != 0 {
                ffi::PyErr_Clear();
                return Ok(ptr::null_mut());
            }
            return Err(());
        }
        Ok(value)
    }

    let c_name = match CString::new(name) {
        Ok(name) => name,
        Err(_) => return Err(()),
    };
    let value = ffi::PyFrame_GetVarString(frame_obj, c_name.as_ptr() as *mut c_char);
    if value.is_null() {
        if ffi::PyErr_ExceptionMatches(ffi::PyExc_NameError) != 0
            || ffi::PyErr_ExceptionMatches(ffi::PyExc_UnboundLocalError) != 0
        {
            ffi::PyErr_Clear();
            // `PyFrame_GetVarString` can legitimately miss names that *do* exist in the
            // executing frame at this point, especially for class-body namespace functions
            // where `_dp_classcell` is both a parameter and an implicit cellvar.
            //
            // In that shape, CPython stores the authoritative runtime binding in the frame
            // locals mapping while the fast lookup path can still report NameError/Unbound.
            // If we treat that as truly missing, nested functions are built with an empty
            // closure slot and later fail with `NameError: _dp_classcell is not defined`.
            //
            // So for name-like misses we fall back to `PyFrame_GetLocals` + mapping lookup,
            // preserving CPython-visible bindings when transitioning from frame execution into
            // SOAC eval. Other exception types remain hard errors.
            let locals = ffi::PyFrame_GetLocals(frame_obj);
            if locals.is_null() {
                return Ok(ptr::null_mut());
            }
            let fallback = mapping_lookup_optional(locals, name);
            ffi::Py_DECREF(locals);
            return fallback;
        }
        return Err(());
    }
    Ok(value)
}

unsafe fn finalize_soac_frame(
    tstate: *mut ffi::PyThreadState,
    frame: *mut ffi::_PyInterpreterFrame,
) {
    _PyEval_FrameClearAndPop(tstate, frame);
}

struct SoacRecursionGuard;

impl Drop for SoacRecursionGuard {
    fn drop(&mut self) {
        unsafe { ffi::Py_LeaveRecursiveCall() };
    }
}

unsafe fn enter_soac_recursion_guard() -> Option<SoacRecursionGuard> {
    if unsafe {
        ffi::Py_EnterRecursiveCall(b" while calling a Python object\0".as_ptr() as *const i8)
    } != 0
    {
        return None;
    }
    Some(SoacRecursionGuard)
}

extern "C" fn soac_eval_frame(
    tstate: *mut ffi::PyThreadState,
    frame: *mut ffi::_PyInterpreterFrame,
    throwflag: c_int,
) -> *mut ffi::PyObject {
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let code = PyUnstable_InterpreterFrame_GetCode(frame);
        if code.is_null() {
            return _PyEval_EvalFrameDefault(tstate, frame, throwflag);
        }
        let extra = get_code_extra(code);
        ffi::Py_DECREF(code);
        let Some(extra) = extra else {
            return _PyEval_EvalFrameDefault(tstate, frame, throwflag);
        };
        if extra.kind == SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER {
            if throwflag != 0 {
                return _PyEval_EvalFrameDefault(tstate, frame, throwflag);
            }
            let Some(_recursion_guard) = enter_soac_recursion_guard() else {
                return ptr::null_mut();
            };
            let frame_obj = _PyFrame_MakeAndSetFrameObject(frame);
            if frame_obj.is_null() {
                return ptr::null_mut();
            }
            ffi::Py_INCREF(frame_obj as *mut ffi::PyObject);
            let result = eval_clif_wrapper_frame(frame_obj);
            ffi::Py_DECREF(frame_obj as *mut ffi::PyObject);
            finalize_soac_frame(tstate, frame);
            return result;
        }
        _PyEval_EvalFrameDefault(tstate, frame, throwflag)
    }));
    match run {
        Ok(result) => result,
        Err(payload) => unsafe {
            let message = panic_payload_to_string(payload);
            if let Ok(c_message) = CString::new(format!("soac_eval_frame panic: {message}")) {
                ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_message.as_ptr());
            } else {
                let fallback = CString::new("soac_eval_frame panic")
                    .expect("static panic fallback contains no null bytes");
                ffi::PyErr_SetString(ffi::PyExc_RuntimeError, fallback.as_ptr());
            }
            ptr::null_mut()
        },
    }
}

pub unsafe fn install_eval_frame_hook() -> Result<(), ()> {
    let mut ok = true;
    INIT_EVAL_FRAME_HOOK.call_once(|| {
        if code_extra_index().is_err() {
            ok = false;
            return;
        }
        let interp = ffi::PyInterpreterState_Get();
        if interp.is_null() {
            ok = false;
            return;
        }
        _PyInterpreterState_SetEvalFrameFunc(interp, soac_eval_frame);
    });
    if ok { Ok(()) } else { Err(()) }
}

pub unsafe fn register_clif_wrapper_code_extra(
    function: *mut ffi::PyObject,
    module_name: &str,
    qualname: &str,
    sig_obj: *mut ffi::PyObject,
    state_order_obj: *mut ffi::PyObject,
    closure_obj: *mut ffi::PyObject,
    build_entry_args_obj: *mut ffi::PyObject,
) -> Result<(), ()> {
    if install_eval_frame_hook().is_err() {
        return Err(());
    }
    if ffi::PyFunction_Check(function) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"register_clif_wrapper_code_extra expects a Python function\0".as_ptr()
                as *const c_char,
        );
        return Err(());
    }
    if sig_obj.is_null()
        || state_order_obj.is_null()
        || closure_obj.is_null()
        || build_entry_args_obj.is_null()
    {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"register_clif_wrapper_code_extra expects non-null metadata objects\0".as_ptr()
                as *const c_char,
        );
        return Err(());
    }
    let py = Python::assume_attached();
    let function_bound = Bound::<PyAny>::from_borrowed_ptr(py, function);
    let code_obj = match function_bound.getattr("__code__") {
        Ok(obj) => obj,
        Err(err) => {
            err.restore(py);
            return Err(());
        }
    };
    let cloned_code = match code_obj.call_method0("replace") {
        Ok(obj) => obj,
        Err(err) => {
            err.restore(py);
            return Err(());
        }
    };
    if let Err(err) = function_bound.setattr("__code__", cloned_code.clone()) {
        err.restore(py);
        return Err(());
    }
    let code = cloned_code.as_ptr();
    if matches!(get_code_extra(code), Some(extra) if extra.kind == SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER)
    {
        return Ok(());
    }
    let plan = jit::lookup_clif_plan(module_name, qualname);
    let Some(plan) = plan else {
        let msg = format!(
            "no specialized JIT plan found for CLIF wrapper: module={module_name:?} qualname={qualname:?}"
        );
        if let Ok(c_msg) = CString::new(msg) {
            ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
        } else {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"no specialized JIT plan found for CLIF wrapper\0".as_ptr() as *const c_char,
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
            "CLIF wrapper requires full fast-path plan; unsupported block at index {index} label {label:?} for module={module_name:?} qualname={qualname:?}"
        );
        if let Ok(c_msg) = CString::new(msg) {
            ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
        } else {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"CLIF wrapper requires full fast-path plan\0".as_ptr() as *const c_char,
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
    let clif_data = Box::new(ClifWrapperData {
        plan,
        module_name: module_name.to_string(),
        qualname: qualname.to_string(),
        true_obj,
        false_obj,
        sig_obj: {
            let ptr = sig_obj;
            ffi::Py_INCREF(ptr);
            ptr
        },
        state_order_obj: {
            let ptr = state_order_obj;
            ffi::Py_INCREF(ptr);
            ptr
        },
        closure_obj: {
            let ptr = closure_obj;
            ffi::Py_INCREF(ptr);
            ptr
        },
        build_entry_args_obj: {
            let ptr = build_entry_args_obj;
            ffi::Py_INCREF(ptr);
            ptr
        },
        compiled_handle: ptr::null_mut(),
    });
    let clif_data_ptr = Box::into_raw(clif_data) as *mut c_void;
    if set_code_extra(
        code,
        SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER,
        clif_data_ptr,
        Some(free_clif_wrapper_data),
    )
    .is_err()
    {
        free_clif_wrapper_data(clif_data_ptr);
        return Err(());
    }
    Ok(())
}

pub unsafe fn compile_clif_wrapper_code_extra(function: *mut ffi::PyObject) -> Result<(), ()> {
    if install_eval_frame_hook().is_err() {
        return Err(());
    }
    if ffi::PyFunction_Check(function) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"compile_clif_wrapper_code_extra expects a Python function\0".as_ptr()
                as *const c_char,
        );
        return Err(());
    }
    let py = Python::assume_attached();
    let function_bound = Bound::<PyAny>::from_borrowed_ptr(py, function);
    let code_obj = match function_bound.getattr("__code__") {
        Ok(obj) => obj,
        Err(err) => {
            err.restore(py);
            return Err(());
        }
    };
    let code_ptr = code_obj.as_ptr();
    let Some(code_extra) = get_code_extra(code_ptr) else {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"missing CLIF wrapper code extra\0".as_ptr() as *const c_char,
        );
        return Err(());
    };
    if code_extra.kind != SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER || code_extra.data.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"invalid CLIF wrapper code extra payload\0".as_ptr() as *const c_char,
        );
        return Err(());
    }
    let clif_data = &mut *(code_extra.data as *mut ClifWrapperData);
    let globals_obj = match function_bound.getattr("__globals__") {
        Ok(obj) => obj,
        Err(err) => {
            err.restore(py);
            return Err(());
        }
    };
    if let Err(err) = ensure_clif_wrapper_compiled(py, clif_data, globals_obj.as_ptr()) {
        err.restore(py);
        return Err(());
    }
    Ok(())
}

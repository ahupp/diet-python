use pyo3::exceptions::PyRuntimeError;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyTuple};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

struct ResolvedSpecializedJitBlocks {
    plan: soac_eval::jit::ClifPlan,
    block_ptrs: Vec<*mut c_void>,
    true_obj: *mut c_void,
    false_obj: *mut c_void,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct CachedSpecializedRunnerKey {
    module_name: String,
    qualname: String,
    globals_ptr: usize,
}

#[derive(Debug, Clone, Copy)]
struct CachedSpecializedRunner {
    compiled_handle: usize,
}

static SPECIALIZED_RUNNER_CACHE: OnceLock<
    Mutex<HashMap<CachedSpecializedRunnerKey, CachedSpecializedRunner>>,
> = OnceLock::new();

fn specialized_runner_cache(
) -> &'static Mutex<HashMap<CachedSpecializedRunnerKey, CachedSpecializedRunner>> {
    SPECIALIZED_RUNNER_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_or_compile_specialized_runner(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
    globals_obj: &Bound<'_, PyAny>,
    resolved: &ResolvedSpecializedJitBlocks,
) -> PyResult<*mut c_void> {
    let key = CachedSpecializedRunnerKey {
        module_name: module_name.to_string(),
        qualname: qualname.to_string(),
        globals_ptr: globals_obj.as_ptr() as usize,
    };
    if let Some(existing) = specialized_runner_cache()
        .lock()
        .map_err(|_| PyRuntimeError::new_err("failed to lock specialized JIT runner cache"))?
        .get(&key)
        .copied()
    {
        return Ok(existing.compiled_handle as *mut c_void);
    }

    let none_obj = py.None();
    let empty_tuple_obj = PyTuple::empty(py);
    let compiled_handle = unsafe {
        soac_eval::jit::compile_cranelift_run_bb_specialized_cached(
            resolved.block_ptrs.as_slice(),
            &resolved.plan,
            globals_obj.as_ptr() as *mut c_void,
            resolved.true_obj,
            resolved.false_obj,
            none_obj.as_ptr() as *mut c_void,
            empty_tuple_obj.as_ptr() as *mut c_void,
        )
    }
    .map_err(PyRuntimeError::new_err)?;

    // Compiled CLIF embeds the module globals pointer as a constant. Hold one
    // strong reference for the lifetime of the process-level cache so repeated
    // nested helper functions can reuse the same machine code safely.
    unsafe {
        ffi::Py_INCREF(globals_obj.as_ptr());
    }

    let mut cache = specialized_runner_cache()
        .lock()
        .map_err(|_| PyRuntimeError::new_err("failed to lock specialized JIT runner cache"))?;
    if let Some(existing) = cache.get(&key).copied() {
        unsafe {
            soac_eval::jit::free_cranelift_run_bb_specialized_cached(compiled_handle);
            ffi::Py_DECREF(globals_obj.as_ptr());
        }
        return Ok(existing.compiled_handle as *mut c_void);
    }
    cache.insert(
        key,
        CachedSpecializedRunner {
            compiled_handle: compiled_handle as usize,
        },
    );
    Ok(compiled_handle)
}

fn run_specialized_jit(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
    globals_obj: &Bound<'_, PyAny>,
    args: &Bound<'_, PyAny>,
    resolved: ResolvedSpecializedJitBlocks,
) -> PyResult<Py<PyAny>> {
    struct RecursionGuard;
    impl Drop for RecursionGuard {
        fn drop(&mut self) {
            unsafe { ffi::Py_LeaveRecursiveCall() };
        }
    }

    if unsafe {
        ffi::Py_EnterRecursiveCall(b" while calling a Python object\0".as_ptr() as *const i8)
    } != 0
    {
        return Err(PyErr::fetch(py));
    }
    let _recursion_guard = RecursionGuard;

    unsafe extern "C" fn preflight_incref(obj: *mut c_void) {
        if !obj.is_null() {
            ffi::Py_INCREF(obj as *mut ffi::PyObject);
        }
    }

    unsafe extern "C" fn preflight_decref(obj: *mut c_void) {
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
            std::ptr::null_mut::<ffi::PyObject>(),
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
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
        }
        let name_obj = ffi::PyUnicode_DecodeUTF8(
            name_ptr as *const i8,
            name_len as ffi::Py_ssize_t,
            b"strict\0".as_ptr() as *const i8,
        );
        if name_obj.is_null() {
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
        }
        ffi::Py_INCREF(load_global);
        let result = ffi::PyObject_CallFunctionObjArgs(
            load_global,
            globals_obj as *mut ffi::PyObject,
            name_obj,
            std::ptr::null_mut::<ffi::PyObject>(),
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
            return std::ptr::null_mut();
        }
        let name_obj = ffi::PyUnicode_DecodeUTF8(
            name_ptr as *const i8,
            name_len as ffi::Py_ssize_t,
            b"strict\0".as_ptr() as *const i8,
        );
        if name_obj.is_null() {
            return std::ptr::null_mut();
        }
        let builtins_dict = ffi::PyEval_GetBuiltins();
        if builtins_dict.is_null() {
            ffi::Py_DECREF(name_obj);
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"PyEval_GetBuiltins returned null\0".as_ptr() as *const i8,
            );
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
        }
        ffi::Py_INCREF(load_local_raw);
        let result = ffi::PyObject_CallFunctionObjArgs(
            load_local_raw,
            owner as *mut ffi::PyObject,
            name_obj,
            std::ptr::null_mut::<ffi::PyObject>(),
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
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
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
            std::ptr::null_mut()
        }
    }

    unsafe extern "C" fn pyobject_getitem_hook(obj: *mut c_void, key: *mut c_void) -> *mut c_void {
        if obj.is_null() || key.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid arguments to dp_jit_pyobject_getitem\0".as_ptr() as *const i8,
            );
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
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
            std::ptr::null_mut()
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
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
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

    let hooks = soac_eval::jit::SpecializedJitHooks {
        incref: preflight_incref,
        decref: preflight_decref,
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
    };

    let compiled_handle =
        get_or_compile_specialized_runner(py, module_name, qualname, globals_obj, &resolved)?;
    let result_ptr = unsafe {
        soac_eval::jit::run_cranelift_run_bb_specialized_cached(
            compiled_handle,
            args.as_ptr() as *mut c_void,
            &hooks,
        )
        .map_err(PyRuntimeError::new_err)?
    };

    if result_ptr.is_null() {
        if unsafe { ffi::PyErr_Occurred() }.is_null() {
            return Err(PyRuntimeError::new_err(
                "Cranelift JIT run_bb returned null result without exception",
            ));
        }
        return Err(PyErr::fetch(py));
    }
    let result = unsafe { Bound::<PyAny>::from_owned_ptr(py, result_ptr as *mut ffi::PyObject) };
    Ok(result.unbind())
}

pub(crate) fn jit_run_bb_plan_impl(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
    globals_obj: &Bound<'_, PyAny>,
    args: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let resolved = resolve_specialized_jit_blocks_by_key(py, module_name, qualname)?;
    run_specialized_jit(py, module_name, qualname, globals_obj, args, resolved)
}

pub(crate) fn jit_has_bb_plan_impl(module_name: &str, qualname: &str) -> bool {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, qualname) else {
        return false;
    };
    let has_none = plan
        .block_fast_paths
        .iter()
        .any(|path| matches!(path, soac_eval::jit::BlockFastPath::None));
    if has_none && std::env::var("DIET_PYTHON_DEBUG_JIT_HAS").as_deref() == Ok("1") {
        eprintln!("jit_has_bb_plan=false for {module_name}.{qualname}");
        for (idx, (label, path)) in plan
            .block_labels
            .iter()
            .zip(plan.block_fast_paths.iter())
            .enumerate()
        {
            eprintln!(
                "  [{idx}] {label}: {path:?}, exc_target={:?}",
                plan.block_exc_targets.get(idx).copied().flatten()
            );
        }
    }
    !has_none
}

pub(crate) fn jit_block_param_names_impl(
    module_name: &str,
    qualname: &str,
    entry_label: &str,
) -> PyResult<Vec<String>> {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, qualname) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.{qualname}"
        )));
    };
    let Some(index) = plan
        .block_labels
        .iter()
        .position(|label| label == entry_label)
    else {
        return Err(PyRuntimeError::new_err(format!(
            "entry label {:?} not found in plan {module_name}.{qualname}",
            entry_label
        )));
    };
    Ok(plan.block_param_names[index].clone())
}

pub(crate) fn jit_debug_plan_impl(module_name: &str, qualname: &str) -> PyResult<String> {
    let Some(plan) = soac_eval::jit::lookup_clif_plan(module_name, qualname) else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.{qualname}"
        )));
    };
    Ok(format!("{plan:#?}"))
}

fn resolve_specialized_jit_blocks_by_key(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
) -> PyResult<ResolvedSpecializedJitBlocks> {
    let plan = soac_eval::jit::lookup_clif_plan(module_name, qualname);
    let Some(plan) = plan else {
        return Err(PyRuntimeError::new_err(format!(
            "no specialized JIT plan for {module_name}.{qualname}"
        )));
    };
    if plan
        .block_fast_paths
        .iter()
        .any(|path| matches!(path, soac_eval::jit::BlockFastPath::None))
    {
        return Err(PyRuntimeError::new_err(format!(
            "specialized JIT requires fully lowered fastpath blocks: {module_name}.{qualname}"
        )));
    }
    let block_ptrs = vec![std::ptr::null_mut::<c_void>(); plan.block_labels.len()];
    if plan.entry_index >= block_ptrs.len() {
        return Err(PyRuntimeError::new_err(format!(
            "invalid JIT entry index {} for {} blocks",
            plan.entry_index,
            block_ptrs.len()
        )));
    }

    let true_obj = PyBool::new(py, true).as_ptr() as *mut c_void;
    let false_obj = PyBool::new(py, false).as_ptr() as *mut c_void;

    Ok(ResolvedSpecializedJitBlocks {
        plan,
        block_ptrs,
        true_obj,
        false_obj,
    })
}

pub(crate) fn jit_render_bb_plan_impl(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
) -> PyResult<String> {
    let resolved = resolve_specialized_jit_blocks_by_key(py, module_name, qualname)?;
    let empty_tuple_obj = PyTuple::empty(py);
    unsafe {
        soac_eval::jit::render_cranelift_run_bb_specialized(
            resolved.block_ptrs.as_slice(),
            &resolved.plan,
            resolved.true_obj,
            resolved.false_obj,
            empty_tuple_obj.as_ptr() as *mut c_void,
        )
        .map_err(PyRuntimeError::new_err)
    }
}

pub(crate) fn jit_render_bb_with_cfg_plan_impl(
    py: Python<'_>,
    module_name: &str,
    qualname: &str,
) -> PyResult<(String, String)> {
    let resolved = resolve_specialized_jit_blocks_by_key(py, module_name, qualname)?;
    let empty_tuple_obj = PyTuple::empty(py);
    unsafe {
        soac_eval::jit::render_cranelift_run_bb_specialized_with_cfg(
            resolved.block_ptrs.as_slice(),
            &resolved.plan,
            resolved.true_obj,
            resolved.false_obj,
            empty_tuple_obj.as_ptr() as *mut c_void,
        )
        .map(|rendered| (rendered.clif, rendered.cfg_dot))
        .map_err(PyRuntimeError::new_err)
    }
}

#[cfg(test)]
mod tests {
    use dp_transform::basic_block::bb_ir;
    use std::any::Any;

    fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
        if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_string()
        }
    }

    fn parse_and_lower(source: &str) -> Result<dp_transform::LoweringResult, String> {
        let options = dp_transform::Options {
            inject_import: true,
            eval_mode: true,
            lower_attributes: true,
            truthy: false,
            force_import_rewrite: true,
            ..dp_transform::Options::default()
        };

        match std::panic::catch_unwind(|| {
            dp_transform::transform_str_to_ruff_with_options(source, options)
        }) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(err.to_string()),
            Err(payload) => Err(panic_payload_to_string(payload)),
        }
    }

    fn validate_bb_module_for_jit(bb_module: Option<&bb_ir::BbModule>) -> Result<(), String> {
        let bb_module = bb_module.ok_or_else(|| {
            "JIT mode requires emitted basic-block IR, but none was produced".to_string()
        })?;
        for function in &bb_module.functions {
            match &function.kind {
                bb_ir::BbFunctionKind::Function
                | bb_ir::BbFunctionKind::Generator { .. }
                | bb_ir::BbFunctionKind::AsyncGenerator { .. } => {}
            }
        }
        Ok(())
    }

    fn run_cranelift_jit_preflight(bb_module: Option<&bb_ir::BbModule>) -> Result<(), String> {
        let bb_module = bb_module.ok_or_else(|| {
            "JIT mode requires emitted basic-block IR, but none was produced".to_string()
        })?;
        let normalized = dp_transform::basic_block::normalize_bb_module_for_codegen(bb_module);
        soac_eval::jit::run_cranelift_smoke(&normalized)
    }

    #[test]
    fn jit_validator_accepts_class_defs_without_def_fn_ops() {
        let source = r#"
class C:
    x = 1
    def m(self):
        return self.x
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref())
            .expect("validator should accept lowered class defs");
    }

    #[test]
    fn jit_validator_accepts_coroutines() {
        let source = r#"
async def run():
    return 1
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref())
            .expect("validator should accept coroutine lowering");
    }

    #[test]
    fn jit_validator_accepts_async_generators() {
        let source = r#"
async def run():
    yield 1
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref())
            .expect("validator should accept async generator lowering");
    }

    #[test]
    fn jit_validator_allows_try_jump_terminators() {
        let source = r#"
def f():
    return 1
"#;
        let mut bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module
            .expect("bb module should be present");
        let function = bb_module
            .functions
            .first_mut()
            .expect("must contain at least one function");
        let block = function
            .blocks
            .first_mut()
            .expect("function must contain at least one block");
        block.term = bb_ir::BbTerm::TryJump {
            body_label: "body".to_string(),
            except_label: "except".to_string(),
            except_exc_name: None,
            body_region_labels: vec![],
            except_region_labels: vec![],
            finally_label: None,
            finally_exc_name: None,
            finally_region_labels: vec![],
            finally_fallthrough_label: None,
        };
        validate_bb_module_for_jit(Some(&bb_module)).expect("validator should allow try_jump");
    }

    #[test]
    fn jit_preflight_runs_cranelift_for_supported_module() {
        let source = r#"
def f(x):
    return x
"#;
        let bb_module = parse_and_lower(source)
            .expect("lowering should succeed")
            .bb_module;
        validate_bb_module_for_jit(bb_module.as_ref()).expect("validator should allow module");
        run_cranelift_jit_preflight(bb_module.as_ref()).expect("cranelift preflight should run");
    }
}

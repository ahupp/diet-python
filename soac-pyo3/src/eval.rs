use dp_transform::{
    Options,
    basic_block::{bb_ir, normalize_bb_module_for_codegen},
    transform_str_to_ruff_with_options,
};
use pyo3::exceptions::{PyRuntimeError, PySyntaxError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList, PyTuple};
use std::any::Any;
use std::ffi::CString;
use std::ffi::c_void;

#[derive(Debug)]
pub(crate) enum TransformError {
    Parse(String),
    Lowering(String),
}

pub(crate) struct ExecLoweringResult {
    pub(crate) bb_module: Option<bb_ir::BbModule>,
    pub(crate) module_docstring: Option<String>,
}

struct ResolvedSpecializedJitBlocks {
    plan: soac_eval::jit::ClifPlan,
    block_ptrs: Vec<*mut c_void>,
    true_obj: *mut c_void,
    false_obj: *mut c_void,
}

impl TransformError {
    pub(crate) fn to_py_err(self) -> PyErr {
        match self {
            TransformError::Parse(msg) => PySyntaxError::new_err(msg),
            TransformError::Lowering(msg) => {
                PyRuntimeError::new_err(format!("AST lowering failed: {msg}"))
            }
        }
    }
}

fn panic_payload_to_string(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn parse_and_lower(source: &str) -> Result<dp_transform::LoweringResult, TransformError> {
    let options = Options {
        inject_import: true,
        eval_mode: true,
        lower_attributes: true,
        truthy: false,
        force_import_rewrite: true,
        ..Options::default()
    };

    match std::panic::catch_unwind(|| transform_str_to_ruff_with_options(source, options)) {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(TransformError::Parse(err.to_string())),
        Err(payload) => Err(TransformError::Lowering(panic_payload_to_string(payload))),
    }
}

fn lower_for_execution(source: &str) -> Result<ExecLoweringResult, TransformError> {
    let lowered = parse_and_lower(source)?;
    let module_docstring = lowered.module_docstring();
    Ok(ExecLoweringResult {
        bb_module: lowered.bb_module,
        module_docstring,
    })
}

fn build_module_spec(
    py: Python<'_>,
    name: &str,
    path: &str,
    is_package: bool,
) -> PyResult<Py<PyAny>> {
    let importlib_util = py.import("importlib.util")?;
    let spec_from = importlib_util.getattr("spec_from_file_location")?;
    let spec = if is_package {
        let kwargs = PyDict::new(py);
        let dir = std::path::Path::new(path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        kwargs.set_item("submodule_search_locations", vec![dir])?;
        spec_from.call((name, path), Some(&kwargs))?
    } else {
        spec_from.call1((name, path))?
    };
    Ok(spec.unbind())
}

fn set_spec_initializing(spec: &Bound<'_, PyAny>, value: bool) {
    if spec.setattr("_initializing", value).is_err() {
        unsafe {
            ffi::PyErr_Clear();
        }
    }
}

pub(crate) fn eval_source_impl(py: Python<'_>, path: &str, source: &str) -> PyResult<Py<PyAny>> {
    eval_source_impl_with_name(py, path, source, "eval_source", None)
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
            bb_ir::BbFunctionKind::Coroutine => {
                return Err(format!(
                    "JIT mode does not support coroutine functions yet: {}",
                    function.qualname
                ));
            }
        }
    }
    Ok(())
}

fn run_cranelift_jit_preflight(bb_module: Option<&bb_ir::BbModule>) -> Result<(), String> {
    let bb_module = bb_module.ok_or_else(|| {
        "JIT mode requires emitted basic-block IR, but none was produced".to_string()
    })?;
    let normalized = normalize_bb_module_for_codegen(bb_module);
    soac_eval::jit::run_cranelift_smoke(&normalized)
}

fn run_cranelift_python_call_preflight(py: Python<'_>) -> Result<(), String> {
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

    unsafe extern "C" fn preflight_call_one_arg(
        callable: *mut c_void,
        arg: *mut c_void,
    ) -> *mut c_void {
        ffi::PyObject_CallFunctionObjArgs(
            callable as *mut ffi::PyObject,
            arg as *mut ffi::PyObject,
            std::ptr::null_mut::<ffi::PyObject>(),
        ) as *mut c_void
    }

    unsafe extern "C" fn preflight_compare_eq(lhs: *mut c_void, rhs: *mut c_void) -> i32 {
        ffi::PyObject_RichCompareBool(
            lhs as *mut ffi::PyObject,
            rhs as *mut ffi::PyObject,
            ffi::Py_EQ,
        )
    }

    // Execute one real Python call through JITed machine code, including
    // imported INCREF/DECREF and Python-call helper symbols.
    let builtins = py
        .import("builtins")
        .map_err(|err| format!("failed to import builtins for JIT preflight: {err}"))?;
    let len_fn = builtins
        .getattr("len")
        .map_err(|err| format!("failed to resolve builtins.len for JIT preflight: {err}"))?;
    let arg = PyList::new(py, [1_i64, 2, 3])
        .map_err(|err| format!("failed to build list arg for JIT preflight: {err}"))?;
    let expected = 3_i64.into_pyobject(py).map_err(|err| {
        format!("failed to build expected result object for JIT preflight: {err}")
    })?;
    unsafe {
        soac_eval::jit::run_cranelift_python_call_smoke(
            len_fn.as_ptr() as *mut c_void,
            arg.as_ptr() as *mut c_void,
            expected.as_ptr() as *mut c_void,
            preflight_incref,
            preflight_decref,
            preflight_call_one_arg,
            preflight_compare_eq,
        )?;
    }
    Ok(())
}

fn run_specialized_jit(
    py: Python<'_>,
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

    unsafe extern "C" fn compare_eq_obj_hook(lhs: *mut c_void, rhs: *mut c_void) -> *mut c_void {
        if lhs.is_null() || rhs.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid arguments to dp_jit_compare_eq_obj\0".as_ptr() as *const i8,
            );
            return std::ptr::null_mut();
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
            return std::ptr::null_mut();
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
        compare_eq_obj: compare_eq_obj_hook,
        compare_lt_obj: compare_lt_obj_hook,
        raise_from_exc: raise_from_exc_hook,
    };

    let none_obj = py.None();
    let empty_tuple_obj = PyTuple::empty(py);
    let result_ptr = unsafe {
        soac_eval::jit::run_cranelift_run_bb_specialized(
            resolved.block_ptrs.as_slice(),
            &resolved.plan,
            globals_obj.as_ptr() as *mut c_void,
            resolved.true_obj,
            resolved.false_obj,
            args.as_ptr() as *mut c_void,
            &hooks,
            none_obj.as_ptr() as *mut c_void,
            empty_tuple_obj.as_ptr() as *mut c_void,
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
    run_specialized_jit(py, globals_obj, args, resolved)
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

fn eval_source_impl_with_name_and_spec(
    py: Python<'_>,
    path: &str,
    source: &str,
    name: &str,
    package: Option<&str>,
    spec_opt: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let lowering = lower_for_execution(source).map_err(TransformError::to_py_err)?;
    let bb_module = lowering.bb_module;
    validate_bb_module_for_jit(bb_module.as_ref()).map_err(PyRuntimeError::new_err)?;
    if let Some(bb_module) = bb_module.as_ref() {
        let normalized = normalize_bb_module_for_codegen(bb_module);
        soac_eval::jit::register_clif_module_plans(name, &normalized).map_err(|err| {
            PyRuntimeError::new_err(format!("Cranelift JIT plan registration failed: {err}"))
        })?;
    }
    run_cranelift_jit_preflight(bb_module.as_ref()).map_err(|err| {
        PyRuntimeError::new_err(format!("Cranelift JIT preflight failed: {err}"))
    })?;
    run_cranelift_python_call_preflight(py).map_err(|err| {
        PyRuntimeError::new_err(format!("Cranelift JIT Python-call preflight failed: {err}"))
    })?;

    unsafe {
        let module = PyModule::new(py, name)?;

        module.setattr("__name__", name)?;

        if let Some(package) = package {
            module.setattr("__package__", package)?;
        }

        module.setattr("__file__", path)?;

        if let Some(docstring) = lowering.module_docstring.as_deref() {
            module.setattr("__doc__", docstring)?;
        } else {
            module.setattr("__doc__", py.None())?;
        }

        let spec = if let Some(spec) = spec_opt {
            spec
        } else {
            let is_package = path.ends_with("__init__.py");
            build_module_spec(py, name, path, is_package)?
        };

        set_spec_initializing(spec.bind(py), true);
        let eval_result = (|| -> PyResult<()> {
            module.setattr("__spec__", spec.bind(py))?;
            match spec.bind(py).getattr("submodule_search_locations") {
                Ok(submodules) => {
                    if !submodules.is_none() {
                        module.setattr("__path__", submodules)?;
                    }
                }
                Err(_) => {
                    ffi::PyErr_Clear();
                }
            }

            let builtins = ffi::PyEval_GetBuiltins();
            if builtins.is_null() {
                return Err(PyErr::fetch(py));
            }
            let builtins_dict =
                Bound::<PyAny>::from_borrowed_ptr(py, builtins).cast_into::<PyDict>()?;
            module.setattr("__builtins__", &builtins_dict)?;

            let dp_module = py.import("__dp__")?;
            let runtime_module = py.import("diet_python")?;
            let jit_run_bb_plan = runtime_module.getattr("jit_run_bb_plan")?;
            let jit_render_bb_plan = runtime_module.getattr("jit_render_bb_plan")?;
            let jit_has_bb_plan = runtime_module.getattr("jit_has_bb_plan")?;
            let jit_block_param_names = runtime_module.getattr("jit_block_param_names")?;
            let register_clif_wrapper = runtime_module.getattr("register_clif_wrapper")?;
            dp_module.setattr("_jit_run_bb_plan", jit_run_bb_plan)?;
            dp_module.setattr("_jit_render_bb_plan", jit_render_bb_plan)?;
            dp_module.setattr("_jit_has_bb_plan", jit_has_bb_plan)?;
            dp_module.setattr("_jit_block_param_names", jit_block_param_names)?;
            dp_module.setattr("_register_clif_wrapper", register_clif_wrapper)?;

            let module_dict = module.dict();
            let name_cstr =
                CString::new(name).map_err(|_| PyRuntimeError::new_err("invalid __name__"))?;
            let modules = ffi::PyImport_GetModuleDict();
            if modules.is_null()
                || ffi::PyDict_SetItemString(modules, name_cstr.as_ptr(), module.as_any().as_ptr())
                    != 0
            {
                return Err(PyErr::fetch(py));
            }

            let bb_module_ref = bb_module.as_ref().ok_or_else(|| {
                PyRuntimeError::new_err(
                    "JIT mode requires emitted basic-block IR, but none was produced",
                )
            })?;
            // JIT-mode module init executes BB plans directly, so rendered
            // `_dp_bb_*` Python functions do not exist. Seed block labels as
            // string placeholders so name loads in transformed DefFn/DefGen
            // calls resolve to labels consumed by `__dp_def_*`.
            for function in &bb_module_ref.functions {
                for block in &function.blocks {
                    module_dict.set_item(block.label.as_str(), block.label.as_str())?;
                }
            }
            let run_result = if let Some(module_init) = bb_module_ref.module_init.as_deref() {
                let init_args = PyTuple::empty(py);
                jit_run_bb_plan_impl(
                    py,
                    name,
                    module_init,
                    module_dict.as_any(),
                    init_args.as_any(),
                )
                .map(|_| ())
            } else {
                Ok(())
            };
            if let Err(err) = run_result {
                ffi::PyDict_DelItemString(modules, name_cstr.as_ptr());
                return Err(err);
            }

            Ok(())
        })();

        set_spec_initializing(spec.bind(py), false);
        eval_result?;
        Ok(module.unbind().into_any())
    }
}

pub(crate) fn eval_source_impl_with_name(
    py: Python<'_>,
    path: &str,
    source: &str,
    name: &str,
    package: Option<&str>,
) -> PyResult<Py<PyAny>> {
    eval_source_impl_with_name_and_spec(py, path, source, name, package, None)
}

pub(crate) fn eval_source_impl_with_spec(
    py: Python<'_>,
    path: &str,
    source: &str,
    name: &str,
    package: Option<&str>,
    spec: Py<PyAny>,
) -> PyResult<Py<PyAny>> {
    eval_source_impl_with_name_and_spec(py, path, source, name, package, Some(spec))
}

#[cfg(test)]
mod tests {
    use super::{parse_and_lower, run_cranelift_jit_preflight, validate_bb_module_for_jit};
    use dp_transform::basic_block::bb_ir;

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

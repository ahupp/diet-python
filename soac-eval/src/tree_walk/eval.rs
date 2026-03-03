use super::*;
use crate::code_extra::{
    SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER, SOAC_CODE_EXTRA_KIND_FUNCTION_DATA, code_extra_index,
    get_code_extra, set_code_extra,
};
use crate::jit::{self, EntryBlockPlan};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyCode, PyDict, PyString, PyTuple};
use std::any::Any;

type TypeParamScope = HashMap<String, *mut ffi::PyObject>;

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
    fn PyCell_New(obj: *mut ffi::PyObject) -> *mut ffi::PyObject;
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

pub(crate) struct ClosureScope {
    pub(crate) _layout: Box<ScopeLayout>,
    pub(crate) scope: ScopeInstance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParamKind {
    Positional,
    VarArg,
    KwOnly,
    KwArg,
}

pub(crate) struct ParamSpec {
    pub(crate) name: String,
    pub(crate) kind: ParamKind,
    pub(crate) default: Option<*mut ffi::PyObject>,
}

impl ParamSpec {
    fn drop_default(&mut self) {
        if let Some(value) = self.default.take() {
            unsafe { ffi::Py_DECREF(value) };
        }
    }
}

pub(crate) struct FunctionData {
    pub(crate) def: min_ast::FunctionDef,
    pub(crate) params: Vec<ParamSpec>,
    pub(crate) param_layout: Box<ScopeLayout>,
    pub(crate) local_layout: Box<ScopeLayout>,
    pub(crate) cellvars: HashSet<String>,
    pub(crate) closure: Option<ClosureScope>,
    pub(crate) type_params: Option<TypeParamState>,
    pub(crate) globals_scope: *mut ScopeInstance,
    pub(crate) globals_dict: *mut ffi::PyObject,
    pub(crate) builtins: *mut ffi::PyObject,
    pub(crate) function_codes: *mut ffi::PyObject,
    pub(crate) runtime_fns: RuntimeFns,
}

struct TypeParamState {
    map: TypeParamScope,
    ordered: Vec<*mut ffi::PyObject>,
}

impl Drop for TypeParamState {
    fn drop(&mut self) {
        unsafe {
            for value in self.ordered.drain(..) {
                ffi::Py_DECREF(value);
            }
        }
    }
}

impl Drop for FunctionData {
    fn drop(&mut self) {
        unsafe {
            for param in &mut self.params {
                param.drop_default();
            }
            if !self.globals_dict.is_null() {
                ffi::Py_DECREF(self.globals_dict);
            }
            ffi::Py_DECREF(self.builtins);
            if !self.function_codes.is_null() {
                ffi::Py_DECREF(self.function_codes);
            }
        }
    }
}

pub struct ExecContext<'a> {
    globals_scope: *mut ScopeInstance,
    globals_dict: *mut ffi::PyObject,
    params: *mut ScopeInstance,
    locals: *mut ScopeInstance,
    builtins: *mut ffi::PyObject,
    function_codes: *mut ffi::PyObject,
    closure: Option<&'a ScopeInstance>,
    cellvars: Option<&'a HashSet<String>>,
    runtime_fns: &'a RuntimeFns,
    type_params: Option<&'a TypeParamScope>,
}

#[derive(Debug, PartialEq, Eq)]
enum StmtFlow {
    Normal,
    Break,
    Continue,
    Return(*mut ffi::PyObject),
}

pub(crate) fn set_type_error<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(ffi::PyExc_TypeError, CString::new(msg).unwrap().as_ptr());
    }
    Err(())
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

fn set_unbound_local<T>(name: &str) -> Result<T, ()> {
    unsafe {
        let msg = CString::new(format!(
            "local variable '{name}' referenced before assignment"
        ))
        .unwrap();
        ffi::PyErr_SetString(ffi::PyExc_UnboundLocalError, msg.as_ptr());
    }
    Err(())
}

fn set_not_implemented<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(
            ffi::PyExc_NotImplementedError,
            CString::new(msg).unwrap().as_ptr(),
        );
    }
    Err(())
}

fn is_cellvar_name(ctx: &ExecContext<'_>, name: &str) -> bool {
    !closure_name_requires_cell(name)
        && ctx
            .cellvars
            .map(|cellvars| cellvars.contains(name))
            .unwrap_or(false)
}

unsafe fn initialize_cellvars_for_function(
    data: &FunctionData,
    params_scope: *mut ScopeInstance,
    locals_scope: *mut ScopeInstance,
) -> Result<(), ()> {
    for name in &data.def.cellvars {
        if closure_name_requires_cell(name.as_str()) {
            continue;
        }
        let initial = if params_scope.is_null() {
            ptr::null_mut()
        } else {
            scope_lookup_name(&*params_scope, name.as_str())
        };
        let cell = PyCell_New(initial);
        if !initial.is_null() {
            ffi::Py_DECREF(initial);
        }
        if cell.is_null() {
            return Err(());
        }
        if scope_assign_name(&mut *locals_scope, name.as_str(), cell).is_err() {
            ffi::Py_DECREF(cell);
            return Err(());
        }
        ffi::Py_DECREF(cell);
    }
    Ok(())
}

unsafe fn get_frame_data_for_code(code: *mut ffi::PyObject) -> Option<*const FunctionData> {
    let code_extra = match unsafe { get_code_extra(code) } {
        Some(view) => view,
        None => return None,
    };
    if code_extra.kind != SOAC_CODE_EXTRA_KIND_FUNCTION_DATA || code_extra.data.is_null() {
        return None;
    }
    Some(code_extra.data as *const FunctionData)
}

unsafe extern "C" fn free_function_data(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(ptr as *mut FunctionData) });
}

struct ClifWrapperData {
    plan: EntryBlockPlan,
    true_obj: *mut ffi::PyObject,
    false_obj: *mut ffi::PyObject,
    compiled_handle: *mut c_void,
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
        let args_obj = unsafe { frame_var_get_required_bound(py, frame_obj, "args") }?;
        let kwargs_obj = unsafe { frame_var_get_optional_bound(py, frame_obj, "kwargs") }?;
        let sig_obj = unsafe { frame_var_get_required_bound(py, frame_obj, "__dp_sig") }?;
        let state_order_obj =
            unsafe { frame_var_get_required_bound(py, frame_obj, "__dp_state_order") }?;
        let closure_obj = unsafe { frame_var_get_required_bound(py, frame_obj, "__dp_closure") }?;
        let build_entry_args_obj =
            unsafe { frame_var_get_required_bound(py, frame_obj, "__dp_build_entry_args") }?;

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

        let empty_tuple_obj = PyTuple::empty(py);
        let globals_obj = unsafe { ffi::PyFrame_GetGlobals(frame_obj) };
        if globals_obj.is_null() {
            return Err(PyErr::fetch(py));
        }
        let is_module_init_entry = clif_data
            .plan
            .block_labels
            .get(clif_data.plan.entry_index)
            .is_some_and(|label| label.contains("_dp_module_init"));
        if !is_module_init_entry && clif_data.compiled_handle.is_null() {
            let block_ptrs = vec![ptr::null_mut::<c_void>(); clif_data.plan.block_labels.len()];
            clif_data.compiled_handle = match unsafe {
                jit::compile_cranelift_run_bb_specialized_cached(
                    block_ptrs.as_slice(),
                    &clif_data.plan,
                    globals_obj as *mut c_void,
                    clif_data.true_obj as *mut c_void,
                    clif_data.false_obj as *mut c_void,
                    py.None().as_ptr() as *mut c_void,
                    empty_tuple_obj.as_ptr() as *mut c_void,
                )
            } {
                Ok(handle) => handle,
                Err(err) => {
                    unsafe { ffi::Py_DECREF(globals_obj) };
                    return Err(PyRuntimeError::new_err(err));
                }
            };
        }
        let result_ptr = match unsafe {
            if is_module_init_entry {
                let block_ptrs = vec![ptr::null_mut::<c_void>(); clif_data.plan.block_labels.len()];
                jit::run_cranelift_run_bb_specialized(
                    block_ptrs.as_slice(),
                    &clif_data.plan,
                    globals_obj as *mut c_void,
                    clif_data.true_obj as *mut c_void,
                    clif_data.false_obj as *mut c_void,
                    bb_args.as_ptr() as *mut c_void,
                    jit_incref,
                    jit_decref,
                    py_call_three_hook,
                    py_call_object_hook,
                    py_call_with_kw_hook,
                    py_get_raised_exception_hook,
                    get_arg_item_hook,
                    make_int_hook,
                    make_float_hook,
                    make_bytes_hook,
                    load_name_hook,
                    load_local_raw_by_name_hook,
                    pyobject_getattr_hook,
                    pyobject_setattr_hook,
                    pyobject_getitem_hook,
                    pyobject_setitem_hook,
                    pyobject_to_i64_hook,
                    decode_literal_bytes_hook,
                    tuple_new_hook,
                    tuple_set_item_hook,
                    is_true_hook,
                    compare_eq_obj_hook,
                    compare_lt_obj_hook,
                    raise_from_exc_hook,
                    py.None().as_ptr() as *mut c_void,
                    empty_tuple_obj.as_ptr() as *mut c_void,
                )
            } else {
                jit::run_cranelift_run_bb_specialized_cached(
                    clif_data.compiled_handle,
                    bb_args.as_ptr() as *mut c_void,
                    jit_incref,
                    jit_decref,
                    py_call_three_hook,
                    py_call_object_hook,
                    py_call_with_kw_hook,
                    py_get_raised_exception_hook,
                    get_arg_item_hook,
                    make_int_hook,
                    make_float_hook,
                    make_bytes_hook,
                    load_name_hook,
                    load_local_raw_by_name_hook,
                    pyobject_getattr_hook,
                    pyobject_setattr_hook,
                    pyobject_getitem_hook,
                    pyobject_setitem_hook,
                    pyobject_to_i64_hook,
                    decode_literal_bytes_hook,
                    tuple_new_hook,
                    tuple_set_item_hook,
                    is_true_hook,
                    compare_eq_obj_hook,
                    compare_lt_obj_hook,
                    raise_from_exc_hook,
                )
            }
        } {
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

unsafe fn eval_frame_with_data(
    data: &FunctionData,
    frame_obj: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    let frame = frame_obj as *mut ffi::PyFrameObject;
    // Use the active Python frame mappings so eval-mode name resolution matches
    // real FunctionType execution when globals/builtins differ per call frame.
    let globals_dict = ffi::PyFrame_GetGlobals(frame);
    if globals_dict.is_null() {
        return ptr::null_mut();
    }
    let builtins = ffi::PyFrame_GetBuiltins(frame);
    if builtins.is_null() {
        ffi::Py_DECREF(globals_dict);
        return ptr::null_mut();
    }

    let mut params_scope = ScopeInstance::new(&*data.param_layout);
    let params_ptr = &mut params_scope as *mut ScopeInstance;
    for param in &data.params {
        let value = match frame_var_get_optional(frame, param.name.as_str()) {
            Ok(value) => value,
            Err(()) => {
                ffi::Py_DECREF(globals_dict);
                ffi::Py_DECREF(builtins);
                return ptr::null_mut();
            }
        };
        if value.is_null() {
            continue;
        }
        if scope_assign_name(&mut params_scope, param.name.as_str(), value).is_err() {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(globals_dict);
            ffi::Py_DECREF(builtins);
            return ptr::null_mut();
        }
        ffi::Py_DECREF(value);
    }

    if apply_param_defaults(&data.params, params_ptr).is_err() {
        ffi::Py_DECREF(globals_dict);
        ffi::Py_DECREF(builtins);
        return ptr::null_mut();
    }

    let mut locals_box = Box::new(ScopeInstance::new(&*data.local_layout));
    let locals_ptr = locals_box.as_mut() as *mut ScopeInstance;
    if initialize_cellvars_for_function(data, params_ptr, locals_ptr).is_err() {
        ffi::Py_DECREF(globals_dict);
        ffi::Py_DECREF(builtins);
        return ptr::null_mut();
    }
    let ctx = ExecContext {
        globals_scope: data.globals_scope,
        globals_dict,
        params: params_ptr,
        locals: locals_ptr,
        builtins,
        function_codes: data.function_codes,
        closure: data.closure.as_ref().map(|closure| &closure.scope),
        cellvars: Some(&data.cellvars),
        runtime_fns: &data.runtime_fns,
        type_params: None,
    };

    let result = match eval_block(&data.def.body, &ctx) {
        Ok(StmtFlow::Return(value)) => value,
        Ok(StmtFlow::Normal) => {
            ffi::Py_INCREF(ffi::Py_None());
            ffi::Py_None()
        }
        Ok(StmtFlow::Break) | Ok(StmtFlow::Continue) => {
            let _ = set_runtime_error::<()>("break/continue outside loop");
            ptr::null_mut()
        }
        Err(()) => ptr::null_mut(),
    };

    drop(locals_box);
    ffi::Py_DECREF(globals_dict);
    ffi::Py_DECREF(builtins);
    result
}

unsafe fn eval_frame_with_data_no_frame(data: &FunctionData) -> *mut ffi::PyObject {
    let mut params_scope = ScopeInstance::new(&*data.param_layout);
    let params_ptr = &mut params_scope as *mut ScopeInstance;
    if apply_param_defaults(&data.params, params_ptr).is_err() {
        return ptr::null_mut();
    }

    let mut locals_box = Box::new(ScopeInstance::new(&*data.local_layout));
    let locals_ptr = locals_box.as_mut() as *mut ScopeInstance;
    if initialize_cellvars_for_function(data, params_ptr, locals_ptr).is_err() {
        return ptr::null_mut();
    }
    let ctx = ExecContext {
        globals_scope: data.globals_scope,
        globals_dict: data.globals_dict,
        params: params_ptr,
        locals: locals_ptr,
        builtins: data.builtins,
        function_codes: data.function_codes,
        closure: data.closure.as_ref().map(|closure| &closure.scope),
        cellvars: Some(&data.cellvars),
        runtime_fns: &data.runtime_fns,
        type_params: None,
    };

    let result = match eval_block(&data.def.body, &ctx) {
        Ok(StmtFlow::Return(value)) => value,
        Ok(StmtFlow::Normal) => {
            ffi::Py_INCREF(ffi::Py_None());
            ffi::Py_None()
        }
        Ok(StmtFlow::Break) | Ok(StmtFlow::Continue) => {
            let _ = set_runtime_error::<()>("break/continue outside loop");
            ptr::null_mut()
        }
        Err(()) => ptr::null_mut(),
    };

    drop(locals_box);
    result
}

unsafe fn find_matching_soac_frame(
    tstate: *mut ffi::PyThreadState,
    data_ptr: *const FunctionData,
) -> *mut ffi::PyFrameObject {
    let mut frame_obj = ffi::PyThreadState_GetFrame(tstate);
    while !frame_obj.is_null() {
        let code = ffi::PyFrame_GetCode(frame_obj);
        if !code.is_null() {
            let code_obj = code as *mut ffi::PyObject;
            let frame_data = get_frame_data_for_code(code_obj);
            ffi::Py_DECREF(code_obj);
            if let Some(candidate) = frame_data {
                if std::ptr::eq(candidate, data_ptr) {
                    return frame_obj;
                }
            }
        } else {
            ffi::PyErr_Clear();
        }

        let back = ffi::PyFrame_GetBack(frame_obj);
        ffi::Py_DECREF(frame_obj as *mut ffi::PyObject);
        frame_obj = back;
    }
    ptr::null_mut()
}

unsafe fn finalize_soac_frame(
    tstate: *mut ffi::PyThreadState,
    frame: *mut ffi::_PyInterpreterFrame,
) {
    _PyEval_FrameClearAndPop(tstate, frame);
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
        if extra.kind == SOAC_CODE_EXTRA_KIND_FUNCTION_DATA {
            let data_ptr = extra.data as *const FunctionData;
            if data_ptr.is_null() || throwflag != 0 || (*data_ptr).def.is_async {
                return _PyEval_EvalFrameDefault(tstate, frame, throwflag);
            }
            if (*data_ptr).params.is_empty() {
                let result = eval_frame_with_data_no_frame(&*data_ptr);
                finalize_soac_frame(tstate, frame);
                return result;
            }
            let mut frame_obj = find_matching_soac_frame(tstate, data_ptr);
            if frame_obj.is_null() {
                frame_obj = _PyFrame_MakeAndSetFrameObject(frame);
                if frame_obj.is_null() {
                    return ptr::null_mut();
                }
                ffi::Py_INCREF(frame_obj as *mut ffi::PyObject);
            }
            let result = eval_frame_with_data(&*data_ptr, frame_obj as *mut ffi::PyObject);
            ffi::Py_DECREF(frame_obj as *mut ffi::PyObject);
            finalize_soac_frame(tstate, frame);
            return result;
        }
        if extra.kind == SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER {
            if throwflag != 0 {
                return _PyEval_EvalFrameDefault(tstate, frame, throwflag);
            }
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

pub unsafe fn register_clif_wrapper_code_extra(function: *mut ffi::PyObject) -> Result<(), ()> {
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
    let module_name = function_bound
        .getattr("__dp_plan_module")
        .ok()
        .and_then(|obj| obj.extract::<String>().ok())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            function_bound
                .getattr("__module__")
                .ok()
                .and_then(|obj| obj.extract::<String>().ok())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default();
    let qualname = function_bound
        .getattr("__dp_plan_qualname")
        .ok()
        .and_then(|obj| obj.extract::<String>().ok())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            function_bound
                .getattr("__qualname__")
                .ok()
                .and_then(|obj| obj.extract::<String>().ok())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default();
    let plan = jit::lookup_bb_plan(module_name.as_str(), qualname.as_str());
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
    if plan
        .block_fast_paths
        .iter()
        .any(|path| matches!(path, jit::BlockFastPath::None))
    {
        // Wrapper shape is unsupported for specialized CLIF execution.
        // Leave this wrapper on default Python execution path.
        return Ok(());
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
        true_obj,
        false_obj,
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

fn context_with_type_params<'a>(
    ctx: &ExecContext<'a>,
    type_params: &'a TypeParamScope,
) -> ExecContext<'a> {
    ExecContext {
        globals_scope: ctx.globals_scope,
        globals_dict: ctx.globals_dict,
        params: ctx.params,
        locals: ctx.locals,
        builtins: ctx.builtins,
        function_codes: ctx.function_codes,
        closure: ctx.closure,
        cellvars: ctx.cellvars,
        runtime_fns: ctx.runtime_fns,
        type_params: Some(type_params),
    }
}

pub(crate) fn exec_context_for_scopes<'a>(
    data: &'a FunctionData,
    params: *mut ScopeInstance,
    locals: *mut ScopeInstance,
) -> ExecContext<'a> {
    ExecContext {
        globals_scope: data.globals_scope,
        globals_dict: data.globals_dict,
        params,
        locals,
        builtins: data.builtins,
        function_codes: data.function_codes,
        closure: data.closure.as_ref().map(|closure| &closure.scope),
        cellvars: Some(&data.cellvars),
        runtime_fns: &data.runtime_fns,
        type_params: None,
    }
}

unsafe fn build_type_params(
    def: &min_ast::FunctionDef,
    ctx: &ExecContext<'_>,
) -> Result<Option<TypeParamState>, ()> {
    if def.type_params.is_empty() {
        return Ok(None);
    }

    let typing = ffi::PyImport_ImportModule(b"typing\0".as_ptr() as *const c_char);
    if typing.is_null() {
        return Err(());
    }
    let type_var = ffi::PyObject_GetAttrString(typing, b"TypeVar\0".as_ptr() as *const c_char);
    let type_var_tuple =
        ffi::PyObject_GetAttrString(typing, b"TypeVarTuple\0".as_ptr() as *const c_char);
    let param_spec = ffi::PyObject_GetAttrString(typing, b"ParamSpec\0".as_ptr() as *const c_char);

    let mut state = TypeParamState {
        map: HashMap::new(),
        ordered: Vec::new(),
    };

    let mut ok = true;
    for param in &def.type_params {
        if !ok {
            break;
        }
        let (name, bound, default, factory) = match param {
            min_ast::TypeParam::TypeVar {
                name,
                bound,
                default,
            } => {
                if type_var.is_null() {
                    ok = false;
                    break;
                }
                (name.as_str(), bound.as_ref(), default.as_ref(), type_var)
            }
            min_ast::TypeParam::TypeVarTuple { name, default } => {
                if type_var_tuple.is_null() {
                    ok = false;
                    break;
                }
                (name.as_str(), None, default.as_ref(), type_var_tuple)
            }
            min_ast::TypeParam::ParamSpec { name, default } => {
                if param_spec.is_null() {
                    ok = false;
                    break;
                }
                (name.as_str(), None, default.as_ref(), param_spec)
            }
        };

        let name_obj =
            ffi::PyUnicode_FromString(CString::new(name).unwrap().as_ptr() as *const c_char);
        if name_obj.is_null() {
            ok = false;
            break;
        }

        let args = ffi::PyTuple_New(1);
        if args.is_null() {
            ffi::Py_DECREF(name_obj);
            ok = false;
            break;
        }
        ffi::PyTuple_SetItem(args, 0, name_obj);

        let kwargs_needed = bound.is_some() || default.is_some();
        let kwargs = if kwargs_needed {
            let dict = ffi::PyDict_New();
            if dict.is_null() {
                ffi::Py_DECREF(args);
                ok = false;
                break;
            }
            dict
        } else {
            ptr::null_mut()
        };

        if let Some(bound_expr) = bound {
            let ann_ctx = context_with_type_params(ctx, &state.map);
            let bound_value = match eval_expr(bound_expr, &ann_ctx) {
                Ok(value) => value,
                Err(()) => {
                    ffi::Py_DECREF(args);
                    if !kwargs.is_null() {
                        ffi::Py_DECREF(kwargs);
                    }
                    ok = false;
                    break;
                }
            };
            if ffi::PyDict_SetItemString(kwargs, b"bound\0".as_ptr() as *const c_char, bound_value)
                != 0
            {
                ffi::Py_DECREF(bound_value);
                ffi::Py_DECREF(args);
                if !kwargs.is_null() {
                    ffi::Py_DECREF(kwargs);
                }
                ok = false;
                break;
            }
            ffi::Py_DECREF(bound_value);
        }

        if let Some(default_expr) = default {
            let ann_ctx = context_with_type_params(ctx, &state.map);
            let default_value = match eval_expr(default_expr, &ann_ctx) {
                Ok(value) => value,
                Err(()) => {
                    ffi::Py_DECREF(args);
                    if !kwargs.is_null() {
                        ffi::Py_DECREF(kwargs);
                    }
                    ok = false;
                    break;
                }
            };
            if ffi::PyDict_SetItemString(
                kwargs,
                b"default\0".as_ptr() as *const c_char,
                default_value,
            ) != 0
            {
                ffi::Py_DECREF(default_value);
                ffi::Py_DECREF(args);
                if !kwargs.is_null() {
                    ffi::Py_DECREF(kwargs);
                }
                ok = false;
                break;
            }
            ffi::Py_DECREF(default_value);
        }

        let obj = ffi::PyObject_Call(factory, args, kwargs);
        ffi::Py_DECREF(args);
        if !kwargs.is_null() {
            ffi::Py_DECREF(kwargs);
        }
        if obj.is_null() {
            ok = false;
            break;
        }
        state.map.insert(name.to_string(), obj);
        state.ordered.push(obj);
    }

    ffi::Py_XDECREF(type_var);
    ffi::Py_XDECREF(type_var_tuple);
    ffi::Py_XDECREF(param_spec);
    ffi::Py_DECREF(typing);

    if !ok {
        return Err(());
    }

    Ok(Some(state))
}

fn collect_bound_names(stmts: &[min_ast::StmtNode], names: &mut HashSet<String>) {
    fn add_assign_target_names(target: &min_ast::AssignTarget, names: &mut HashSet<String>) {
        match target {
            min_ast::AssignTarget::Name(name) => {
                names.insert(name.clone());
            }
            min_ast::AssignTarget::Unpack(targets) | min_ast::AssignTarget::Chained(targets) => {
                for target in targets {
                    names.insert(target.clone());
                }
            }
        }
    }

    for stmt in stmts {
        match stmt {
            min_ast::StmtNode::Assign { target, .. } => add_assign_target_names(target, names),
            min_ast::StmtNode::Delete { target, .. } => {
                names.insert(target.clone());
            }
            min_ast::StmtNode::FunctionDef(func) => {
                names.insert(func.name.clone());
            }
            min_ast::StmtNode::While { body, orelse, .. }
            | min_ast::StmtNode::If { body, orelse, .. } => {
                collect_bound_names(body, names);
                collect_bound_names(orelse, names);
            }
            min_ast::StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
                ..
            } => {
                collect_bound_names(body, names);
                if let Some(handler) = handler {
                    collect_bound_names(handler, names);
                }
                collect_bound_names(orelse, names);
                collect_bound_names(finalbody, names);
            }
            _ => {}
        }
    }
}

fn collect_local_names(def: &min_ast::FunctionDef) -> HashSet<String> {
    let mut locals = HashSet::new();
    for param in &def.params {
        match param {
            min_ast::Parameter::Positional { name, .. }
            | min_ast::Parameter::VarArg { name, .. }
            | min_ast::Parameter::KwOnly { name, .. }
            | min_ast::Parameter::KwArg { name, .. } => {
                locals.insert(name.clone());
            }
        }
    }
    collect_bound_names(&def.body, &mut locals);
    locals
}

pub fn build_module_layout(module: &min_ast::Module) -> ScopeLayout {
    let mut names = HashSet::new();
    collect_bound_names(&module.body, &mut names);
    ScopeLayout::new(names)
}

fn capture_closure(freevars: &[String], ctx: &ExecContext<'_>) -> Result<Option<ClosureScope>, ()> {
    unsafe fn cleanup_captured(captured: &mut Vec<(String, *mut ffi::PyObject)>) {
        for (_, value) in captured.drain(..) {
            ffi::Py_DECREF(value);
        }
    }

    unsafe fn capture_if_cell(
        captured: &mut Vec<(String, *mut ffi::PyObject)>,
        name: &str,
        value: *mut ffi::PyObject,
    ) -> Result<bool, ()> {
        if value.is_null() {
            return Ok(false);
        }
        if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) == 0 {
            if closure_name_requires_cell(name) {
                ffi::Py_DECREF(value);
                cleanup_captured(captured);
                set_runtime_error("closure values must be cells")?;
            }
            // `_dp_cell_*` / `_dp_classcell` are explicit transformed cell bindings and must
            // already be cells. Other captured names can be implicit CPython cellvars
            // (for example nested def/class binding names), so eval mode promotes them.
            let promoted = PyCell_New(value);
            ffi::Py_DECREF(value);
            if promoted.is_null() {
                cleanup_captured(captured);
                return Err(());
            }
            captured.push((name.to_string(), promoted));
            return Ok(true);
        }
        captured.push((name.to_string(), value));
        Ok(true)
    }

    let mut captured = Vec::new();
    for name in freevars {
        unsafe {
            if ctx.locals != ctx.globals_scope {
                let locals_scope = &*ctx.locals;
                let value = scope_lookup_name(locals_scope, name);
                if capture_if_cell(&mut captured, name.as_str(), value)? {
                    continue;
                }
            }
            if !ctx.params.is_null() {
                let params_scope = &*ctx.params;
                let value = scope_lookup_name(params_scope, name);
                if capture_if_cell(&mut captured, name.as_str(), value)? {
                    continue;
                }
            }
            if let Some(closure) = ctx.closure {
                let value = scope_lookup_name(closure, name);
                if capture_if_cell(&mut captured, name.as_str(), value)? {
                    continue;
                }
            }
        }
    }

    if captured.is_empty() {
        return Ok(None);
    }

    let mut names = HashSet::new();
    for (name, _) in &captured {
        names.insert(name.clone());
    }
    let layout = Box::new(ScopeLayout::new(names));
    let mut scope = ScopeInstance::new(&*layout);
    for (idx, (name, value)) in captured.iter().enumerate() {
        let result = unsafe { scope_assign_name(&mut scope, name.as_str(), *value) };
        if result.is_err() {
            for (_, value) in captured[idx..].iter() {
                unsafe { ffi::Py_DECREF(*value) };
            }
            return Err(());
        }
        unsafe { ffi::Py_DECREF(*value) };
    }

    Ok(Some(ClosureScope {
        _layout: layout,
        scope,
    }))
}

impl FunctionData {
    pub(crate) unsafe fn call_from_python(
        &self,
        args: *mut ffi::PyObject,
        kwargs: *mut ffi::PyObject,
    ) -> *mut ffi::PyObject {
        if self.def.is_async {
            ffi::PyErr_SetString(
                ffi::PyExc_NotImplementedError,
                b"async functions not supported\0".as_ptr() as *const c_char,
            );
            return ptr::null_mut();
        }

        let mut params_scope = ScopeInstance::new(&*self.param_layout);
        let params_ptr = &mut params_scope as *mut ScopeInstance;

        if bind_args(&self.params, args, kwargs, params_ptr).is_err() {
            if std::env::var_os("DIET_PYTHON_DEBUG_BIND").is_some() {
                eprintln!("bind_args failed for {}", self.def.name);
            }
            return ptr::null_mut();
        }

        self.call(params_ptr)
    }

    unsafe fn call(&self, params_scope: *mut ScopeInstance) -> *mut ffi::PyObject {
        if apply_param_defaults(&self.params, params_scope).is_err() {
            return ptr::null_mut();
        }

        let mut locals_box = Box::new(ScopeInstance::new(&*self.local_layout));
        let locals_ptr = locals_box.as_mut() as *mut ScopeInstance;
        if initialize_cellvars_for_function(self, params_scope, locals_ptr).is_err() {
            return ptr::null_mut();
        }

        let ctx = ExecContext {
            globals_scope: self.globals_scope,
            globals_dict: self.globals_dict,
            params: params_scope,
            locals: locals_ptr,
            builtins: self.builtins,
            function_codes: self.function_codes,
            closure: self.closure.as_ref().map(|closure| &closure.scope),
            cellvars: Some(&self.cellvars),
            runtime_fns: &self.runtime_fns,
            type_params: None,
        };

        let result = match eval_block(&self.def.body, &ctx) {
            Ok(StmtFlow::Return(value)) => value,
            Ok(StmtFlow::Normal) => {
                ffi::Py_INCREF(ffi::Py_None());
                ffi::Py_None()
            }
            Ok(StmtFlow::Break) | Ok(StmtFlow::Continue) => {
                let _ = set_runtime_error::<()>("break/continue outside loop");
                ptr::null_mut()
            }
            Err(()) => ptr::null_mut(),
        };

        drop(locals_box);
        result
    }
}

pub unsafe fn eval_module(
    module: &min_ast::Module,
    globals_scope: *mut ScopeInstance,
    globals_dict: *mut ffi::PyObject,
    builtins: *mut ffi::PyObject,
    function_codes: *mut ffi::PyObject,
    runtime_fns: &RuntimeFns,
) -> Result<(), ()> {
    let ctx = ExecContext {
        globals_scope,
        globals_dict,
        params: ptr::null_mut(),
        locals: globals_scope,
        builtins,
        function_codes,
        closure: None,
        cellvars: None,
        runtime_fns,
        type_params: None,
    };
    match eval_block(&module.body, &ctx) {
        Ok(StmtFlow::Normal) => Ok(()),
        Ok(StmtFlow::Return(_)) => set_runtime_error("return outside function"),
        Ok(StmtFlow::Break) | Ok(StmtFlow::Continue) => {
            set_runtime_error("break/continue outside loop")
        }
        Err(()) => Err(()),
    }
}

fn normalize_freevar_name(name: &str) -> &str {
    name.strip_prefix("_dp_cell_").unwrap_or(name)
}

fn closure_name_requires_cell(name: &str) -> bool {
    name.starts_with("_dp_cell_") || name == "_dp_classcell"
}

unsafe fn function_defaults_object(params: &[ParamSpec]) -> Result<Py<PyAny>, ()> {
    let py = Python::assume_attached();
    let mut defaults = Vec::new();
    let mut seen_default = false;
    for param in params {
        if param.kind == ParamKind::Positional {
            if let Some(value) = param.default {
                seen_default = true;
                defaults.push(value);
            } else if seen_default {
                break;
            }
        }
    }
    if defaults.is_empty() {
        let none = ffi::Py_None();
        ffi::Py_INCREF(none);
        return Ok(Bound::<PyAny>::from_owned_ptr(py, none).unbind());
    }
    let tuple = ffi::PyTuple_New(defaults.len() as ffi::Py_ssize_t);
    if tuple.is_null() {
        return Err(());
    }
    for (idx, value) in defaults.iter().enumerate() {
        ffi::Py_INCREF(*value);
        if ffi::PyTuple_SetItem(tuple, idx as ffi::Py_ssize_t, *value) != 0 {
            ffi::Py_DECREF(tuple);
            return Err(());
        }
    }
    Ok(Bound::<PyAny>::from_owned_ptr(py, tuple).unbind())
}

unsafe fn function_kwdefaults_object(params: &[ParamSpec]) -> Result<Py<PyAny>, ()> {
    let py = Python::assume_attached();
    let dict = PyDict::new(py);
    let mut has_defaults = false;
    for param in params {
        if param.kind != ParamKind::KwOnly {
            continue;
        }
        let Some(value) = param.default else {
            continue;
        };
        has_defaults = true;
        let value_obj = Bound::<PyAny>::from_borrowed_ptr(py, value);
        if dict.set_item(param.name.as_str(), value_obj).is_err() {
            return Err(());
        }
    }
    if !has_defaults {
        let none = ffi::Py_None();
        ffi::Py_INCREF(none);
        return Ok(Bound::<PyAny>::from_owned_ptr(py, none).unbind());
    }
    Ok(dict.into_any().unbind())
}

unsafe fn function_closure_object(
    data: &FunctionData,
    code: *mut ffi::PyObject,
) -> Result<Py<PyAny>, ()> {
    let py = Python::assume_attached();
    let Some(closure) = data.closure.as_ref() else {
        let none = ffi::Py_None();
        ffi::Py_INCREF(none);
        return Ok(Bound::<PyAny>::from_owned_ptr(py, none).unbind());
    };
    if ffi::PyObject_TypeCheck(code, std::ptr::addr_of_mut!(ffi::PyCode_Type)) != 0 {
        let freevars =
            ffi::PyObject_GetAttrString(code, b"co_freevars\0".as_ptr() as *const c_char);
        if freevars.is_null() {
            return Err(());
        }
        let freevars_len = ffi::PyTuple_Size(freevars);
        if freevars_len < 0 {
            ffi::Py_DECREF(freevars);
            return Err(());
        }
        let tuple = ffi::PyTuple_New(freevars_len);
        if tuple.is_null() {
            ffi::Py_DECREF(freevars);
            return Err(());
        }
        for idx in 0..freevars_len {
            let name_obj = ffi::PyTuple_GetItem(freevars, idx);
            if name_obj.is_null() || ffi::PyUnicode_Check(name_obj) == 0 {
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                ffi::PyErr_SetString(
                    ffi::PyExc_TypeError,
                    b"co_freevars must contain strings\0".as_ptr() as *const c_char,
                );
                return Err(());
            }
            let name_ptr = ffi::PyUnicode_AsUTF8(name_obj);
            if name_ptr.is_null() {
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                return Err(());
            }
            let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy();
            let mut value = scope_lookup_name(&closure.scope, name.as_ref());
            if value.is_null() && !name.starts_with("_dp_cell_") {
                let cell_name = format!("_dp_cell_{name}");
                value = scope_lookup_name(&closure.scope, cell_name.as_str());
            }
            if value.is_null() {
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                ffi::PyErr_SetString(
                    ffi::PyExc_RuntimeError,
                    b"closure value missing for compiled freevar\0".as_ptr() as *const c_char,
                );
                return Err(());
            }
            if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) == 0 {
                ffi::Py_DECREF(value);
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                ffi::PyErr_SetString(
                    ffi::PyExc_TypeError,
                    b"closure values must be cells\0".as_ptr() as *const c_char,
                );
                return Err(());
            }
            let tuple_value = if closure_name_requires_cell(name.as_ref()) {
                // Compiled CPython bytecode loads freevars as cell contents. For transformed
                // `_dp_cell_*` bindings we need the value itself to remain a cell object, so
                // wrap the captured cell in an outer closure cell.
                // TODO: eliminate this double-cell representation by aligning transformed
                // closure conventions with CPython freevar expectations directly.
                let wrapped = PyCell_New(value);
                ffi::Py_DECREF(value);
                if wrapped.is_null() {
                    ffi::Py_DECREF(tuple);
                    ffi::Py_DECREF(freevars);
                    return Err(());
                }
                wrapped
            } else {
                value
            };
            if ffi::PyTuple_SetItem(tuple, idx, tuple_value) != 0 {
                ffi::Py_DECREF(tuple_value);
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                return Err(());
            }
        }
        ffi::Py_DECREF(freevars);
        return Ok(Bound::<PyAny>::from_owned_ptr(py, tuple).unbind());
    }

    let tuple = ffi::PyTuple_New(closure._layout.names.len() as ffi::Py_ssize_t);
    if tuple.is_null() {
        return Err(());
    }
    for (idx, name) in closure._layout.names.iter().enumerate() {
        let value = scope_lookup_name(&closure.scope, name.as_str());
        if value.is_null() {
            ffi::Py_DECREF(tuple);
            return Err(());
        }
        if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) == 0 {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(tuple);
            ffi::PyErr_SetString(
                ffi::PyExc_TypeError,
                b"closure values must be cells\0".as_ptr() as *const c_char,
            );
            return Err(());
        }
        if ffi::PyTuple_SetItem(tuple, idx as ffi::Py_ssize_t, value) != 0 {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(tuple);
            return Err(());
        }
    }
    Ok(Bound::<PyAny>::from_owned_ptr(py, tuple).unbind())
}

unsafe fn function_code_object(data: &FunctionData) -> Result<Py<PyAny>, ()> {
    let py = Python::assume_attached();
    if !data.function_codes.is_null() {
        let code_map = Bound::<PyAny>::from_borrowed_ptr(py, data.function_codes)
            .cast_into::<PyDict>()
            .map_err(|_| ())?;
        if let Some(code_obj) = code_map.get_item(data.def.name.as_str()).map_err(|_| ())? {
            let code_obj = match code_obj.cast_into::<PyCode>() {
                Ok(code_obj) => code_obj,
                Err(_) => {
                    ffi::PyErr_SetString(
                        ffi::PyExc_TypeError,
                        b"function code map value is not a code object\0".as_ptr() as *const c_char,
                    );
                    return Err(());
                }
            };
            let kwargs = PyDict::new(py);
            kwargs
                .set_item("co_name", data.def.display_name.as_str())
                .map_err(|_| ())?;
            kwargs
                .set_item("co_qualname", data.def.qualname.as_str())
                .map_err(|_| ())?;
            let replaced = code_obj.call_method("replace", (), Some(&kwargs));
            return match replaced {
                Ok(replaced) => Ok(replaced.unbind()),
                Err(err) => {
                    // Keep compiled code if replace fails.
                    err.restore(py);
                    ffi::PyErr_Clear();
                    Ok(code_obj.into_any().unbind())
                }
            };
        }
    }

    let mut positional = Vec::new();
    let mut kwonly = Vec::new();
    let mut vararg = None;
    let mut kwarg = None;
    for param in &data.params {
        match param.kind {
            ParamKind::Positional => positional.push(param.name.clone()),
            ParamKind::VarArg => vararg = Some(param.name.clone()),
            ParamKind::KwOnly => kwonly.push(param.name.clone()),
            ParamKind::KwArg => kwarg = Some(param.name.clone()),
        }
    }
    let mut varnames = Vec::new();
    varnames.extend(positional.iter().cloned());
    if let Some(name) = vararg.as_ref() {
        varnames.push(name.clone());
    }
    varnames.extend(kwonly.iter().cloned());
    if let Some(name) = kwarg.as_ref() {
        varnames.push(name.clone());
    }

    let globals_dict = Bound::<PyAny>::from_borrowed_ptr(py, data.globals_dict)
        .cast_into::<PyDict>()
        .map_err(|_| ())?;
    let filename_obj: Py<PyAny> = match globals_dict.get_item("__file__").map_err(|_| ())? {
        Some(file_obj) if file_obj.is_instance_of::<PyString>() => file_obj.unbind(),
        _ => PyString::new(py, "<eval>").into_any().unbind(),
    };

    const CO_VARARGS: c_long = 0x04;
    const CO_VARKEYWORDS: c_long = 0x08;
    const CO_COROUTINE: c_long = 0x80;
    let mut flags = 0 as c_long;
    if vararg.is_some() {
        flags |= CO_VARARGS;
    }
    if kwarg.is_some() {
        flags |= CO_VARKEYWORDS;
    }
    if data.def.is_async {
        flags |= CO_COROUTINE;
    }

    let varnames_tuple = PyTuple::new(py, varnames).map_err(|_| ())?;
    let freevar_names = if let Some(closure) = data.closure.as_ref() {
        closure
            ._layout
            .names
            .iter()
            .map(|name| normalize_freevar_name(name.as_str()))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let freevars_tuple = PyTuple::new(py, freevar_names).map_err(|_| ())?;
    let cellvars_tuple = PyTuple::new(py, &data.def.cellvars).map_err(|_| ())?;

    let code = ffi::PyCode_NewEmpty(
        b"<eval>\0".as_ptr() as *const c_char,
        CString::new(data.def.display_name.as_str())
            .unwrap()
            .as_ptr(),
        1,
    ) as *mut ffi::PyObject;
    let code_obj = Bound::<PyAny>::from_owned_ptr_or_opt(py, code).ok_or(())?;
    let code_obj = code_obj.cast_into::<PyCode>().map_err(|_| ())?;

    let kwargs = PyDict::new(py);
    kwargs
        .set_item("co_argcount", positional.len() as c_long)
        .map_err(|_| ())?;
    kwargs.set_item("co_posonlyargcount", 0).map_err(|_| ())?;
    kwargs
        .set_item("co_kwonlyargcount", kwonly.len() as c_long)
        .map_err(|_| ())?;
    kwargs
        .set_item("co_nlocals", varnames_tuple.len())
        .map_err(|_| ())?;
    kwargs.set_item("co_flags", flags).map_err(|_| ())?;
    kwargs
        .set_item("co_varnames", &varnames_tuple)
        .map_err(|_| ())?;
    kwargs
        .set_item("co_freevars", &freevars_tuple)
        .map_err(|_| ())?;
    kwargs
        .set_item("co_cellvars", &cellvars_tuple)
        .map_err(|_| ())?;
    kwargs
        .set_item("co_filename", filename_obj.bind(py))
        .map_err(|_| ())?;
    kwargs
        .set_item("co_name", data.def.display_name.as_str())
        .map_err(|_| ())?;
    kwargs
        .set_item("co_qualname", data.def.qualname.as_str())
        .map_err(|_| ())?;
    kwargs.set_item("co_firstlineno", 1).map_err(|_| ())?;

    let replaced = code_obj
        .call_method("replace", (), Some(&kwargs))
        .map_err(|_| ())?;
    Ok(replaced.unbind())
}

static mut SOAC_FUNCTION_ANNOTATE_PYFUNC_DEF: ffi::PyMethodDef = ffi::PyMethodDef {
    ml_name: b"__annotate__\0".as_ptr() as *const c_char,
    ml_meth: ffi::PyMethodDefPointer {
        PyCFunction: soac_function_annotate_pyfunc,
    },
    ml_flags: ffi::METH_O,
    ml_doc: ptr::null(),
};

unsafe extern "C" fn soac_function_annotate_pyfunc(
    slf: *mut ffi::PyObject,
    arg: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    if arg.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"__annotate__ expects a format\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    }
    let format = ffi::PyLong_AsLong(arg);
    if format == -1 && !ffi::PyErr_Occurred().is_null() {
        return ptr::null_mut();
    }
    if format > 2 {
        ffi::PyErr_SetString(
            ffi::PyExc_NotImplementedError,
            b"format not supported\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    }
    let code = ffi::PyObject_GetAttrString(slf, b"__code__\0".as_ptr() as *const c_char);
    if code.is_null() {
        return ptr::null_mut();
    }
    let data = get_frame_data_for_code(code);
    ffi::Py_DECREF(code);
    let Some(data) = data else {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"soac function missing data\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    };
    match eval_function_annotations(&*data, format as i32) {
        Ok(dict) => dict.into_ptr(),
        Err(()) => ptr::null_mut(),
    }
}

pub unsafe fn build_function(
    def: min_ast::FunctionDef,
    ctx: &ExecContext<'_>,
    module_name: *mut ffi::PyObject,
) -> Result<*mut ffi::PyObject, ()> {
    let local_names = collect_local_names(&def);
    let cellvars = def.cellvars.iter().cloned().collect::<HashSet<_>>();
    let closure = capture_closure(&def.freevars, ctx)?;
    let local_layout = Box::new(ScopeLayout::new(local_names));

    let mut param_names = HashSet::new();
    for param in &def.params {
        match param {
            min_ast::Parameter::Positional { name, .. }
            | min_ast::Parameter::VarArg { name, .. }
            | min_ast::Parameter::KwOnly { name, .. }
            | min_ast::Parameter::KwArg { name, .. } => {
                param_names.insert(name.clone());
            }
        }
    }
    let param_layout = Box::new(ScopeLayout::new(param_names));

    let type_params = build_type_params(&def, ctx)?;

    let mut params = Vec::new();
    for param in &def.params {
        match param {
            min_ast::Parameter::Positional {
                name,
                default,
                annotation,
            } => {
                let _ = annotation;
                let default_value = if let Some(expr) = default {
                    Some(eval_expr(expr, ctx)?)
                } else {
                    None
                };
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::Positional,
                    default: default_value,
                });
            }
            min_ast::Parameter::VarArg { name, annotation } => {
                let _ = annotation;
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::VarArg,
                    default: None,
                });
            }
            min_ast::Parameter::KwOnly {
                name,
                default,
                annotation,
            } => {
                let _ = annotation;
                let default_value = if let Some(expr) = default {
                    Some(eval_expr(expr, ctx)?)
                } else {
                    None
                };
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::KwOnly,
                    default: default_value,
                });
            }
            min_ast::Parameter::KwArg { name, annotation } => {
                let _ = annotation;
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::KwArg,
                    default: None,
                });
            }
        }
    }

    let _ = &def.returns;

    let display_name = def.display_name.clone();
    let qualname = def.qualname.clone();
    let doc = if let Some(min_ast::StmtNode::Expr { value, .. }) = def.body.first() {
        if let min_ast::ExprNode::String { value, .. } = value {
            let doc = ffi::PyUnicode_FromStringAndSize(
                value.as_ptr() as *const c_char,
                value.len() as ffi::Py_ssize_t,
            );
            if doc.is_null() {
                return Err(());
            }
            doc
        } else {
            ffi::Py_INCREF(ffi::Py_None());
            ffi::Py_None()
        }
    } else {
        ffi::Py_INCREF(ffi::Py_None());
        ffi::Py_None()
    };

    let name_obj = ffi::PyUnicode_FromString(CString::new(display_name.as_str()).unwrap().as_ptr());
    if name_obj.is_null() {
        ffi::Py_DECREF(doc);
        return Err(());
    }
    let qualname_obj = ffi::PyUnicode_FromString(CString::new(qualname.as_str()).unwrap().as_ptr());
    if qualname_obj.is_null() {
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(doc);
        return Err(());
    }

    let module_obj = if module_name.is_null() {
        ffi::Py_INCREF(ffi::Py_None());
        ffi::Py_None()
    } else {
        ffi::Py_INCREF(module_name);
        module_name
    };

    if install_eval_frame_hook().is_err() {
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    let globals_scope = ctx.globals_scope;
    let globals_dict = ctx.globals_dict;
    let builtins = ctx.builtins;
    let function_codes = ctx.function_codes;
    ffi::Py_INCREF(globals_dict);
    ffi::Py_INCREF(builtins);
    if !function_codes.is_null() {
        ffi::Py_INCREF(function_codes);
    }
    let py = Python::assume_attached();

    let data = Box::new(FunctionData {
        def,
        params,
        param_layout,
        local_layout,
        cellvars,
        closure,
        type_params,
        globals_scope,
        globals_dict,
        builtins,
        function_codes,
        runtime_fns: ctx.runtime_fns.clone(),
    });

    let code = match function_code_object(&data) {
        Ok(code) => code,
        Err(()) => {
            ffi::Py_DECREF(name_obj);
            ffi::Py_DECREF(qualname_obj);
            ffi::Py_DECREF(doc);
            ffi::Py_DECREF(module_obj);
            return Err(());
        }
    };

    let data_ptr = Box::into_raw(data);
    if set_code_extra(
        code.bind(py).as_ptr(),
        SOAC_CODE_EXTRA_KIND_FUNCTION_DATA,
        data_ptr as *mut c_void,
        Some(free_function_data),
    )
    .is_err()
    {
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    let func = ffi::PyFunction_NewWithQualName(code.bind(py).as_ptr(), globals_dict, qualname_obj);
    if func.is_null() {
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }
    if ffi::PyFunction_Check(func) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"PyFunction_NewWithQualName did not return function\0".as_ptr() as *const c_char,
        );
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    let defaults = function_defaults_object(&(*data_ptr).params)?;
    if ffi::PyFunction_SetDefaults(func, defaults.bind(py).as_ptr()) != 0 {
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    let kwdefaults = function_kwdefaults_object(&(*data_ptr).params)?;
    if ffi::PyFunction_SetKwDefaults(func, kwdefaults.bind(py).as_ptr()) != 0 {
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    let closure = match function_closure_object(&*data_ptr, code.bind(py).as_ptr()) {
        Ok(closure) => closure,
        Err(()) => {
            ffi::Py_DECREF(func);
            ffi::Py_DECREF(name_obj);
            ffi::Py_DECREF(qualname_obj);
            ffi::Py_DECREF(doc);
            ffi::Py_DECREF(module_obj);
            return Err(());
        }
    };
    if ffi::PyFunction_SetClosure(func, closure.bind(py).as_ptr()) != 0 {
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    if ffi::PyObject_SetAttrString(func, b"__module__\0".as_ptr() as *const c_char, module_obj) != 0
    {
        ffi::PyErr_Clear();
    }
    if ffi::PyObject_SetAttrString(func, b"__doc__\0".as_ptr() as *const c_char, doc) != 0 {
        ffi::PyErr_Clear();
    }

    if function_has_annotations(&(*data_ptr).def) {
        let annotate = ffi::PyCFunction_NewEx(
            std::ptr::addr_of_mut!(SOAC_FUNCTION_ANNOTATE_PYFUNC_DEF),
            func,
            ptr::null_mut(),
        );
        if !annotate.is_null() {
            if ffi::PyObject_SetAttrString(
                func,
                b"__annotate__\0".as_ptr() as *const c_char,
                annotate,
            ) != 0
            {
                ffi::PyErr_Clear();
            }
            ffi::Py_DECREF(annotate);
        } else {
            ffi::PyErr_Clear();
        }
    }

    ffi::Py_DECREF(name_obj);
    ffi::Py_DECREF(qualname_obj);
    ffi::Py_DECREF(doc);
    ffi::Py_DECREF(module_obj);
    Ok(func)
}

pub(crate) fn function_has_annotations(def: &min_ast::FunctionDef) -> bool {
    if def.returns.is_some() {
        return true;
    }
    def.params.iter().any(|param| match param {
        min_ast::Parameter::Positional { annotation, .. }
        | min_ast::Parameter::VarArg { annotation, .. }
        | min_ast::Parameter::KwOnly { annotation, .. }
        | min_ast::Parameter::KwArg { annotation, .. } => annotation.is_some(),
    })
}

pub(crate) unsafe fn eval_function_annotations(
    data: &FunctionData,
    _format: i32,
) -> Result<Py<PyDict>, ()> {
    let py = Python::assume_attached();
    let annotations = PyDict::new(py);

    let ctx = ExecContext {
        globals_scope: data.globals_scope,
        globals_dict: data.globals_dict,
        params: ptr::null_mut(),
        locals: data.globals_scope,
        builtins: data.builtins,
        function_codes: data.function_codes,
        closure: data.closure.as_ref().map(|closure| &closure.scope),
        cellvars: Some(&data.cellvars),
        runtime_fns: &data.runtime_fns,
        type_params: data.type_params.as_ref().map(|state| &state.map),
    };

    for param in &data.def.params {
        let (name, annotation) = match param {
            min_ast::Parameter::Positional {
                name, annotation, ..
            }
            | min_ast::Parameter::VarArg { name, annotation }
            | min_ast::Parameter::KwOnly {
                name, annotation, ..
            }
            | min_ast::Parameter::KwArg { name, annotation } => (name, annotation),
        };
        if let Some(annotation) = annotation {
            let value = eval_expr(annotation, &ctx)?;
            let value_obj = Bound::<PyAny>::from_owned_ptr(py, value);
            if annotations.set_item(name.as_str(), value_obj).is_err() {
                return Err(());
            }
        }
    }

    if let Some(returns) = &data.def.returns {
        let value = eval_expr(returns, &ctx)?;
        let value_obj = Bound::<PyAny>::from_owned_ptr(py, value);
        if annotations.set_item("return", value_obj).is_err() {
            return Err(());
        }
    }

    Ok(annotations.unbind())
}

pub(crate) fn bind_args(
    params: &[ParamSpec],
    args: *mut ffi::PyObject,
    kwargs: *mut ffi::PyObject,
    param_scope: *mut ScopeInstance,
) -> Result<(), ()> {
    let py = unsafe { Python::assume_attached() };
    unsafe {
        let args_tuple = if args.is_null() {
            match Bound::<PyAny>::from_owned_ptr_or_opt(py, ffi::PyTuple_New(0)) {
                Some(tuple) => tuple,
                None => return Err(()),
            }
        } else {
            Bound::<PyAny>::from_borrowed_ptr(py, args)
        };
        let args_len = ffi::PyTuple_Size(args_tuple.as_ptr());
        if args_len < 0 {
            return Err(());
        }
        let mut kw_map = HashMap::new();
        if !kwargs.is_null() {
            let mut pos: ffi::Py_ssize_t = 0;
            let mut key: *mut ffi::PyObject = ptr::null_mut();
            let mut value: *mut ffi::PyObject = ptr::null_mut();
            while ffi::PyDict_Next(kwargs, &mut pos, &mut key, &mut value) != 0 {
                if ffi::PyUnicode_Check(key) == 0 {
                    return set_type_error("keywords must be strings");
                }
                let mut len: ffi::Py_ssize_t = 0;
                let ptr = ffi::PyUnicode_AsUTF8AndSize(key, &mut len);
                if ptr.is_null() {
                    return Err(());
                }
                let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    ptr as *const u8,
                    len as usize,
                ));
                kw_map.insert(name.to_string(), value);
            }
        }

        let mut arg_index: ffi::Py_ssize_t = 0;
        let mut has_vararg = false;

        for param in params {
            match param.kind {
                ParamKind::Positional => {
                    if arg_index < args_len {
                        if kw_map.contains_key(&param.name) {
                            return set_type_error("multiple values for argument");
                        }
                        let value = ffi::PyTuple_GetItem(args_tuple.as_ptr(), arg_index);
                        if value.is_null() {
                            return Err(());
                        }
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            return Err(());
                        }
                        arg_index += 1;
                    } else if let Some(value) = kw_map.remove(&param.name) {
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            return Err(());
                        }
                    } else if param.default.is_some() {
                        // Default applied later by apply_param_defaults.
                    } else {
                        return set_type_error("missing required positional argument");
                    }
                }
                ParamKind::VarArg => {
                    has_vararg = true;
                    let remaining = args_len - arg_index;
                    let tuple = match Bound::<PyAny>::from_owned_ptr_or_opt(
                        py,
                        ffi::PyTuple_New(remaining),
                    ) {
                        Some(tuple) => tuple,
                        None => return Err(()),
                    };
                    for idx in 0..remaining {
                        let value = ffi::PyTuple_GetItem(args_tuple.as_ptr(), arg_index + idx);
                        if value.is_null() {
                            return Err(());
                        }
                        ffi::Py_INCREF(value);
                        if ffi::PyTuple_SetItem(tuple.as_ptr(), idx, value) != 0 {
                            return Err(());
                        }
                    }
                    arg_index = args_len;
                    if scope_assign_name(&mut *param_scope, param.name.as_str(), tuple.as_ptr())
                        .is_err()
                    {
                        return Err(());
                    }
                }
                ParamKind::KwOnly => {
                    if let Some(value) = kw_map.remove(&param.name) {
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            return Err(());
                        }
                    } else if param.default.is_some() {
                        // Default applied later by apply_param_defaults.
                    } else {
                        return set_type_error("missing required keyword-only argument");
                    }
                }
                ParamKind::KwArg => {
                    let dict = PyDict::new(py);
                    for (key, value) in &kw_map {
                        let value_obj = Bound::<PyAny>::from_borrowed_ptr(py, *value);
                        if dict.set_item(key.as_str(), value_obj).is_err() {
                            return Err(());
                        }
                    }
                    kw_map.clear();
                    if scope_assign_name(&mut *param_scope, param.name.as_str(), dict.as_ptr())
                        .is_err()
                    {
                        return Err(());
                    }
                }
            }
        }

        if arg_index < args_len && !has_vararg {
            return set_type_error("too many positional arguments");
        }

        if !kw_map.is_empty() {
            return set_type_error("unexpected keyword argument");
        }

        Ok(())
    }
}

pub(crate) fn apply_param_defaults(
    params: &[ParamSpec],
    param_scope: *mut ScopeInstance,
) -> Result<(), ()> {
    unsafe {
        let scope = &mut *param_scope;
        for param in params {
            let Some(default) = param.default else {
                continue;
            };
            let Some(slot) = scope.layout().slot_for(param.name.as_str()) else {
                continue;
            };
            if scope.slots[slot].is_null() {
                if scope_assign_name(scope, param.name.as_str(), default).is_err() {
                    return Err(());
                }
            }
        }
    }
    Ok(())
}

fn collect_call_args(args: &[min_ast::Arg], ctx: &ExecContext<'_>) -> Result<(), ()> {
    let py = unsafe { Python::assume_attached() };
    let mut positional: Vec<Py<PyAny>> = Vec::new();
    let mut kw_map: HashMap<String, Py<PyAny>> = HashMap::new();

    for arg in args {
        match arg {
            min_ast::Arg::Positional(expr) => {
                let value = eval_expr(expr, ctx)?;
                positional.push(unsafe { Bound::<PyAny>::from_owned_ptr(py, value).unbind() });
            }
            min_ast::Arg::Starred(expr) => unsafe {
                let value = eval_expr(expr, ctx)?;
                let value_obj = Bound::<PyAny>::from_owned_ptr(py, value);
                let seq = ffi::PySequence_Fast(
                    value_obj.as_ptr(),
                    b"argument after * must be iterable\0".as_ptr() as *const c_char,
                );
                if seq.is_null() {
                    return Err(());
                }
                let seq_obj = Bound::<PyAny>::from_owned_ptr(py, seq);
                let seq_len = ffi::PySequence_Size(seq);
                if seq_len < 0 {
                    return Err(());
                }
                for idx in 0..seq_len {
                    let item = ffi::PySequence_GetItem(seq, idx);
                    if item.is_null() {
                        return Err(());
                    }
                    positional.push(Bound::<PyAny>::from_owned_ptr(py, item).unbind());
                }
                drop(seq_obj);
            },
            min_ast::Arg::Keyword { name, value } => unsafe {
                let val = eval_expr(value, ctx)?;
                let val_obj = Bound::<PyAny>::from_owned_ptr(py, val);
                if kw_map.contains_key(name) {
                    return set_type_error("multiple values for keyword argument");
                }
                kw_map.insert(name.clone(), val_obj.unbind());
            },
            min_ast::Arg::KwStarred(expr) => unsafe {
                let mapping = eval_expr(expr, ctx)?;
                let mapping_obj = Bound::<PyAny>::from_owned_ptr(py, mapping);
                let items = ffi::PyMapping_Items(mapping);
                if items.is_null() {
                    return Err(());
                }
                let items_obj = Bound::<PyAny>::from_owned_ptr(py, items);
                let items_len = ffi::PySequence_Size(items);
                if items_len < 0 {
                    return Err(());
                }
                for idx in 0..items_len {
                    let item = ffi::PySequence_GetItem(items, idx);
                    if item.is_null() {
                        return Err(());
                    }
                    let item_obj = Bound::<PyAny>::from_owned_ptr(py, item);
                    let key = ffi::PySequence_GetItem(item, 0);
                    let val = ffi::PySequence_GetItem(item, 1);
                    if key.is_null() || val.is_null() {
                        let _ = Bound::<PyAny>::from_owned_ptr_or_opt(py, key);
                        let _ = Bound::<PyAny>::from_owned_ptr_or_opt(py, val);
                        return Err(());
                    }
                    let key_obj = Bound::<PyAny>::from_owned_ptr(py, key);
                    let val_obj = Bound::<PyAny>::from_owned_ptr(py, val);
                    if ffi::PyUnicode_Check(key) == 0 {
                        return set_type_error("keywords must be strings");
                    }
                    let mut len: ffi::Py_ssize_t = 0;
                    let ptr = ffi::PyUnicode_AsUTF8AndSize(key, &mut len);
                    if ptr.is_null() {
                        return Err(());
                    }
                    let key_str = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        ptr as *const u8,
                        len as usize,
                    ));
                    if kw_map.contains_key(key_str) {
                        return set_type_error("multiple values for keyword argument");
                    }
                    kw_map.insert(key_str.to_string(), val_obj.unbind());
                    drop(key_obj);
                    drop(item_obj);
                }
                drop(items_obj);
                drop(mapping_obj);
            },
        }
    }

    drop(positional);
    drop(kw_map);
    Ok(())
}

fn eval_block(stmts: &[min_ast::StmtNode], ctx: &ExecContext<'_>) -> Result<StmtFlow, ()> {
    for stmt in stmts {
        match eval_stmt(stmt, ctx)? {
            StmtFlow::Normal => {}
            flow => return Ok(flow),
        }
    }
    Ok(StmtFlow::Normal)
}

unsafe fn scope_is_module_globals(ctx: &ExecContext<'_>) -> bool {
    ctx.locals == ctx.globals_scope
}

unsafe fn name_key(name: &str) -> Result<*mut ffi::PyObject, ()> {
    let key = ffi::PyUnicode_FromStringAndSize(name.as_ptr() as *const c_char, name.len() as _);
    if key.is_null() { Err(()) } else { Ok(key) }
}

unsafe fn dict_lookup_name(dict: *mut ffi::PyObject, name: &str) -> Result<*mut ffi::PyObject, ()> {
    let key = name_key(name)?;
    let value = ffi::PyObject_GetItem(dict, key);
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

unsafe fn assign_name_in_context(
    ctx: &ExecContext<'_>,
    name: &str,
    value: *mut ffi::PyObject,
) -> Result<(), ()> {
    if scope_is_module_globals(ctx) {
        let key = name_key(name)?;
        let set_result = ffi::PyObject_SetItem(ctx.globals_dict, key, value);
        ffi::Py_DECREF(key);
        if set_result != 0 {
            return Err(());
        }
        return Ok(());
    }
    if is_cellvar_name(ctx, name) {
        let locals = &mut *ctx.locals;
        let existing = scope_lookup_name(&*locals, name);
        if !existing.is_null() {
            if ffi::PyObject_TypeCheck(existing, std::ptr::addr_of_mut!(PyCell_Type)) != 0 {
                let status = ffi::PyObject_SetAttrString(
                    existing,
                    b"cell_contents\0".as_ptr() as *const c_char,
                    value,
                );
                ffi::Py_DECREF(existing);
                if status != 0 {
                    return Err(());
                }
                return Ok(());
            }
            ffi::Py_DECREF(existing);
        }
        let cell = PyCell_New(value);
        if cell.is_null() {
            return Err(());
        }
        let status = scope_assign_name(locals, name, cell);
        ffi::Py_DECREF(cell);
        return status;
    }
    scope_assign_name(&mut *ctx.locals, name, value)
}

unsafe fn delete_name_in_context(ctx: &ExecContext<'_>, name: &str) -> Result<(), ()> {
    if scope_is_module_globals(ctx) {
        let key = name_key(name)?;
        let del_result = ffi::PyObject_DelItem(ctx.globals_dict, key);
        ffi::Py_DECREF(key);
        if del_result != 0 {
            if ffi::PyErr_ExceptionMatches(ffi::PyExc_KeyError) != 0 {
                ffi::PyErr_Clear();
                return set_name_error(name);
            }
            return Err(());
        }
        return Ok(());
    }
    if is_cellvar_name(ctx, name) {
        let locals = &mut *ctx.locals;
        let existing = scope_lookup_name(&*locals, name);
        if existing.is_null() {
            return set_unbound_local(name);
        }
        if ffi::PyObject_TypeCheck(existing, std::ptr::addr_of_mut!(PyCell_Type)) != 0 {
            let status =
                ffi::PyObject_DelAttrString(existing, b"cell_contents\0".as_ptr() as *const c_char);
            ffi::Py_DECREF(existing);
            if status != 0 {
                if ffi::PyErr_ExceptionMatches(ffi::PyExc_ValueError) != 0
                    || ffi::PyErr_ExceptionMatches(ffi::PyExc_AttributeError) != 0
                {
                    ffi::PyErr_Clear();
                    return set_unbound_local(name);
                }
                return Err(());
            }
            return Ok(());
        }
        ffi::Py_DECREF(existing);
    }
    scope_delete_name(&mut *ctx.locals, name)
}

fn eval_stmt(stmt: &min_ast::StmtNode, ctx: &ExecContext<'_>) -> Result<StmtFlow, ()> {
    match stmt {
        min_ast::StmtNode::FunctionDef(func) => unsafe {
            let module_name = dict_lookup_name(ctx.globals_dict, "__name__")?;
            let function = build_function(func.clone(), ctx, module_name)?;
            if !module_name.is_null() {
                ffi::Py_DECREF(module_name);
            }
            if assign_name_in_context(ctx, func.name.as_str(), function).is_err() {
                ffi::Py_DECREF(function);
                return Err(());
            }
            ffi::Py_DECREF(function);
            Ok(StmtFlow::Normal)
        },
        min_ast::StmtNode::While {
            test, body, orelse, ..
        } => {
            loop {
                let condition = eval_expr(test, ctx)?;
                let truthy = unsafe { ffi::PyObject_IsTrue(condition) };
                unsafe {
                    ffi::Py_DECREF(condition);
                }
                if truthy < 0 {
                    return Err(());
                }
                if truthy == 0 {
                    break;
                }
                match eval_block(body, ctx)? {
                    StmtFlow::Normal => {}
                    StmtFlow::Continue => continue,
                    StmtFlow::Break => {
                        return Ok(StmtFlow::Normal);
                    }
                    StmtFlow::Return(value) => return Ok(StmtFlow::Return(value)),
                }
            }
            eval_block(orelse, ctx)
        }
        min_ast::StmtNode::If {
            test, body, orelse, ..
        } => {
            let condition = eval_expr(test, ctx)?;
            let truthy = unsafe { ffi::PyObject_IsTrue(condition) };
            unsafe {
                ffi::Py_DECREF(condition);
            }
            if truthy < 0 {
                return Err(());
            }
            if truthy == 0 {
                eval_block(orelse, ctx)
            } else {
                eval_block(body, ctx)
            }
        }
        min_ast::StmtNode::Try {
            body,
            handler,
            orelse,
            finalbody,
            ..
        } => {
            let mut had_exception = false;
            let mut flow = match eval_block(body, ctx) {
                Ok(flow) => flow,
                Err(()) => {
                    had_exception = true;
                    if let Some(handler) = handler {
                        unsafe {
                            let mut prev_type: *mut ffi::PyObject = ptr::null_mut();
                            let mut prev_value: *mut ffi::PyObject = ptr::null_mut();
                            let mut prev_tb: *mut ffi::PyObject = ptr::null_mut();
                            ffi::PyErr_GetExcInfo(&mut prev_type, &mut prev_value, &mut prev_tb);

                            let raised = ffi::PyErr_GetRaisedException();
                            let mut raised_type: *mut ffi::PyObject = ptr::null_mut();
                            let mut raised_tb: *mut ffi::PyObject = ptr::null_mut();
                            if !raised.is_null() {
                                raised_type = ffi::Py_TYPE(raised) as *mut ffi::PyObject;
                                ffi::Py_INCREF(raised_type);
                                raised_tb = ffi::PyException_GetTraceback(raised);
                            }
                            ffi::PyErr_SetExcInfo(raised_type, raised, raised_tb);
                            // Clear the error indicator before running the handler.
                            ffi::PyErr_Clear();

                            let handler_result = eval_block(handler, ctx);
                            ffi::PyErr_SetExcInfo(prev_type, prev_value, prev_tb);
                            match handler_result {
                                Ok(flow) => flow,
                                Err(()) => return Err(()),
                            }
                        }
                    } else {
                        return Err(());
                    }
                }
            };

            if !had_exception && matches!(flow, StmtFlow::Normal) {
                flow = eval_block(orelse, ctx)?;
            }

            let final_flow = eval_block(finalbody, ctx)?;
            if matches!(final_flow, StmtFlow::Normal) {
                Ok(flow)
            } else {
                Ok(final_flow)
            }
        }
        min_ast::StmtNode::Raise { exc, .. } => unsafe {
            if let Some(expr) = exc {
                let value = eval_expr(expr, ctx)?;
                let typ = if ffi::PyExceptionInstance_Check(value) != 0 {
                    ffi::Py_TYPE(value) as *mut ffi::PyObject
                } else {
                    value
                };
                ffi::PyErr_SetObject(typ, value);
                ffi::Py_DECREF(value);
                Err(())
            } else {
                if ffi::PyErr_Occurred().is_null() {
                    let mut typ: *mut ffi::PyObject = ptr::null_mut();
                    let mut val: *mut ffi::PyObject = ptr::null_mut();
                    let mut tb: *mut ffi::PyObject = ptr::null_mut();
                    ffi::PyErr_GetExcInfo(&mut typ, &mut val, &mut tb);
                    if !val.is_null() {
                        if typ.is_null() {
                            typ = ffi::Py_TYPE(val) as *mut ffi::PyObject;
                            ffi::Py_INCREF(typ);
                        }
                        if !tb.is_null() {
                            ffi::PyException_SetTraceback(val, tb);
                        }
                        ffi::PyErr_SetObject(typ, val);
                        ffi::Py_XDECREF(typ);
                        ffi::Py_XDECREF(val);
                        ffi::Py_XDECREF(tb);
                    } else {
                        ffi::PyErr_SetString(
                            ffi::PyExc_RuntimeError,
                            b"No active exception to reraise\0".as_ptr() as *const c_char,
                        );
                    }
                }
                Err(())
            }
        },
        min_ast::StmtNode::Break(_) => Ok(StmtFlow::Break),
        min_ast::StmtNode::Continue(_) => Ok(StmtFlow::Continue),
        min_ast::StmtNode::Return { value, .. } => {
            let result = if let Some(expr) = value {
                eval_expr(expr, ctx)?
            } else {
                unsafe {
                    ffi::Py_INCREF(ffi::Py_None());
                }
                unsafe { ffi::Py_None() }
            };
            Ok(StmtFlow::Return(result))
        }
        min_ast::StmtNode::Expr { value, .. } => {
            let result = eval_expr(value, ctx)?;
            unsafe {
                ffi::Py_DECREF(result);
            }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Assign { target, value, .. } => {
            let result = eval_expr(value, ctx)?;
            let status = unsafe {
                match target {
                    min_ast::AssignTarget::Name(name) => {
                        assign_name_in_context(ctx, name.as_str(), result)
                    }
                    min_ast::AssignTarget::Chained(targets) => {
                        for name in targets {
                            assign_name_in_context(ctx, name.as_str(), result)?;
                        }
                        Ok(())
                    }
                    min_ast::AssignTarget::Unpack(targets) => {
                        let seq = ffi::PySequence_Fast(
                            result,
                            b"cannot unpack non-iterable object\0".as_ptr() as *const c_char,
                        );
                        if seq.is_null() {
                            Err(())
                        } else {
                            let count = ffi::PySequence_Size(seq);
                            if count < 0 {
                                ffi::Py_DECREF(seq);
                                return Err(());
                            }
                            let count = count as usize;
                            if count != targets.len() {
                                ffi::Py_DECREF(seq);
                                ffi::PyErr_SetString(
                                    ffi::PyExc_ValueError,
                                    b"wrong number of values to unpack\0".as_ptr() as *const c_char,
                                );
                                Err(())
                            } else {
                                let mut ok = Ok(());
                                for (idx, name) in targets.iter().enumerate() {
                                    let item = ffi::PySequence_GetItem(seq, idx as ffi::Py_ssize_t);
                                    if item.is_null() {
                                        ok = Err(());
                                        break;
                                    }
                                    if let Err(()) =
                                        assign_name_in_context(ctx, name.as_str(), item)
                                    {
                                        ok = Err(());
                                    }
                                    ffi::Py_DECREF(item);
                                    if ok.is_err() {
                                        break;
                                    }
                                }
                                ffi::Py_DECREF(seq);
                                ok
                            }
                        }
                    }
                }
            };
            unsafe {
                ffi::Py_DECREF(result);
            }
            if status.is_err() {
                return Err(());
            }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Delete { target, .. } => {
            if unsafe { delete_name_in_context(ctx, target.as_str()) }.is_err() {
                return Err(());
            }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Pass(_) => Ok(StmtFlow::Normal),
    }
}

pub(crate) fn eval_expr(
    expr: &min_ast::ExprNode,
    ctx: &ExecContext<'_>,
) -> Result<*mut ffi::PyObject, ()> {
    match expr {
        min_ast::ExprNode::Name { id, .. } => lookup_name(id.as_str(), ctx),
        min_ast::ExprNode::Number { value, .. } => match value {
            min_ast::Number::Int(text) => unsafe {
                let cstr = CString::new(text.as_str()).unwrap();
                let result = ffi::PyLong_FromString(cstr.as_ptr(), ptr::null_mut(), 0);
                if result.is_null() {
                    Err(())
                } else {
                    Ok(result)
                }
            },
            min_ast::Number::Float(text) => unsafe {
                let py_str =
                    ffi::PyUnicode_FromString(CString::new(text.as_str()).unwrap().as_ptr());
                if py_str.is_null() {
                    return Err(());
                }
                let result = ffi::PyFloat_FromString(py_str);
                ffi::Py_DECREF(py_str);
                if result.is_null() {
                    Err(())
                } else {
                    Ok(result)
                }
            },
        },
        min_ast::ExprNode::String { value, .. } => unsafe {
            let bytes = value.as_bytes();
            let result =
                ffi::PyUnicode_FromStringAndSize(bytes.as_ptr() as *const c_char, bytes.len() as _);
            if result.is_null() {
                Err(())
            } else {
                Ok(result)
            }
        },
        min_ast::ExprNode::Bytes { value, .. } => unsafe {
            let result =
                ffi::PyBytes_FromStringAndSize(value.as_ptr() as *const c_char, value.len() as _);
            if result.is_null() {
                Err(())
            } else {
                Ok(result)
            }
        },
        min_ast::ExprNode::Tuple { elts, .. } => {
            let mut values = Vec::with_capacity(elts.len());
            for elt in elts {
                match eval_expr(elt, ctx) {
                    Ok(value) => values.push(value),
                    Err(()) => {
                        unsafe {
                            for value in values {
                                ffi::Py_DECREF(value);
                            }
                        }
                        return Err(());
                    }
                }
            }
            unsafe {
                let tuple = ffi::PyTuple_New(values.len() as _);
                if tuple.is_null() {
                    for value in values {
                        ffi::Py_DECREF(value);
                    }
                    return Err(());
                }
                for (idx, value) in values.into_iter().enumerate() {
                    if ffi::PyTuple_SetItem(tuple, idx as _, value) != 0 {
                        ffi::Py_DECREF(tuple);
                        return Err(());
                    }
                }
                Ok(tuple)
            }
        }
        min_ast::ExprNode::Await { .. } => set_not_implemented("await not supported"),
        min_ast::ExprNode::Call { func, args, .. } => eval_call(func, args, ctx),
    }
}

fn lookup_name(name: &str, ctx: &ExecContext<'_>) -> Result<*mut ffi::PyObject, ()> {
    unsafe {
        if let Some(type_params) = ctx.type_params {
            if let Some(value) = type_params.get(name) {
                ffi::Py_INCREF(*value);
                return Ok(*value);
            }
        }
        let locals = &*ctx.locals;
        if ctx.locals != ctx.globals_scope {
            if let Some(slot) = locals.layout().slot_for(name) {
                let value = scope_get_slot(locals, slot);
                if value.is_null() {
                    if !ctx.params.is_null() {
                        let param_value = scope_lookup_name(&*ctx.params, name);
                        if !param_value.is_null() {
                            return Ok(param_value);
                        }
                    }
                    return set_unbound_local(name);
                }
                if is_cellvar_name(ctx, name)
                    && ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) != 0
                {
                    let loaded = ffi::PyObject_GetAttrString(
                        value,
                        b"cell_contents\0".as_ptr() as *const c_char,
                    );
                    ffi::Py_DECREF(value);
                    if loaded.is_null() {
                        if ffi::PyErr_ExceptionMatches(ffi::PyExc_ValueError) != 0 {
                            ffi::PyErr_Clear();
                            return set_unbound_local(name);
                        }
                        return Err(());
                    }
                    return Ok(loaded);
                }
                return Ok(value);
            }
            let value = scope_get_dynamic(locals, name);
            if !value.is_null() {
                return Ok(value);
            }
            if !ctx.params.is_null() {
                let param_value = scope_lookup_name(&*ctx.params, name);
                if !param_value.is_null() {
                    return Ok(param_value);
                }
            }
        } else {
            let value = dict_lookup_name(ctx.globals_dict, name)?;
            if !value.is_null() {
                return Ok(value);
            }
        }

        if let Some(closure) = ctx.closure {
            let value = scope_lookup_name(closure, name);
            if !value.is_null() {
                if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) != 0
                    && !closure_name_requires_cell(name)
                {
                    let loaded = ffi::PyObject_GetAttrString(
                        value,
                        b"cell_contents\0".as_ptr() as *const c_char,
                    );
                    ffi::Py_DECREF(value);
                    if loaded.is_null() {
                        if ffi::PyErr_ExceptionMatches(ffi::PyExc_ValueError) != 0 {
                            ffi::PyErr_Clear();
                            return set_name_error(name);
                        }
                        return Err(());
                    }
                    return Ok(loaded);
                }
                return Ok(value);
            }
        }

        if ctx.locals != ctx.globals_scope {
            let value = dict_lookup_name(ctx.globals_dict, name)?;
            if !value.is_null() {
                return Ok(value);
            }
        }

        let value = dict_lookup_name(ctx.builtins, name)?;
        if !value.is_null() {
            return Ok(value);
        }
    }

    set_name_error(name)
}

unsafe fn locals_snapshot(ctx: &ExecContext<'_>) -> Result<Py<PyDict>, ()> {
    unsafe fn merge_scope_dict(
        target: *mut ffi::PyObject,
        source: *mut ffi::PyObject,
        allow_overwrite: bool,
    ) -> Result<(), ()> {
        let mut pos: ffi::Py_ssize_t = 0;
        let mut key: *mut ffi::PyObject = ptr::null_mut();
        let mut value: *mut ffi::PyObject = ptr::null_mut();
        while ffi::PyDict_Next(source, &mut pos, &mut key, &mut value) != 0 {
            let mut alias_key: *mut ffi::PyObject = ptr::null_mut();
            let mut is_dp_cell_alias = false;
            if ffi::PyUnicode_Check(key) != 0 {
                let mut len: ffi::Py_ssize_t = 0;
                let ptr = ffi::PyUnicode_AsUTF8AndSize(key, &mut len);
                if ptr.is_null() {
                    return Err(());
                }
                let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    ptr as *const u8,
                    len as usize,
                ));
                if let Some(stripped) = name.strip_prefix("_dp_cell_") {
                    is_dp_cell_alias = true;
                    alias_key = ffi::PyUnicode_FromStringAndSize(
                        stripped.as_ptr() as *const c_char,
                        stripped.len() as _,
                    );
                    if alias_key.is_null() {
                        return Err(());
                    }
                }
            }
            let mut value_for_dict = value;
            let mut value_needs_decref = false;
            if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) != 0 {
                let cell_contents = ffi::PyObject_GetAttrString(
                    value,
                    b"cell_contents\0".as_ptr() as *const c_char,
                );
                if !cell_contents.is_null() {
                    value_for_dict = cell_contents;
                    value_needs_decref = true;
                } else {
                    ffi::PyErr_Clear();
                    ffi::Py_XDECREF(alias_key);
                    continue;
                }
            } else if is_dp_cell_alias {
                // `_dp_cell_*` should only be surfaced as the stripped public
                // name when the runtime value is an actual cell. During BB
                // prologue execution these slots can transiently hold helper
                // wrapper values (e.g. BlockParam), which must not appear as
                // user-visible locals.
                ffi::Py_XDECREF(alias_key);
                continue;
            }
            let target_key = if !alias_key.is_null() { alias_key } else { key };
            if !allow_overwrite {
                let contains = ffi::PyDict_Contains(target, target_key);
                if contains < 0 {
                    ffi::Py_XDECREF(alias_key);
                    if value_needs_decref {
                        ffi::Py_DECREF(value_for_dict);
                    }
                    return Err(());
                }
                if contains != 0 {
                    ffi::Py_XDECREF(alias_key);
                    if value_needs_decref {
                        ffi::Py_DECREF(value_for_dict);
                    }
                    continue;
                }
            }
            if ffi::PyDict_SetItem(target, target_key, value_for_dict) != 0 {
                ffi::Py_XDECREF(alias_key);
                if value_needs_decref {
                    ffi::Py_DECREF(value_for_dict);
                }
                return Err(());
            }
            ffi::Py_XDECREF(alias_key);
            if value_needs_decref {
                ffi::Py_DECREF(value_for_dict);
            }
        }
        Ok(())
    }

    let py = Python::assume_attached();
    let dict = PyDict::new(py);
    if !ctx.params.is_null() {
        let params = Bound::<PyAny>::from_owned_ptr(py, scope_to_dict(&*ctx.params)?)
            .cast_into::<PyDict>()
            .map_err(|_| ())?;
        if merge_scope_dict(dict.as_ptr(), params.as_ptr(), true).is_err() {
            return Err(());
        }
    }
    let locals = Bound::<PyAny>::from_owned_ptr(py, scope_to_dict(&*ctx.locals)?)
        .cast_into::<PyDict>()
        .map_err(|_| ())?;
    if merge_scope_dict(dict.as_ptr(), locals.as_ptr(), true).is_err() {
        return Err(());
    }
    if let Some(closure) = ctx.closure {
        let closure_dict = Bound::<PyAny>::from_owned_ptr(py, scope_to_dict(closure)?)
            .cast_into::<PyDict>()
            .map_err(|_| ())?;
        if merge_scope_dict(dict.as_ptr(), closure_dict.as_ptr(), false).is_err() {
            return Err(());
        }
    }
    Ok(dict.unbind())
}

fn eval_call(
    func: &min_ast::ExprNode,
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<*mut ffi::PyObject, ()> {
    if let Some(result) = try_eval_dp_bb_helper_call(func, args, ctx)? {
        return Ok(result);
    }

    if let Some(type_param) = type_param_lookup_target(func, args, ctx) {
        let func_obj = eval_expr(func, ctx)?;
        collect_call_args(args, ctx)?;
        unsafe {
            ffi::Py_DECREF(func_obj);
            ffi::Py_INCREF(type_param);
        }
        return Ok(type_param);
    }

    let func_obj = eval_expr(func, ctx)?;

    unsafe {
        if (func_obj == ctx.runtime_fns.builtins_globals.as_ptr()
            || func_obj == ctx.runtime_fns.builtins_locals.as_ptr()
            || func_obj == ctx.runtime_fns.dp_globals.as_ptr()
            || func_obj == ctx.runtime_fns.dp_locals.as_ptr())
            && args.is_empty()
        {
            let result = if func_obj == ctx.runtime_fns.builtins_locals.as_ptr()
                || func_obj == ctx.runtime_fns.dp_locals.as_ptr()
            {
                if ctx.locals == ctx.globals_scope {
                    ffi::Py_INCREF(ctx.globals_dict);
                    ctx.globals_dict
                } else {
                    match locals_snapshot(ctx) {
                        Ok(dict) => dict.into_ptr(),
                        Err(()) => {
                            ffi::Py_DECREF(func_obj);
                            return Err(());
                        }
                    }
                }
            } else {
                ffi::Py_INCREF(ctx.globals_dict);
                ctx.globals_dict
            };
            ffi::Py_DECREF(func_obj);
            if result.is_null() {
                return Err(());
            }
            return Ok(result);
        }
    }

    let mut positional: Vec<*mut ffi::PyObject> = Vec::new();
    let kwargs = unsafe { ffi::PyDict_New() };
    if kwargs.is_null() {
        unsafe {
            ffi::Py_DECREF(func_obj);
        }
        return Err(());
    }

    for arg in args {
        match arg {
            min_ast::Arg::Positional(expr) => {
                let value = eval_expr(expr, ctx)?;
                positional.push(value);
            }
            min_ast::Arg::Starred(expr) => unsafe {
                let value = eval_expr(expr, ctx)?;
                let seq = ffi::PySequence_Fast(
                    value,
                    b"argument after * must be iterable\0".as_ptr() as *const c_char,
                );
                ffi::Py_DECREF(value);
                if seq.is_null() {
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                let seq_len = ffi::PySequence_Size(seq);
                if seq_len < 0 {
                    ffi::Py_DECREF(seq);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                for idx in 0..seq_len {
                    let item = ffi::PySequence_GetItem(seq, idx);
                    if item.is_null() {
                        ffi::Py_DECREF(seq);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    positional.push(item);
                }
                ffi::Py_DECREF(seq);
            },
            min_ast::Arg::Keyword { name, value } => unsafe {
                let val = eval_expr(value, ctx)?;
                let key = CString::new(name.as_str()).unwrap();
                let key_obj = ffi::PyUnicode_FromString(key.as_ptr());
                if key_obj.is_null() {
                    ffi::Py_DECREF(val);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                let contains = ffi::PyDict_Contains(kwargs, key_obj);
                if contains == 1 {
                    ffi::Py_DECREF(val);
                    ffi::Py_DECREF(key_obj);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return set_type_error("multiple values for keyword argument");
                } else if contains < 0 {
                    ffi::Py_DECREF(val);
                    ffi::Py_DECREF(key_obj);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                if ffi::PyDict_SetItem(kwargs, key_obj, val) != 0 {
                    ffi::Py_DECREF(val);
                    ffi::Py_DECREF(key_obj);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                ffi::Py_DECREF(val);
                ffi::Py_DECREF(key_obj);
            },
            min_ast::Arg::KwStarred(expr) => unsafe {
                let mapping = eval_expr(expr, ctx)?;
                let items = ffi::PyMapping_Items(mapping);
                ffi::Py_DECREF(mapping);
                if items.is_null() {
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                let items_len = ffi::PySequence_Size(items);
                if items_len < 0 {
                    ffi::Py_DECREF(items);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                for idx in 0..items_len {
                    let item = ffi::PySequence_GetItem(items, idx);
                    if item.is_null() {
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    let key = ffi::PySequence_GetItem(item, 0);
                    let val = ffi::PySequence_GetItem(item, 1);
                    ffi::Py_DECREF(item);
                    if key.is_null() || val.is_null() {
                        ffi::Py_XDECREF(key);
                        ffi::Py_XDECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    if ffi::PyUnicode_Check(key) == 0 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return set_type_error("keywords must be strings");
                    }
                    let contains = ffi::PyDict_Contains(kwargs, key);
                    if contains == 1 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return set_type_error("multiple values for keyword argument");
                    } else if contains < 0 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    if ffi::PyDict_SetItem(kwargs, key, val) != 0 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    ffi::Py_DECREF(key);
                    ffi::Py_DECREF(val);
                }
                ffi::Py_DECREF(items);
            },
        }
    }

    let call_kwargs = unsafe {
        if ffi::PyDict_Size(kwargs) == 0 {
            ffi::Py_DECREF(kwargs);
            ptr::null_mut()
        } else {
            kwargs
        }
    };

    let args_ptr = if positional.is_empty() {
        ptr::null()
    } else {
        positional.as_ptr()
    };

    let result = unsafe {
        ffi::PyObject_VectorcallDict(func_obj, args_ptr, positional.len() as _, call_kwargs)
    };
    unsafe {
        ffi::Py_DECREF(func_obj);
        if !call_kwargs.is_null() {
            ffi::Py_DECREF(call_kwargs);
        }
        for value in positional {
            ffi::Py_DECREF(value);
        }
    }

    if result.is_null() {
        return Err(());
    }
    unsafe {
        if !ffi::PyErr_Occurred().is_null() {
            ffi::Py_DECREF(result);
            return Err(());
        }
    }
    Ok(result)
}

fn try_dp_helper_name(func: &min_ast::ExprNode) -> Option<String> {
    dp_helper_lookup_name(func, "__dp__")
}

fn try_eval_dp_bb_helper_call(
    func: &min_ast::ExprNode,
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<Option<*mut ffi::PyObject>, ()> {
    let Some(helper) = try_dp_helper_name(func) else {
        return Ok(None);
    };
    match helper.as_str() {
        "run_bb" => eval_dp_run_bb(args, ctx),
        "jump" => eval_dp_jump(args, ctx),
        "brif" => eval_dp_brif(args, ctx),
        "ret" => eval_dp_ret(args, ctx),
        "raise_" => eval_dp_raise(args, ctx),
        "take_args" => eval_dp_take_args(args, ctx),
        "take_arg1" => eval_dp_take_arg1(args, ctx),
        _ => Ok(None),
    }
}

unsafe fn call_dp_term_helper(
    ctx: &ExecContext<'_>,
    helper_name: &[u8],
    positional: Vec<*mut ffi::PyObject>,
) -> Result<*mut ffi::PyObject, ()> {
    let dp_module = lookup_name("__dp__", ctx)?;
    let helper = ffi::PyObject_GetAttrString(dp_module, helper_name.as_ptr() as *const c_char);
    ffi::Py_DECREF(dp_module);
    if helper.is_null() {
        for value in positional {
            ffi::Py_DECREF(value);
        }
        return Err(());
    }

    let args_ptr = if positional.is_empty() {
        ptr::null()
    } else {
        positional.as_ptr()
    };
    let result =
        ffi::PyObject_VectorcallDict(helper, args_ptr, positional.len() as _, ptr::null_mut());
    ffi::Py_DECREF(helper);
    for value in positional {
        ffi::Py_DECREF(value);
    }
    if result.is_null() {
        return Err(());
    }
    Ok(result)
}

fn eval_dp_jump(
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<Option<*mut ffi::PyObject>, ()> {
    if args.len() != 2
        || !args
            .iter()
            .all(|arg| matches!(arg, min_ast::Arg::Positional(_)))
    {
        return Ok(None);
    }
    unsafe {
        let target = match &args[0] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        let jump_args = match &args[1] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        call_dp_term_helper(ctx, b"jump\0", vec![target, jump_args]).map(Some)
    }
}

fn eval_dp_brif(
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<Option<*mut ffi::PyObject>, ()> {
    if args.len() != 5
        || !args
            .iter()
            .all(|arg| matches!(arg, min_ast::Arg::Positional(_)))
    {
        return Ok(None);
    }
    unsafe {
        let cond = match &args[0] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        let then_target = match &args[1] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        let then_args = match &args[2] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        let else_target = match &args[3] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        let else_args = match &args[4] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        let truthy = ffi::PyObject_IsTrue(cond);
        ffi::Py_DECREF(cond);
        if truthy < 0 {
            ffi::Py_DECREF(then_target);
            ffi::Py_DECREF(then_args);
            ffi::Py_DECREF(else_target);
            ffi::Py_DECREF(else_args);
            return Err(());
        }
        if truthy != 0 {
            ffi::Py_DECREF(else_target);
            ffi::Py_DECREF(else_args);
            call_dp_term_helper(ctx, b"jump\0", vec![then_target, then_args]).map(Some)
        } else {
            ffi::Py_DECREF(then_target);
            ffi::Py_DECREF(then_args);
            call_dp_term_helper(ctx, b"jump\0", vec![else_target, else_args]).map(Some)
        }
    }
}

fn eval_dp_ret(
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<Option<*mut ffi::PyObject>, ()> {
    if args.len() > 2
        || !args
            .iter()
            .all(|arg| matches!(arg, min_ast::Arg::Positional(_)))
    {
        return Ok(None);
    }
    unsafe {
        let value = if let Some(min_ast::Arg::Positional(expr)) = args.first() {
            eval_expr(expr, ctx)?
        } else {
            ffi::Py_INCREF(ffi::Py_None());
            ffi::Py_None()
        };
        if let Some(min_ast::Arg::Positional(expr)) = args.get(1) {
            let state = eval_expr(expr, ctx)?;
            ffi::Py_DECREF(state);
        }
        call_dp_term_helper(ctx, b"ret\0", vec![value]).map(Some)
    }
}

fn eval_dp_raise(
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<Option<*mut ffi::PyObject>, ()> {
    if args.is_empty()
        || args.len() > 2
        || !args
            .iter()
            .all(|arg| matches!(arg, min_ast::Arg::Positional(_)))
    {
        return Ok(None);
    }
    unsafe {
        let exc = match &args[0] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        if let Some(min_ast::Arg::Positional(expr)) = args.get(1) {
            let state = eval_expr(expr, ctx)?;
            ffi::Py_DECREF(state);
        }
        call_dp_term_helper(ctx, b"raise_\0", vec![exc]).map(Some)
    }
}

fn eval_dp_take_args(
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<Option<*mut ffi::PyObject>, ()> {
    if args.len() != 1 || !matches!(args[0], min_ast::Arg::Positional(_)) {
        return Ok(None);
    }
    unsafe {
        let args_ptr = match &args[0] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        let index = ffi::PyLong_FromLong(0);
        if index.is_null() {
            ffi::Py_DECREF(args_ptr);
            return Err(());
        }
        let value = ffi::PyObject_GetItem(args_ptr, index);
        ffi::Py_DECREF(index);
        if value.is_null() {
            ffi::Py_DECREF(args_ptr);
            return Err(());
        }
        let set_index = ffi::PyLong_FromLong(0);
        if set_index.is_null() {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(args_ptr);
            return Err(());
        }
        let none = ffi::Py_None();
        ffi::Py_INCREF(none);
        let set_result = ffi::PyObject_SetItem(args_ptr, set_index, none);
        ffi::Py_DECREF(set_index);
        ffi::Py_DECREF(none);
        ffi::Py_DECREF(args_ptr);
        if set_result != 0 {
            ffi::Py_DECREF(value);
            return Err(());
        }
        Ok(Some(value))
    }
}

fn eval_dp_take_arg1(
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<Option<*mut ffi::PyObject>, ()> {
    let Some(value) = eval_dp_take_args(args, ctx)? else {
        return Ok(None);
    };
    unsafe {
        let len = ffi::PySequence_Size(value);
        if len < 0 {
            ffi::Py_DECREF(value);
            return Err(());
        }
        if len != 1 {
            ffi::Py_DECREF(value);
            let msg =
                CString::new(format!("too many values to unpack (expected 1, got {len})")).unwrap();
            ffi::PyErr_SetString(ffi::PyExc_ValueError, msg.as_ptr());
            return Err(());
        }
        let item = ffi::PySequence_GetItem(value, 0);
        ffi::Py_DECREF(value);
        if item.is_null() {
            return Err(());
        }
        Ok(Some(item))
    }
}

fn eval_dp_run_bb(
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<Option<*mut ffi::PyObject>, ()> {
    if args.len() != 2
        || !args
            .iter()
            .all(|arg| matches!(arg, min_ast::Arg::Positional(_)))
    {
        return Ok(None);
    }
    unsafe {
        let mut block = match &args[0] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };
        let mut block_args = match &args[1] {
            min_ast::Arg::Positional(expr) => eval_expr(expr, ctx)?,
            _ => unreachable!(),
        };

        loop {
            if ffi::PyCallable_Check(block) == 0 {
                ffi::Py_DECREF(block);
                ffi::Py_DECREF(block_args);
                let _ = set_runtime_error::<()>("invalid basic-block target");
                return Err(());
            }
            let args_ptr = ffi::PyList_New(1);
            if args_ptr.is_null() {
                ffi::Py_DECREF(block);
                ffi::Py_DECREF(block_args);
                return Err(());
            }
            ffi::Py_INCREF(block_args);
            if ffi::PyList_SetItem(args_ptr, 0, block_args) != 0 {
                ffi::Py_DECREF(block);
                ffi::Py_DECREF(block_args);
                ffi::Py_DECREF(args_ptr);
                return Err(());
            }
            let term = ffi::PyObject_CallFunctionObjArgs(
                block,
                args_ptr,
                ptr::null_mut::<ffi::PyObject>(),
            );
            ffi::Py_DECREF(args_ptr);
            ffi::Py_DECREF(block);
            ffi::Py_DECREF(block_args);
            if term.is_null() {
                return Err(());
            }
            if ffi::PyTuple_Check(term) == 0 || ffi::PyTuple_Size(term) <= 0 {
                ffi::Py_DECREF(term);
                let _ = set_runtime_error::<()>("invalid basic-block terminator");
                return Err(());
            }
            let tag = ffi::PyTuple_GetItem(term, 0);
            if tag.is_null() || ffi::PyUnicode_Check(tag) == 0 {
                ffi::Py_DECREF(term);
                let _ = set_runtime_error::<()>("invalid basic-block terminator");
                return Err(());
            }
            let is_jump =
                ffi::PyUnicode_CompareWithASCIIString(tag, b"jump\0".as_ptr() as *const c_char)
                    == 0;
            let is_ret =
                ffi::PyUnicode_CompareWithASCIIString(tag, b"ret\0".as_ptr() as *const c_char) == 0;
            let is_raise =
                ffi::PyUnicode_CompareWithASCIIString(tag, b"raise\0".as_ptr() as *const c_char)
                    == 0;
            let term_len = ffi::PyTuple_Size(term);
            if is_jump && term_len == 3 {
                let next_block = ffi::PyTuple_GetItem(term, 1);
                let next_args = ffi::PyTuple_GetItem(term, 2);
                ffi::Py_INCREF(next_block);
                ffi::Py_INCREF(next_args);
                ffi::Py_DECREF(term);
                block = next_block;
                block_args = next_args;
                continue;
            }
            if is_ret && (term_len == 2 || term_len == 3) {
                let value = ffi::PyTuple_GetItem(term, 1);
                ffi::Py_INCREF(value);
                ffi::Py_DECREF(term);
                return Ok(Some(value));
            }
            if is_raise && (term_len == 2 || term_len == 3) {
                let exc = ffi::PyTuple_GetItem(term, 1);
                if exc.is_null() {
                    ffi::Py_DECREF(term);
                    return Err(());
                }
                let typ = if ffi::PyExceptionInstance_Check(exc) != 0 {
                    ffi::Py_TYPE(exc) as *mut ffi::PyObject
                } else {
                    exc
                };
                ffi::PyErr_SetObject(typ, exc);
                ffi::Py_DECREF(term);
                return Err(());
            }
            ffi::Py_DECREF(term);
            let _ = set_runtime_error::<()>("invalid basic-block terminator");
            return Err(());
        }
    }
}

fn type_param_lookup_target(
    func: &min_ast::ExprNode,
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Option<*mut ffi::PyObject> {
    let type_params = ctx.type_params?;
    let attr = dp_helper_lookup_name(func, "__dp__")?;
    let name_arg_index = match attr.as_str() {
        "class_lookup_global" | "load_global" => 1,
        _ => return None,
    };
    let name = match args.get(name_arg_index) {
        Some(min_ast::Arg::Positional(min_ast::ExprNode::String { value, .. })) => value,
        _ => return None,
    };
    type_params.get(name).copied()
}

fn dp_helper_lookup_name(func: &min_ast::ExprNode, module_name: &str) -> Option<String> {
    let min_ast::ExprNode::Call {
        func: getter, args, ..
    } = func
    else {
        return None;
    };
    if !matches!(getter.as_ref(), min_ast::ExprNode::Name { id, .. } if id == "__dp_getattr") {
        return None;
    }
    if args.len() != 2 {
        return None;
    }
    let module = match &args[0] {
        min_ast::Arg::Positional(min_ast::ExprNode::Name { id, .. }) => id.as_str(),
        _ => return None,
    };
    if module != module_name {
        return None;
    }
    match &args[1] {
        min_ast::Arg::Positional(min_ast::ExprNode::String { value, .. }) => Some(value.clone()),
        min_ast::Arg::Positional(min_ast::ExprNode::Call {
            func: decoder,
            args,
            ..
        }) if matches!(
            decoder.as_ref(),
            min_ast::ExprNode::Name { id, .. }
                if matches!(
                    id.as_str(),
                    "__dp_decode_literal_bytes" | "__dp_decode_literal_source_bytes"
                )
        ) && args.len() == 1 =>
        {
            match &args[0] {
                min_ast::Arg::Positional(min_ast::ExprNode::Bytes { value, .. }) => {
                    String::from_utf8(value.clone()).ok()
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn cleanup_call_args(
    func: *mut ffi::PyObject,
    kwargs: *mut ffi::PyObject,
    positional: Vec<*mut ffi::PyObject>,
) {
    unsafe {
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(kwargs);
        for value in positional {
            ffi::Py_DECREF(value);
        }
    }
}

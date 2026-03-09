use crate::jit::{self, ClifPlan};
use log::info;
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyTuple;
use std::any::Any;
use std::collections::HashMap;
use std::ffi::{CString, c_char, c_void};
use std::panic::{self, AssertUnwindSafe};
use std::ptr;
use std::time::Instant;

unsafe extern "C" {
    fn PyFunction_SetVectorcall(func: *mut ffi::PyFunctionObject, vectorcall: ffi::vectorcallfunc);
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

fn set_runtime_error<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(ffi::PyExc_RuntimeError, CString::new(msg).unwrap().as_ptr());
    }
    Err(())
}

const CLIF_VECTORCALL_CAPSULE_NAME: &[u8] = b"soac.clif_vectorcall_data\0";
const CLIF_VECTORCALL_ATTR: &[u8] = b"__dp_clif_vectorcall_data\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BindingKind {
    Function,
    GeneratorResume,
    AsyncGeneratorResume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BindingParamKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarArgs,
    KeywordOnly,
    VarKeyword,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeneratorClosureInit {
    InheritedCapture,
    Parameter,
    DeletedSentinel,
    RuntimePcZero,
    RuntimeNone,
    Deferred,
}

#[derive(Debug)]
struct GeneratorClosureSlot {
    logical_name: String,
    storage_name: String,
    init: GeneratorClosureInit,
}

#[derive(Debug)]
struct GeneratorClosureLayout {
    slots: Vec<GeneratorClosureSlot>,
}

#[derive(Debug)]
struct BindingParam {
    name: String,
    kind: BindingParamKind,
    state_index: Option<usize>,
    default_value: *mut ffi::PyObject,
}

#[derive(Debug)]
struct BindingMetadata {
    kind: BindingKind,
    state_order: Vec<String>,
    state_index_by_name: HashMap<String, usize>,
    params: Vec<BindingParam>,
    positional_param_indices: Vec<usize>,
    param_lookup: HashMap<String, usize>,
    varargs_param: Option<usize>,
    varkw_param: Option<usize>,
    closure_state_values: Vec<*mut ffi::PyObject>,
    deleted_obj: *mut ffi::PyObject,
}

struct ClifFunctionData {
    plan: ClifPlan,
    module_name: String,
    qualname: String,
    true_obj: *mut ffi::PyObject,
    false_obj: *mut ffi::PyObject,
    binding: BindingMetadata,
    closure_layout: Option<GeneratorClosureLayout>,
    materialize_entry_obj: *mut ffi::PyObject,
    ambient_args_obj: *mut ffi::PyObject,
    compiled_handle: *mut c_void,
    compiled_vectorcall_handle: *mut c_void,
    compiled_vectorcall_entry: Option<jit::VectorcallEntryFn>,
}

fn set_type_error<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(ffi::PyExc_TypeError, CString::new(msg).unwrap().as_ptr());
    }
    Err(())
}

unsafe fn decref_if_non_null(obj: *mut ffi::PyObject) {
    if !obj.is_null() {
        ffi::Py_DECREF(obj);
    }
}

unsafe fn free_binding_metadata(binding: BindingMetadata) {
    for param in binding.params {
        decref_if_non_null(param.default_value);
    }
    for value in binding.closure_state_values {
        decref_if_non_null(value);
    }
    decref_if_non_null(binding.deleted_obj);
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
    unsafe { free_binding_metadata(data.binding) };
    if !data.materialize_entry_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.materialize_entry_obj) };
    }
    if !data.ambient_args_obj.is_null() {
        unsafe { ffi::Py_DECREF(data.ambient_args_obj) };
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

unsafe fn py_string(obj: *mut ffi::PyObject) -> Result<String, ()> {
    if ffi::PyUnicode_Check(obj) == 0 {
        return set_type_error("expected string metadata while registering CLIF vectorcall");
    }
    let mut len = 0;
    let ptr = ffi::PyUnicode_AsUTF8AndSize(obj, &mut len);
    if ptr.is_null() {
        return Err(());
    }
    let bytes = std::slice::from_raw_parts(ptr as *const u8, len as usize);
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

unsafe fn tuple_size(obj: *mut ffi::PyObject, context: &str) -> Result<usize, ()> {
    let size = ffi::PyTuple_Size(obj);
    if size < 0 {
        if ffi::PyErr_Occurred().is_null() {
            return set_type_error(context);
        }
        return Err(());
    }
    Ok(size as usize)
}

unsafe fn tuple_get_item(
    obj: *mut ffi::PyObject,
    index: usize,
    context: &str,
) -> Result<*mut ffi::PyObject, ()> {
    let item = ffi::PyTuple_GetItem(obj, index as ffi::Py_ssize_t);
    if item.is_null() {
        if ffi::PyErr_Occurred().is_null() {
            return set_type_error(context);
        }
        return Err(());
    }
    Ok(item)
}

fn parse_param_kind(raw_name: &str) -> (BindingParamKind, &str) {
    if let Some(name) = raw_name.strip_prefix("**") {
        return (BindingParamKind::VarKeyword, name);
    }
    if let Some(name) = raw_name.strip_prefix('*') {
        return (BindingParamKind::VarArgs, name);
    }
    if let Some(name) = raw_name.strip_prefix("kw:") {
        return (BindingParamKind::KeywordOnly, name);
    }
    if let Some(name) = raw_name.strip_prefix('/') {
        return (BindingParamKind::PositionalOnly, name);
    }
    (BindingParamKind::PositionalOrKeyword, raw_name)
}

unsafe fn parse_binding_metadata(
    state_order_obj: *mut ffi::PyObject,
    params_obj: *mut ffi::PyObject,
    closure_values_obj: *mut ffi::PyObject,
    deleted_obj: *mut ffi::PyObject,
    no_default_obj: *mut ffi::PyObject,
    bind_kind: i32,
) -> Result<BindingMetadata, ()> {
    let kind = match bind_kind {
        0 => BindingKind::Function,
        1 => BindingKind::GeneratorResume,
        2 => BindingKind::AsyncGeneratorResume,
        _ => {
            return set_type_error("invalid CLIF vectorcall bind kind");
        }
    };
    if !ffi::PyTuple_Check(state_order_obj).is_positive() {
        return set_type_error("CLIF vectorcall state_order must be a tuple");
    }
    if deleted_obj.is_null() {
        return set_type_error("CLIF vectorcall requires a deleted sentinel");
    }

    let state_len = tuple_size(
        state_order_obj,
        "failed to read CLIF vectorcall state_order",
    )?;
    let mut state_order = Vec::with_capacity(state_len);
    let mut state_index_by_name = HashMap::with_capacity(state_len);
    for index in 0..state_len {
        let name_obj = tuple_get_item(
            state_order_obj,
            index,
            "failed to read CLIF vectorcall state_order entry",
        )?;
        let name = py_string(name_obj)?;
        if state_index_by_name.insert(name.clone(), index).is_some() {
            return set_type_error("duplicate state_order entry in CLIF vectorcall metadata");
        }
        state_order.push(name);
    }

    ffi::Py_INCREF(deleted_obj);
    let mut closure_state_values = vec![ptr::null_mut(); state_len];
    if !closure_values_obj.is_null() {
        if ffi::PyDict_Check(closure_values_obj) == 0 {
            return set_type_error("CLIF vectorcall closure_values must be a dict");
        }
        for (index, name) in state_order.iter().enumerate() {
            let c_name = CString::new(name.as_str()).map_err(|_| ())?;
            let value =
                ffi::PyDict_GetItemString(closure_values_obj, c_name.as_ptr() as *const c_char);
            if !value.is_null() {
                ffi::Py_INCREF(value);
                closure_state_values[index] = value;
            }
        }
    }

    let mut params = Vec::new();
    let mut positional_param_indices = Vec::new();
    let mut param_lookup = HashMap::new();
    let mut varargs_param = None;
    let mut varkw_param = None;

    if matches!(kind, BindingKind::Function) {
        if params_obj.is_null() || ffi::PyTuple_Check(params_obj) == 0 {
            return set_type_error("CLIF function binding params must be a tuple");
        }
        let param_count = tuple_size(params_obj, "failed to read CLIF function binding params")?;
        params.reserve(param_count);
        for index in 0..param_count {
            let param_obj = tuple_get_item(
                params_obj,
                index,
                "failed to read CLIF function binding param entry",
            )?;
            if ffi::PyTuple_Check(param_obj) == 0 {
                return set_type_error("CLIF function binding param entry must be a tuple");
            }
            let entry_len = tuple_size(param_obj, "failed to read CLIF function binding param")?
                as ffi::Py_ssize_t;
            if entry_len < 2 {
                return set_type_error("invalid CLIF function binding param entry");
            }
            let raw_name_obj = tuple_get_item(
                param_obj,
                0,
                "failed to read CLIF function binding param name",
            )?;
            let raw_name = py_string(raw_name_obj)?;
            let (param_kind, name) = parse_param_kind(&raw_name);
            let state_index = state_index_by_name.get(name).copied();
            let mut default_value = ptr::null_mut();
            if entry_len >= 3 {
                let candidate = tuple_get_item(
                    param_obj,
                    2,
                    "failed to read CLIF function binding param default",
                )?;
                if candidate != no_default_obj {
                    ffi::Py_INCREF(candidate);
                    default_value = candidate;
                }
            }
            let name_string = name.to_string();
            if param_lookup
                .insert(name_string.clone(), params.len())
                .is_some()
            {
                decref_if_non_null(default_value);
                return set_type_error(
                    "duplicate parameter name in CLIF function binding metadata",
                );
            }
            match param_kind {
                BindingParamKind::PositionalOnly | BindingParamKind::PositionalOrKeyword => {
                    positional_param_indices.push(params.len());
                }
                BindingParamKind::VarArgs => {
                    varargs_param = Some(params.len());
                }
                BindingParamKind::VarKeyword => {
                    varkw_param = Some(params.len());
                }
                BindingParamKind::KeywordOnly => {}
            }
            params.push(BindingParam {
                name: name_string,
                kind: param_kind,
                state_index,
                default_value,
            });
        }
    }

    Ok(BindingMetadata {
        kind,
        state_order,
        state_index_by_name,
        params,
        positional_param_indices,
        param_lookup,
        varargs_param,
        varkw_param,
        closure_state_values,
        deleted_obj,
    })
}

unsafe fn parse_generator_closure_layout(
    closure_layout_obj: *mut ffi::PyObject,
) -> Result<Option<GeneratorClosureLayout>, ()> {
    if closure_layout_obj.is_null() {
        return Ok(None);
    }
    if ffi::PyTuple_Check(closure_layout_obj) == 0 {
        return set_type_error("CLIF vectorcall closure_layout must be a 3-tuple");
    }
    if tuple_size(
        closure_layout_obj,
        "failed to read CLIF vectorcall closure_layout",
    )? != 3
    {
        return set_type_error("CLIF vectorcall closure_layout must be a 3-tuple");
    }
    let mut slots = Vec::new();
    for section_index in 0..3 {
        let section = tuple_get_item(
            closure_layout_obj,
            section_index,
            "failed to read CLIF vectorcall closure_layout section",
        )?;
        if ffi::PyTuple_Check(section) == 0 {
            return set_type_error("CLIF vectorcall closure_layout sections must be tuples");
        }
        let slot_count = tuple_size(
            section,
            "failed to read CLIF vectorcall closure_layout section size",
        )?;
        for slot_index in 0..slot_count {
            let slot_obj = tuple_get_item(
                section,
                slot_index,
                "failed to read CLIF vectorcall closure_layout slot",
            )?;
            if ffi::PyTuple_Check(slot_obj) == 0 {
                return set_type_error("CLIF vectorcall closure_layout slots must be 3-tuples");
            }
            if tuple_size(
                slot_obj,
                "failed to read CLIF vectorcall closure_layout slot size",
            )? != 3
            {
                return set_type_error("CLIF vectorcall closure_layout slots must be 3-tuples");
            }
            let logical_name = py_string(tuple_get_item(
                slot_obj,
                0,
                "failed to read CLIF vectorcall closure logical name",
            )?)?;
            let storage_name = py_string(tuple_get_item(
                slot_obj,
                1,
                "failed to read CLIF vectorcall closure storage name",
            )?)?;
            let init_name = py_string(tuple_get_item(
                slot_obj,
                2,
                "failed to read CLIF vectorcall closure init kind",
            )?)?;
            let init = match init_name.as_str() {
                "InheritedCapture" => GeneratorClosureInit::InheritedCapture,
                "Parameter" => GeneratorClosureInit::Parameter,
                "DeletedSentinel" => GeneratorClosureInit::DeletedSentinel,
                "RuntimePcZero" => GeneratorClosureInit::RuntimePcZero,
                "RuntimeNone" => GeneratorClosureInit::RuntimeNone,
                "Deferred" => GeneratorClosureInit::Deferred,
                _ => {
                    return set_type_error("invalid generator closure init kind");
                }
            };
            slots.push(GeneratorClosureSlot {
                logical_name,
                storage_name,
                init,
            });
        }
    }
    Ok(Some(GeneratorClosureLayout { slots }))
}

unsafe fn build_ambient_args_tuple(
    plan: &ClifPlan,
    binding: &BindingMetadata,
) -> Result<*mut ffi::PyObject, ()> {
    let result = ffi::PyTuple_New(plan.ambient_param_names.len() as ffi::Py_ssize_t);
    if result.is_null() {
        return Err(());
    }
    for (index, name) in plan.ambient_param_names.iter().enumerate() {
        let Some(state_index) = binding.state_index_by_name.get(name).copied() else {
            ffi::Py_DECREF(result);
            let msg =
                format!("missing ambient closure state {name:?} while registering CLIF vectorcall");
            let _ = set_runtime_error::<*mut ffi::PyObject>(&msg);
            return Err(());
        };
        let value = binding.closure_state_values[state_index];
        if value.is_null() {
            ffi::Py_DECREF(result);
            let msg = format!(
                "ambient closure state {name:?} is unavailable while registering CLIF vectorcall"
            );
            let _ = set_runtime_error::<*mut ffi::PyObject>(&msg);
            return Err(());
        }
        ffi::Py_INCREF(value);
        if ffi::PyTuple_SetItem(result, index as ffi::Py_ssize_t, value) != 0 {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(result);
            return Err(());
        }
    }
    Ok(result)
}

unsafe fn make_clif_function_data(
    module_name: &str,
    qualname: &str,
    state_order_obj: *mut ffi::PyObject,
    params_obj: *mut ffi::PyObject,
    closure_values_obj: *mut ffi::PyObject,
    closure_layout_obj: *mut ffi::PyObject,
    deleted_obj: *mut ffi::PyObject,
    no_default_obj: *mut ffi::PyObject,
    bind_kind: i32,
    materialize_entry_obj: *mut ffi::PyObject,
) -> Result<*mut c_void, ()> {
    let plan = jit::lookup_clif_plan(module_name, qualname);
    let Some(plan) = plan else {
        let msg =
            format!("no specialized JIT plan found: module={module_name:?} qualname={qualname:?}");
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
    let binding = match parse_binding_metadata(
        state_order_obj,
        params_obj,
        closure_values_obj,
        deleted_obj,
        no_default_obj,
        bind_kind,
    ) {
        Ok(value) => value,
        Err(()) => {
            ffi::Py_DECREF(true_obj);
            ffi::Py_DECREF(false_obj);
            return Err(());
        }
    };
    let closure_layout = match parse_generator_closure_layout(closure_layout_obj) {
        Ok(value) => value,
        Err(()) => {
            ffi::Py_DECREF(true_obj);
            ffi::Py_DECREF(false_obj);
            unsafe { free_binding_metadata(binding) };
            return Err(());
        }
    };
    let ambient_args_obj = match build_ambient_args_tuple(&plan, &binding) {
        Ok(value) => value,
        Err(()) => {
            ffi::Py_DECREF(true_obj);
            ffi::Py_DECREF(false_obj);
            unsafe { free_binding_metadata(binding) };
            return Err(());
        }
    };

    let clif_data = Box::new(ClifFunctionData {
        plan,
        module_name: module_name.to_string(),
        qualname: qualname.to_string(),
        true_obj,
        false_obj,
        binding,
        closure_layout,
        materialize_entry_obj: {
            let ptr = materialize_entry_obj;
            if !ptr.is_null() {
                ffi::Py_INCREF(ptr);
            }
            ptr
        },
        ambient_args_obj,
        compiled_handle: ptr::null_mut(),
        compiled_vectorcall_handle: ptr::null_mut(),
        compiled_vectorcall_entry: None,
    });
    Ok(Box::into_raw(clif_data) as *mut c_void)
}

unsafe fn clif_vectorcall_data(
    function: *mut ffi::PyObject,
) -> Result<&'static mut ClifFunctionData, ()> {
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
    let capsule = ffi::PyDict_GetItemString(dict, CLIF_VECTORCALL_ATTR.as_ptr() as *const c_char);
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
            data.binding.deleted_obj as *mut c_void,
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
                        b"failed to compile CLIF vectorcall trampoline\0".as_ptr() as *const i8,
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

unsafe fn cleanup_state_values(state_values: &mut [*mut ffi::PyObject]) {
    for value in state_values.iter_mut() {
        if !value.is_null() {
            ffi::Py_DECREF(*value);
            *value = ptr::null_mut();
        }
    }
}

unsafe fn state_value_from_borrowed(
    state_values: &mut [*mut ffi::PyObject],
    state_index: usize,
    value: *mut ffi::PyObject,
) {
    ffi::Py_INCREF(value);
    state_values[state_index] = value;
}

unsafe fn state_value_from_owned(
    state_values: &mut [*mut ffi::PyObject],
    state_index: usize,
    value: *mut ffi::PyObject,
) {
    state_values[state_index] = value;
}

unsafe fn fill_state_tuple_from_values(
    binding: &BindingMetadata,
    mut state_values: Vec<*mut ffi::PyObject>,
) -> *mut ffi::PyObject {
    let result = ffi::PyTuple_New(binding.state_order.len() as ffi::Py_ssize_t);
    if result.is_null() {
        cleanup_state_values(&mut state_values);
        return ptr::null_mut();
    }
    for index in 0..binding.state_order.len() {
        let item = if !state_values[index].is_null() {
            let owned = state_values[index];
            state_values[index] = ptr::null_mut();
            owned
        } else if !binding.closure_state_values[index].is_null() {
            let borrowed = binding.closure_state_values[index];
            ffi::Py_INCREF(borrowed);
            borrowed
        } else {
            ffi::Py_INCREF(binding.deleted_obj);
            binding.deleted_obj
        };
        if ffi::PyTuple_SetItem(result, index as ffi::Py_ssize_t, item) != 0 {
            ffi::Py_DECREF(item);
            ffi::Py_DECREF(result);
            cleanup_state_values(&mut state_values);
            return ptr::null_mut();
        }
    }
    cleanup_state_values(&mut state_values);
    result
}

unsafe fn build_function_state_tuple(
    args: *const *mut ffi::PyObject,
    nargsf: usize,
    kwnames: *mut ffi::PyObject,
    binding: &BindingMetadata,
) -> *mut ffi::PyObject {
    let nargs = ffi::PyVectorcall_NARGS(nargsf) as usize;
    let nkw = if kwnames.is_null() {
        0
    } else {
        ffi::PyTuple_GET_SIZE(kwnames) as usize
    };
    let mut state_values = vec![ptr::null_mut(); binding.state_order.len()];
    let mut assigned = vec![false; binding.params.len()];

    let positional_capacity = binding.positional_param_indices.len();
    if binding.varargs_param.is_none() && nargs > positional_capacity {
        cleanup_state_values(&mut state_values);
        let msg = format!(
            "{}() takes {} positional argument{} but {} {} given",
            binding
                .state_order
                .first()
                .map(String::as_str)
                .unwrap_or("<function>"),
            positional_capacity,
            if positional_capacity == 1 { "" } else { "s" },
            nargs,
            if nargs == 1 { "was" } else { "were" }
        );
        return set_type_error::<*mut ffi::PyObject>(&msg)
            .err()
            .map_or(ptr::null_mut(), |_| ptr::null_mut());
    }

    let positional_bound = nargs.min(positional_capacity);
    for position in 0..positional_bound {
        let param_index = binding.positional_param_indices[position];
        let value = *args.add(position);
        if value.is_null() {
            cleanup_state_values(&mut state_values);
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"null vectorcall positional argument\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        if let Some(state_index) = binding.params[param_index].state_index {
            state_value_from_borrowed(&mut state_values, state_index, value);
        }
        assigned[param_index] = true;
    }

    if let Some(varargs_param) = binding.varargs_param {
        if let Some(state_index) = binding.params[varargs_param].state_index {
            let extras = nargs.saturating_sub(positional_capacity);
            let extra_tuple = ffi::PyTuple_New(extras as ffi::Py_ssize_t);
            if extra_tuple.is_null() {
                cleanup_state_values(&mut state_values);
                return ptr::null_mut();
            }
            for offset in 0..extras {
                let value = *args.add(positional_capacity + offset);
                if value.is_null() {
                    ffi::Py_DECREF(extra_tuple);
                    cleanup_state_values(&mut state_values);
                    ffi::PyErr_SetString(
                        ffi::PyExc_RuntimeError,
                        b"null vectorcall positional vararg\0".as_ptr() as *const i8,
                    );
                    return ptr::null_mut();
                }
                ffi::Py_INCREF(value);
                if ffi::PyTuple_SetItem(extra_tuple, offset as ffi::Py_ssize_t, value) != 0 {
                    ffi::Py_DECREF(value);
                    ffi::Py_DECREF(extra_tuple);
                    cleanup_state_values(&mut state_values);
                    return ptr::null_mut();
                }
            }
            state_value_from_owned(&mut state_values, state_index, extra_tuple);
        }
        assigned[varargs_param] = true;
    }

    let has_varkw = binding.varkw_param.is_some();
    let mut varkw_dict = ptr::null_mut();
    if let Some(varkw_param) = binding.varkw_param {
        if let Some(state_index) = binding.params[varkw_param].state_index {
            varkw_dict = ffi::PyDict_New();
            if varkw_dict.is_null() {
                cleanup_state_values(&mut state_values);
                return ptr::null_mut();
            }
            state_value_from_owned(&mut state_values, state_index, varkw_dict);
        }
        assigned[varkw_param] = true;
    }

    for kw_index in 0..nkw {
        let key = ffi::PyTuple_GetItem(kwnames, kw_index as ffi::Py_ssize_t);
        if key.is_null() {
            cleanup_state_values(&mut state_values);
            return ptr::null_mut();
        }
        let value = *args.add(nargs + kw_index);
        if value.is_null() {
            cleanup_state_values(&mut state_values);
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"null vectorcall keyword argument\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        let key_name = match py_string(key) {
            Ok(name) => name,
            Err(()) => {
                cleanup_state_values(&mut state_values);
                return ptr::null_mut();
            }
        };
        if let Some(&param_index) = binding.param_lookup.get(key_name.as_str()) {
            let param = &binding.params[param_index];
            match param.kind {
                BindingParamKind::PositionalOnly | BindingParamKind::VarArgs => {
                    if !has_varkw {
                        cleanup_state_values(&mut state_values);
                        let msg = format!(
                            "{}() got an unexpected keyword argument '{}'",
                            binding
                                .state_order
                                .first()
                                .map(String::as_str)
                                .unwrap_or("<function>"),
                            key_name
                        );
                        return set_type_error::<*mut ffi::PyObject>(&msg)
                            .err()
                            .map_or(ptr::null_mut(), |_| ptr::null_mut());
                    }
                    if !varkw_dict.is_null() {
                        if ffi::PyDict_SetItem(varkw_dict, key, value) != 0 {
                            cleanup_state_values(&mut state_values);
                            return ptr::null_mut();
                        }
                    }
                }
                BindingParamKind::PositionalOrKeyword | BindingParamKind::KeywordOnly => {
                    if assigned[param_index] {
                        cleanup_state_values(&mut state_values);
                        let msg = format!(
                            "{}() got multiple values for argument '{}'",
                            binding
                                .state_order
                                .first()
                                .map(String::as_str)
                                .unwrap_or("<function>"),
                            key_name
                        );
                        return set_type_error::<*mut ffi::PyObject>(&msg)
                            .err()
                            .map_or(ptr::null_mut(), |_| ptr::null_mut());
                    }
                    if param.kind == BindingParamKind::VarKeyword {
                        if ffi::PyDict_SetItem(varkw_dict, key, value) != 0 {
                            cleanup_state_values(&mut state_values);
                            return ptr::null_mut();
                        }
                    } else {
                        if let Some(state_index) = param.state_index {
                            state_value_from_borrowed(&mut state_values, state_index, value);
                        }
                        assigned[param_index] = true;
                    }
                }
                BindingParamKind::VarKeyword => {
                    if !varkw_dict.is_null() && ffi::PyDict_SetItem(varkw_dict, key, value) != 0 {
                        cleanup_state_values(&mut state_values);
                        return ptr::null_mut();
                    }
                }
            }
        } else if has_varkw {
            if !varkw_dict.is_null() && ffi::PyDict_SetItem(varkw_dict, key, value) != 0 {
                cleanup_state_values(&mut state_values);
                return ptr::null_mut();
            }
        } else {
            cleanup_state_values(&mut state_values);
            let msg = format!(
                "{}() got an unexpected keyword argument '{}'",
                binding
                    .state_order
                    .first()
                    .map(String::as_str)
                    .unwrap_or("<function>"),
                key_name
            );
            return set_type_error::<*mut ffi::PyObject>(&msg)
                .err()
                .map_or(ptr::null_mut(), |_| ptr::null_mut());
        }
    }

    for (param_index, param) in binding.params.iter().enumerate() {
        if assigned[param_index] {
            continue;
        }
        match param.kind {
            BindingParamKind::VarArgs | BindingParamKind::VarKeyword => {}
            _ if !param.default_value.is_null() => {
                if let Some(state_index) = param.state_index {
                    state_value_from_borrowed(&mut state_values, state_index, param.default_value);
                }
                assigned[param_index] = true;
            }
            _ => {
                cleanup_state_values(&mut state_values);
                let msg = format!(
                    "{}() missing required argument '{}'",
                    binding
                        .state_order
                        .first()
                        .map(String::as_str)
                        .unwrap_or("<function>"),
                    param.name
                );
                return set_type_error::<*mut ffi::PyObject>(&msg)
                    .err()
                    .map_or(ptr::null_mut(), |_| ptr::null_mut());
            }
        }
    }

    fill_state_tuple_from_values(binding, state_values)
}

unsafe fn build_resume_state_tuple(
    args: *const *mut ffi::PyObject,
    nargsf: usize,
    kwnames: *mut ffi::PyObject,
    binding: &BindingMetadata,
) -> *mut ffi::PyObject {
    let expected = match binding.kind {
        BindingKind::GeneratorResume => 3usize,
        BindingKind::AsyncGeneratorResume => 4usize,
        BindingKind::Function => unreachable!(),
    };
    let nargs = ffi::PyVectorcall_NARGS(nargsf) as usize;
    let nkw = if kwnames.is_null() {
        0usize
    } else {
        ffi::PyTuple_GET_SIZE(kwnames) as usize
    };
    if nkw != 0 {
        let kind = if matches!(binding.kind, BindingKind::AsyncGeneratorResume) {
            "async generator"
        } else {
            "generator"
        };
        return set_type_error::<*mut ffi::PyObject>(&format!(
            "hidden {kind} resume entry does not accept keyword arguments"
        ))
        .err()
        .map_or(ptr::null_mut(), |_| ptr::null_mut());
    }
    if nargs != expected {
        let kind = if matches!(binding.kind, BindingKind::AsyncGeneratorResume) {
            "async generator"
        } else {
            "generator"
        };
        return set_type_error::<*mut ffi::PyObject>(&format!(
            "hidden {kind} resume entry expected {expected} arguments, got {nargs}"
        ))
        .err()
        .map_or(ptr::null_mut(), |_| ptr::null_mut());
    }
    let gen_obj = *args.add(0);
    let send_value = *args.add(1);
    let resume_exc = *args.add(2);
    let transport_sent = if expected == 4 {
        *args.add(3)
    } else {
        ptr::null_mut()
    };
    if gen_obj.is_null()
        || send_value.is_null()
        || resume_exc.is_null()
        || (expected == 4 && transport_sent.is_null())
    {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"null vectorcall argument in generator resume entry\0".as_ptr() as *const i8,
        );
        return ptr::null_mut();
    }

    let mut state_values = vec![ptr::null_mut(); binding.state_order.len()];
    let frame_obj = ffi::PyObject_GetAttrString(gen_obj, b"gi_frame\0".as_ptr() as *const c_char);
    let frame_dict = if frame_obj.is_null() {
        if !ffi::PyErr_Occurred().is_null() {
            return ptr::null_mut();
        }
        ptr::null_mut()
    } else if ffi::PyDict_Check(frame_obj) != 0 {
        frame_obj
    } else {
        ffi::Py_DECREF(frame_obj);
        ptr::null_mut()
    };

    for (index, name) in binding.state_order.iter().enumerate() {
        match name.as_str() {
            "_dp_self" | "_dp_state" => {
                state_value_from_borrowed(&mut state_values, index, gen_obj);
            }
            "_dp_send_value" => {
                state_value_from_borrowed(&mut state_values, index, send_value);
            }
            "_dp_resume_exc" => {
                state_value_from_borrowed(&mut state_values, index, resume_exc);
            }
            "_dp_transport_sent" => {
                if expected == 4 {
                    state_value_from_borrowed(&mut state_values, index, transport_sent);
                }
            }
            _ => {
                if !frame_dict.is_null() {
                    let c_name = match CString::new(name.as_str()) {
                        Ok(value) => value,
                        Err(_) => {
                            ffi::Py_DECREF(frame_dict);
                            cleanup_state_values(&mut state_values);
                            return set_type_error::<*mut ffi::PyObject>(
                                "invalid generator frame local name",
                            )
                            .err()
                            .map_or(ptr::null_mut(), |_| ptr::null_mut());
                        }
                    };
                    let value = ffi::PyDict_GetItemString(frame_dict, c_name.as_ptr());
                    if !value.is_null() {
                        state_value_from_borrowed(&mut state_values, index, value);
                    }
                }
            }
        }
    }
    if !frame_dict.is_null() {
        ffi::Py_DECREF(frame_dict);
    }
    fill_state_tuple_from_values(binding, state_values)
}

unsafe fn state_tuple_item_by_name(
    state_tuple: *mut ffi::PyObject,
    binding: &BindingMetadata,
    name: &str,
) -> *mut ffi::PyObject {
    let Some(index) = binding.state_index_by_name.get(name).copied() else {
        return ptr::null_mut();
    };
    ffi::PyTuple_GET_ITEM(state_tuple, index as ffi::Py_ssize_t)
}

unsafe fn build_resume_closure_from_state_tuple(
    state_tuple: *mut ffi::PyObject,
    binding: &BindingMetadata,
    closure_layout: &GeneratorClosureLayout,
) -> *mut ffi::PyObject {
    if ffi::PyTuple_Check(state_tuple) == 0 {
        return set_type_error::<*mut ffi::PyObject>(
            "generator materialization expected a state tuple",
        )
        .err()
        .map_or(ptr::null_mut(), |_| ptr::null_mut());
    }
    let resume_closure = ffi::PyDict_New();
    if resume_closure.is_null() {
        return ptr::null_mut();
    }
    for slot in &closure_layout.slots {
        let mut decref_value = false;
        let value = match slot.init {
            GeneratorClosureInit::InheritedCapture => {
                let inherited =
                    state_tuple_item_by_name(state_tuple, binding, slot.storage_name.as_str());
                if !inherited.is_null() {
                    inherited
                } else {
                    let fallback =
                        state_tuple_item_by_name(state_tuple, binding, slot.logical_name.as_str());
                    if fallback.is_null() {
                        ffi::Py_DECREF(resume_closure);
                        return set_runtime_error::<*mut ffi::PyObject>(&format!(
                            "missing inherited generator closure state for {:?}",
                            slot.storage_name
                        ))
                        .err()
                        .map_or(ptr::null_mut(), |_| ptr::null_mut());
                    }
                    fallback
                }
            }
            GeneratorClosureInit::RuntimePcZero => {
                let zero = ffi::PyLong_FromLong(0);
                if zero.is_null() {
                    ffi::Py_DECREF(resume_closure);
                    return ptr::null_mut();
                }
                let cell = PyCell_New(zero);
                ffi::Py_DECREF(zero);
                if cell.is_null() {
                    ffi::Py_DECREF(resume_closure);
                    return ptr::null_mut();
                }
                decref_value = true;
                cell
            }
            GeneratorClosureInit::RuntimeNone => {
                let none = ffi::Py_None();
                ffi::Py_INCREF(none);
                let cell = PyCell_New(none);
                ffi::Py_DECREF(none);
                if cell.is_null() {
                    ffi::Py_DECREF(resume_closure);
                    return ptr::null_mut();
                }
                decref_value = true;
                cell
            }
            GeneratorClosureInit::Parameter
            | GeneratorClosureInit::DeletedSentinel
            | GeneratorClosureInit::Deferred => {
                let state_value =
                    state_tuple_item_by_name(state_tuple, binding, slot.logical_name.as_str());
                if state_value.is_null() {
                    ffi::Py_DECREF(resume_closure);
                    return set_runtime_error::<*mut ffi::PyObject>(&format!(
                        "missing generator state value for {:?} -> {:?}",
                        slot.logical_name, slot.storage_name
                    ))
                    .err()
                    .map_or(ptr::null_mut(), |_| ptr::null_mut());
                }
                let cell = PyCell_New(state_value);
                if cell.is_null() {
                    ffi::Py_DECREF(resume_closure);
                    return ptr::null_mut();
                }
                decref_value = true;
                cell
            }
        };
        let storage_name = match CString::new(slot.storage_name.as_str()) {
            Ok(value) => value,
            Err(_) => {
                if decref_value {
                    ffi::Py_DECREF(value);
                }
                ffi::Py_DECREF(resume_closure);
                return set_type_error::<*mut ffi::PyObject>(
                    "invalid generator closure storage name",
                )
                .err()
                .map_or(ptr::null_mut(), |_| ptr::null_mut());
            }
        };
        if ffi::PyDict_SetItemString(resume_closure, storage_name.as_ptr(), value) != 0 {
            if decref_value {
                ffi::Py_DECREF(value);
            }
            ffi::Py_DECREF(resume_closure);
            return ptr::null_mut();
        }
        if decref_value {
            ffi::Py_DECREF(value);
        }
    }
    resume_closure
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
        let data = &mut *(data_ptr as *mut ClifFunctionData);
        match data.binding.kind {
            BindingKind::Function => build_function_state_tuple(
                args as *const *mut ffi::PyObject,
                nargsf,
                kwnames as *mut ffi::PyObject,
                &data.binding,
            ) as *mut c_void,
            BindingKind::GeneratorResume | BindingKind::AsyncGeneratorResume => {
                build_resume_state_tuple(
                    args as *const *mut ffi::PyObject,
                    nargsf,
                    kwnames as *mut ffi::PyObject,
                    &data.binding,
                ) as *mut c_void
            }
        }
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
    data_ptr: *mut c_void,
) -> *mut c_void {
    if ffi::Py_EnterRecursiveCall(b" while calling a Python object\0".as_ptr() as *const i8) != 0 {
        return ptr::null_mut();
    }
    struct RecursiveCallGuard;
    impl Drop for RecursiveCallGuard {
        fn drop(&mut self) {
            unsafe { ffi::Py_LeaveRecursiveCall() };
        }
    }
    let _recursive_call_guard = RecursiveCallGuard;
    match panic::catch_unwind(AssertUnwindSafe(|| {
        if compiled_handle.is_null() || bb_args.is_null() || data_ptr.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid CLIF vectorcall compiled input\0".as_ptr() as *const i8,
            );
            return ptr::null_mut();
        }
        let data = &mut *(data_ptr as *mut ClifFunctionData);
        let hooks = jit::default_specialized_hooks();
        match jit::run_cranelift_run_bb_specialized_cached(
            compiled_handle,
            bb_args,
            data.ambient_args_obj as *mut c_void,
            &hooks,
        ) {
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
            let result = if let Some(closure_layout) = data.closure_layout.as_ref() {
                let resume_closure = build_resume_closure_from_state_tuple(
                    bb_args as *mut ffi::PyObject,
                    &data.binding,
                    closure_layout,
                );
                if resume_closure.is_null() {
                    ffi::Py_DECREF(bb_args as *mut ffi::PyObject);
                    return ptr::null_mut();
                }
                let result = ffi::PyObject_CallFunctionObjArgs(
                    data.materialize_entry_obj,
                    bb_args as *mut ffi::PyObject,
                    resume_closure,
                    ptr::null_mut::<ffi::PyObject>(),
                );
                ffi::Py_DECREF(resume_closure);
                result
            } else {
                ffi::PyObject_CallFunctionObjArgs(
                    data.materialize_entry_obj,
                    bb_args as *mut ffi::PyObject,
                    ptr::null_mut::<ffi::PyObject>(),
                )
            };
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
    state_order_obj: *mut ffi::PyObject,
    params_obj: *mut ffi::PyObject,
    closure_values_obj: *mut ffi::PyObject,
    closure_layout_obj: *mut ffi::PyObject,
    deleted_obj: *mut ffi::PyObject,
    no_default_obj: *mut ffi::PyObject,
    bind_kind: i32,
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
        state_order_obj,
        params_obj,
        closure_values_obj,
        closure_layout_obj,
        deleted_obj,
        no_default_obj,
        bind_kind,
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

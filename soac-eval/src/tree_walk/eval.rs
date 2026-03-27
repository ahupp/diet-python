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
enum BindingParamKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarArgs,
    KeywordOnly,
    VarKeyword,
}

#[derive(Debug)]
struct BindingParam {
    name: String,
    kind: BindingParamKind,
    has_default: bool,
}

#[derive(Debug)]
struct BindingMetadata {
    state_order: Vec<String>,
    params: Vec<BindingParam>,
    positional_param_indices: Vec<usize>,
    param_lookup: HashMap<String, usize>,
    varargs_param: Option<usize>,
    varkw_param: Option<usize>,
    deleted_obj: *mut ffi::PyObject,
}

struct ClifFunctionData {
    plan: ClifPlan,
    module_name: String,
    qualname: String,
    true_obj: *mut ffi::PyObject,
    false_obj: *mut ffi::PyObject,
    binding: BindingMetadata,
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

fn parse_param_kind(kind_name: &str) -> Result<BindingParamKind, ()> {
    match kind_name {
        "Any" => Ok(BindingParamKind::PositionalOrKeyword),
        "PosOnly" => Ok(BindingParamKind::PositionalOnly),
        "VarArg" => Ok(BindingParamKind::VarArgs),
        "KwOnly" => Ok(BindingParamKind::KeywordOnly),
        "KwArg" => Ok(BindingParamKind::VarKeyword),
        _ => set_type_error("invalid CLIF function binding param kind"),
    }
}

unsafe fn parse_binding_metadata(
    state_order_obj: *mut ffi::PyObject,
    params_obj: *mut ffi::PyObject,
    _param_defaults_obj: *mut ffi::PyObject,
    deleted_obj: *mut ffi::PyObject,
) -> Result<BindingMetadata, ()> {
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
    for index in 0..state_len {
        let name_obj = tuple_get_item(
            state_order_obj,
            index,
            "failed to read CLIF vectorcall state_order entry",
        )?;
        let name = py_string(name_obj)?;
        state_order.push(name);
    }

    ffi::Py_INCREF(deleted_obj);
    let mut params = Vec::new();
    let mut positional_param_indices = Vec::new();
    let mut param_lookup = HashMap::new();
    let mut varargs_param = None;
    let mut varkw_param = None;

    if params_obj.is_null() || ffi::PyTuple_Check(params_obj) == 0 {
        return set_type_error("CLIF binding params must be a tuple");
    }
    let param_count = tuple_size(params_obj, "failed to read CLIF binding params")?;
    let mut parsed_params = Vec::with_capacity(param_count);
    for index in 0..param_count {
        let param_obj =
            tuple_get_item(params_obj, index, "failed to read CLIF binding param entry")?;
        if ffi::PyTuple_Check(param_obj) == 0 {
            return set_type_error("CLIF binding param entry must be a tuple");
        }
        let entry_len =
            tuple_size(param_obj, "failed to read CLIF binding param")? as ffi::Py_ssize_t;
        if entry_len != 3 {
            return set_type_error("invalid CLIF binding param entry");
        }
        let name_obj = tuple_get_item(param_obj, 0, "failed to read CLIF binding param name")?;
        let name_string = py_string(name_obj)?;
        let kind_obj = tuple_get_item(param_obj, 1, "failed to read CLIF binding param kind")?;
        let kind_name = py_string(kind_obj)?;
        let param_kind = parse_param_kind(&kind_name)?;
        let has_default_obj = tuple_get_item(
            param_obj,
            2,
            "failed to read CLIF binding param default flag",
        )?;
        let has_default = ffi::PyObject_IsTrue(has_default_obj);
        if has_default < 0 {
            return Err(());
        }
        if param_lookup
            .insert(name_string.clone(), parsed_params.len())
            .is_some()
        {
            return set_type_error("duplicate parameter name in CLIF binding metadata");
        }
        match param_kind {
            BindingParamKind::PositionalOnly | BindingParamKind::PositionalOrKeyword => {
                positional_param_indices.push(parsed_params.len());
            }
            BindingParamKind::VarArgs => {
                varargs_param = Some(parsed_params.len());
            }
            BindingParamKind::VarKeyword => {
                varkw_param = Some(parsed_params.len());
            }
            BindingParamKind::KeywordOnly => {}
        }
        parsed_params.push((name_string, param_kind, has_default != 0));
    }

    for (name, kind, has_default) in parsed_params {
        params.push(BindingParam {
            name,
            kind,
            has_default,
        });
    }

    Ok(BindingMetadata {
        state_order,
        params,
        positional_param_indices,
        param_lookup,
        varargs_param,
        varkw_param,
        deleted_obj,
    })
}

unsafe fn make_clif_function_data(
    module_name: &str,
    function_id: usize,
    state_order_obj: *mut ffi::PyObject,
    params_obj: *mut ffi::PyObject,
    param_defaults_obj: *mut ffi::PyObject,
    deleted_obj: *mut ffi::PyObject,
) -> Result<*mut c_void, ()> {
    let plan = jit::lookup_clif_plan(module_name, function_id);
    let Some(plan) = plan else {
        let msg = format!(
            "no specialized JIT plan found: module={module_name:?} function_id={function_id:?}"
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
        .blocks
        .iter()
        .enumerate()
        .find(|(_, block)| matches!(block.fast_path, jit::BlockFastPath::None))
    {
        let label = plan
            .blocks
            .get(index)
            .map(|block| block.label.as_str())
            .unwrap_or("<unknown>");
        let msg = format!(
            "CLIF function requires full fast-path plan; unsupported block at index {index} label {label:?} for module={module_name:?} function_id={function_id:?}"
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
        param_defaults_obj,
        deleted_obj,
    ) {
        Ok(value) => value,
        Err(()) => {
            ffi::Py_DECREF(true_obj);
            ffi::Py_DECREF(false_obj);
            return Err(());
        }
    };

    let clif_data = Box::new(ClifFunctionData {
        plan,
        module_name: module_name.to_string(),
        qualname: format!("fn#{function_id}"),
        true_obj,
        false_obj,
        binding,
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
    if data.compiled_handle.is_null() {
        let globals_obj = ffi::PyFunction_GetGlobals(callable);
        if globals_obj.is_null() {
            return Err(());
        }
        let empty_tuple_obj = PyTuple::empty(py);
        let compile_start = Instant::now();
        let block_ptrs = vec![ptr::null_mut::<c_void>(); data.plan.blocks.len()];
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
            data.plan.blocks.len(),
        );
    }
    if data.compiled_vectorcall_handle.is_null() {
        let (handle, entry) = match jit::compile_cranelift_vectorcall_direct_trampoline(
            bind_direct_args_from_vectorcall,
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
                        b"failed to compile direct CLIF vectorcall trampoline\0".as_ptr()
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

unsafe fn cleanup_state_values(state_values: &mut [*mut ffi::PyObject]) {
    for value in state_values.iter_mut() {
        if !value.is_null() {
            ffi::Py_DECREF(*value);
            *value = ptr::null_mut();
        }
    }
}

unsafe fn bound_arg_value_from_borrowed(
    bound_args: &mut [*mut ffi::PyObject],
    param_index: usize,
    value: *mut ffi::PyObject,
) {
    ffi::Py_INCREF(value);
    bound_args[param_index] = value;
}

unsafe fn bound_arg_value_from_owned(
    bound_args: &mut [*mut ffi::PyObject],
    param_index: usize,
    value: *mut ffi::PyObject,
) {
    bound_args[param_index] = value;
}

unsafe fn build_function_bound_args(
    callable: *mut ffi::PyObject,
    args: *const *mut ffi::PyObject,
    nargsf: usize,
    kwnames: *mut ffi::PyObject,
    binding: &BindingMetadata,
) -> Result<Vec<*mut ffi::PyObject>, ()> {
    if callable.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"null callable in CLIF function binding\0".as_ptr() as *const i8,
        );
        return Err(());
    }
    let nargs = ffi::PyVectorcall_NARGS(nargsf) as usize;
    let nkw = if kwnames.is_null() {
        0
    } else {
        ffi::PyTuple_GET_SIZE(kwnames) as usize
    };
    let mut bound_args = vec![ptr::null_mut(); binding.params.len()];
    let mut assigned = vec![false; binding.params.len()];
    let positional_capacity = binding.positional_param_indices.len();

    if binding.varargs_param.is_none() && nargs > positional_capacity {
        cleanup_state_values(&mut bound_args);
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
        let _ = set_type_error::<()>(&msg);
        return Err(());
    }

    let positional_bound = nargs.min(positional_capacity);
    for position in 0..positional_bound {
        let param_index = binding.positional_param_indices[position];
        let value = *args.add(position);
        if value.is_null() {
            cleanup_state_values(&mut bound_args);
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"null vectorcall positional argument\0".as_ptr() as *const i8,
            );
            return Err(());
        }
        bound_arg_value_from_borrowed(&mut bound_args, param_index, value);
        assigned[param_index] = true;
    }

    if let Some(varargs_param) = binding.varargs_param {
        let extras = nargs.saturating_sub(positional_capacity);
        let extra_tuple = ffi::PyTuple_New(extras as ffi::Py_ssize_t);
        if extra_tuple.is_null() {
            cleanup_state_values(&mut bound_args);
            return Err(());
        }
        for offset in 0..extras {
            let value = *args.add(positional_capacity + offset);
            if value.is_null() {
                ffi::Py_DECREF(extra_tuple);
                cleanup_state_values(&mut bound_args);
                ffi::PyErr_SetString(
                    ffi::PyExc_RuntimeError,
                    b"null vectorcall positional vararg\0".as_ptr() as *const i8,
                );
                return Err(());
            }
            ffi::Py_INCREF(value);
            if ffi::PyTuple_SetItem(extra_tuple, offset as ffi::Py_ssize_t, value) != 0 {
                ffi::Py_DECREF(value);
                ffi::Py_DECREF(extra_tuple);
                cleanup_state_values(&mut bound_args);
                return Err(());
            }
        }
        bound_arg_value_from_owned(&mut bound_args, varargs_param, extra_tuple);
        assigned[varargs_param] = true;
    }

    let has_varkw = binding.varkw_param.is_some();
    let mut varkw_dict = ptr::null_mut();
    if let Some(varkw_param) = binding.varkw_param {
        varkw_dict = ffi::PyDict_New();
        if varkw_dict.is_null() {
            cleanup_state_values(&mut bound_args);
            return Err(());
        }
        bound_arg_value_from_owned(&mut bound_args, varkw_param, varkw_dict);
        assigned[varkw_param] = true;
    }

    for kw_index in 0..nkw {
        let key = ffi::PyTuple_GetItem(kwnames, kw_index as ffi::Py_ssize_t);
        if key.is_null() {
            cleanup_state_values(&mut bound_args);
            return Err(());
        }
        let value = *args.add(nargs + kw_index);
        if value.is_null() {
            cleanup_state_values(&mut bound_args);
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"null vectorcall keyword argument\0".as_ptr() as *const i8,
            );
            return Err(());
        }
        let key_name = match py_string(key) {
            Ok(name) => name,
            Err(()) => {
                cleanup_state_values(&mut bound_args);
                return Err(());
            }
        };
        if let Some(&param_index) = binding.param_lookup.get(key_name.as_str()) {
            let param = &binding.params[param_index];
            match param.kind {
                BindingParamKind::PositionalOnly | BindingParamKind::VarArgs => {
                    if !has_varkw {
                        cleanup_state_values(&mut bound_args);
                        let msg = format!(
                            "{}() got an unexpected keyword argument '{}'",
                            binding
                                .state_order
                                .first()
                                .map(String::as_str)
                                .unwrap_or("<function>"),
                            key_name
                        );
                        let _ = set_type_error::<()>(&msg);
                        return Err(());
                    }
                    if !varkw_dict.is_null() && ffi::PyDict_SetItem(varkw_dict, key, value) != 0 {
                        cleanup_state_values(&mut bound_args);
                        return Err(());
                    }
                }
                BindingParamKind::PositionalOrKeyword | BindingParamKind::KeywordOnly => {
                    if assigned[param_index] {
                        cleanup_state_values(&mut bound_args);
                        let msg = format!(
                            "{}() got multiple values for argument '{}'",
                            binding
                                .state_order
                                .first()
                                .map(String::as_str)
                                .unwrap_or("<function>"),
                            key_name
                        );
                        let _ = set_type_error::<()>(&msg);
                        return Err(());
                    }
                    if param.kind == BindingParamKind::VarKeyword {
                        if ffi::PyDict_SetItem(varkw_dict, key, value) != 0 {
                            cleanup_state_values(&mut bound_args);
                            return Err(());
                        }
                    } else {
                        bound_arg_value_from_borrowed(&mut bound_args, param_index, value);
                        assigned[param_index] = true;
                    }
                }
                BindingParamKind::VarKeyword => {
                    if !varkw_dict.is_null() && ffi::PyDict_SetItem(varkw_dict, key, value) != 0 {
                        cleanup_state_values(&mut bound_args);
                        return Err(());
                    }
                }
            }
        } else if has_varkw {
            if !varkw_dict.is_null() && ffi::PyDict_SetItem(varkw_dict, key, value) != 0 {
                cleanup_state_values(&mut bound_args);
                return Err(());
            }
        } else {
            cleanup_state_values(&mut bound_args);
            let msg = format!(
                "{}() got an unexpected keyword argument '{}'",
                binding
                    .state_order
                    .first()
                    .map(String::as_str)
                    .unwrap_or("<function>"),
                key_name
            );
            let _ = set_type_error::<()>(&msg);
            return Err(());
        }
    }

    for (param_index, param) in binding.params.iter().enumerate() {
        if assigned[param_index] {
            continue;
        }
        match param.kind {
            BindingParamKind::VarArgs | BindingParamKind::VarKeyword => {}
            _ => {
                if param.has_default {
                    assigned[param_index] = true;
                    continue;
                }
                cleanup_state_values(&mut bound_args);
                let msg = format!(
                    "{}() missing required argument '{}'",
                    binding
                        .state_order
                        .first()
                        .map(String::as_str)
                        .unwrap_or("<function>"),
                    param.name
                );
                let _ = set_type_error::<()>(&msg);
                return Err(());
            }
        }
    }
    Ok(bound_args)
}

unsafe fn write_owned_bound_args_to_buffer(
    mut bound_args: Vec<*mut ffi::PyObject>,
    out_args: *mut *mut ffi::PyObject,
    out_len: usize,
) -> Result<(), ()> {
    if bound_args.len() != out_len {
        cleanup_state_values(&mut bound_args);
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"bound CLIF argument count did not match direct entry arity\0".as_ptr() as *const i8,
        );
        return Err(());
    }
    if out_len == 0 {
        cleanup_state_values(&mut bound_args);
        return Ok(());
    }
    if out_args.is_null() {
        cleanup_state_values(&mut bound_args);
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"missing output buffer for direct CLIF function arguments\0".as_ptr() as *const i8,
        );
        return Err(());
    }
    for (index, value) in bound_args.iter_mut().enumerate() {
        let owned = *value;
        *out_args.add(index) = owned;
        *value = ptr::null_mut();
    }
    cleanup_state_values(&mut bound_args);
    Ok(())
}

unsafe extern "C" fn bind_direct_args_from_vectorcall(
    callable: *mut c_void,
    args: *const *mut c_void,
    nargsf: usize,
    kwnames: *mut c_void,
    data_ptr: *mut c_void,
    out_args: *mut *mut c_void,
    out_len: i64,
) -> i32 {
    match panic::catch_unwind(AssertUnwindSafe(|| {
        if callable.is_null() || data_ptr.is_null() || out_len < 0 {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid direct vectorcall bind input\0".as_ptr() as *const i8,
            );
            return 0;
        }
        let data = &mut *(data_ptr as *mut ClifFunctionData);
        let bound_args = match build_function_bound_args(
            callable as *mut ffi::PyObject,
            args as *const *mut ffi::PyObject,
            nargsf,
            kwnames as *mut ffi::PyObject,
            &data.binding,
        ) {
            Ok(value) => value,
            Err(()) => return 0,
        };
        match write_owned_bound_args_to_buffer(
            bound_args,
            out_args as *mut *mut ffi::PyObject,
            out_len as usize,
        ) {
            Ok(()) => 1,
            Err(()) => 0,
        }
    })) {
        Ok(value) => value,
        Err(payload) => {
            let message = format!(
                "panic in bind_direct_args_from_vectorcall: {}",
                panic_payload_to_string(payload)
            );
            if let Ok(c_msg) = CString::new(message) {
                ffi::PyErr_SetString(ffi::PyExc_RuntimeError, c_msg.as_ptr());
            } else {
                ffi::PyErr_SetString(
                    ffi::PyExc_RuntimeError,
                    b"panic in bind_direct_args_from_vectorcall\0".as_ptr() as *const i8,
                );
            }
            0
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
        if ffi::Py_EnterRecursiveCall(b" while calling a Python object\0".as_ptr() as *const i8)
            != 0
        {
            return ptr::null_mut();
        }
        struct RecursiveCallGuard;
        impl Drop for RecursiveCallGuard {
            fn drop(&mut self) {
                unsafe { ffi::Py_LeaveRecursiveCall() };
            }
        }
        let _recursive_call_guard = RecursiveCallGuard;
        let hooks = jit::default_specialized_hooks();
        unsafe { jit::install_specialized_hooks(&hooks) };
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
    function_id: usize,
    state_order_obj: *mut ffi::PyObject,
    params_obj: *mut ffi::PyObject,
    param_defaults_obj: *mut ffi::PyObject,
    deleted_obj: *mut ffi::PyObject,
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
        function_id,
        state_order_obj,
        params_obj,
        param_defaults_obj,
        deleted_obj,
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
    ensure_clif_vectorcall_compiled(py, function, data)
}

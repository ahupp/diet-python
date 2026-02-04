use dp_transform::{Options, min_ast, transform_str_to_ruff_with_options};
use pyo3::exceptions::{PyRuntimeError, PySyntaxError};
use pyo3::ffi;
use pyo3::prelude::*;
use soac_eval::tree_walk::{self as interpreter, RuntimeFns};
use std::any::Any;
use std::collections::HashSet;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;

#[derive(Debug)]
pub(crate) enum TransformToMinAstError {
    Parse(String),
    Lowering(String),
    MinAstConversion(String),
}

pub(crate) struct EvalLoweringResult {
    pub(crate) min_ast_module: min_ast::Module,
    pub(crate) transformed_source: String,
}

impl TransformToMinAstError {
    pub(crate) fn to_py_err(self) -> PyErr {
        match self {
            TransformToMinAstError::Parse(msg) => PySyntaxError::new_err(msg),
            TransformToMinAstError::Lowering(msg) => {
                PyRuntimeError::new_err(format!("AST lowering failed: {msg}"))
            }
            TransformToMinAstError::MinAstConversion(msg) => {
                PyRuntimeError::new_err(format!("min_ast conversion failed: {msg}"))
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

fn parse_and_lower(source: &str) -> Result<dp_transform::LoweringResult, TransformToMinAstError> {
    let options = Options {
        inject_import: true,
        eval_mode: true,
        lower_attributes: true,
        truthy: false,
        cleanup_dp_globals: false,
        force_import_rewrite: true,
        ..Options::default()
    };

    match std::panic::catch_unwind(|| transform_str_to_ruff_with_options(source, options)) {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(TransformToMinAstError::Parse(err.to_string())),
        Err(payload) => Err(TransformToMinAstError::Lowering(panic_payload_to_string(
            payload,
        ))),
    }
}

pub(crate) fn transform_to_min_ast(
    source: &str,
) -> Result<EvalLoweringResult, TransformToMinAstError> {
    let lowered = parse_and_lower(source)?;
    let transformed_source = lowered.to_string();
    match std::panic::catch_unwind(|| lowered.into_min_ast()) {
        Ok(module) => Ok(EvalLoweringResult {
            min_ast_module: module,
            transformed_source,
        }),
        Err(payload) => Err(TransformToMinAstError::MinAstConversion(
            panic_payload_to_string(payload),
        )),
    }
}

unsafe fn set_module_dict_item(
    module_obj: *mut ffi::PyObject,
    name: &str,
    value: *mut ffi::PyObject,
) -> Result<(), ()> {
    let dict = ffi::PyModule_GetDict(module_obj);
    if dict.is_null() {
        return Err(());
    }
    if ffi::PyDict_SetItemString(dict, CString::new(name).unwrap().as_ptr(), value) != 0 {
        return Err(());
    }
    Ok(())
}

unsafe fn build_module_spec(
    py: Python<'_>,
    name: &str,
    path: &str,
    is_package: bool,
) -> PyResult<*mut ffi::PyObject> {
    let importlib_util = ffi::PyImport_ImportModule(b"importlib.util\0".as_ptr() as *const c_char);
    if importlib_util.is_null() {
        return Err(PyErr::fetch(py));
    }
    let spec_from = ffi::PyObject_GetAttrString(
        importlib_util,
        b"spec_from_file_location\0".as_ptr() as *const c_char,
    );
    ffi::Py_DECREF(importlib_util);
    if spec_from.is_null() {
        return Err(PyErr::fetch(py));
    }

    let name_obj = ffi::PyUnicode_FromString(CString::new(name).unwrap().as_ptr());
    let path_obj = ffi::PyUnicode_FromString(CString::new(path).unwrap().as_ptr());
    if name_obj.is_null() || path_obj.is_null() {
        ffi::Py_XDECREF(name_obj);
        ffi::Py_XDECREF(path_obj);
        ffi::Py_DECREF(spec_from);
        return Err(PyErr::fetch(py));
    }
    let args = ffi::PyTuple_New(2);
    if args.is_null() {
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(path_obj);
        ffi::Py_DECREF(spec_from);
        return Err(PyErr::fetch(py));
    }
    ffi::PyTuple_SetItem(args, 0, name_obj);
    ffi::PyTuple_SetItem(args, 1, path_obj);

    let kwargs = if is_package {
        let kwargs = ffi::PyDict_New();
        if kwargs.is_null() {
            ffi::Py_DECREF(args);
            ffi::Py_DECREF(spec_from);
            return Err(PyErr::fetch(py));
        }
        let list = ffi::PyList_New(1);
        if list.is_null() {
            ffi::Py_DECREF(kwargs);
            ffi::Py_DECREF(args);
            ffi::Py_DECREF(spec_from);
            return Err(PyErr::fetch(py));
        }
        let dir = std::path::Path::new(path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        let dir_obj = ffi::PyUnicode_FromString(CString::new(dir).unwrap().as_ptr());
        if dir_obj.is_null() {
            ffi::Py_DECREF(list);
            ffi::Py_DECREF(kwargs);
            ffi::Py_DECREF(args);
            ffi::Py_DECREF(spec_from);
            return Err(PyErr::fetch(py));
        }
        ffi::PyList_SetItem(list, 0, dir_obj);
        if ffi::PyDict_SetItemString(
            kwargs,
            b"submodule_search_locations\0".as_ptr() as *const c_char,
            list,
        ) != 0
        {
            ffi::Py_DECREF(list);
            ffi::Py_DECREF(kwargs);
            ffi::Py_DECREF(args);
            ffi::Py_DECREF(spec_from);
            return Err(PyErr::fetch(py));
        }
        ffi::Py_DECREF(list);
        kwargs
    } else {
        std::ptr::null_mut()
    };

    let spec = ffi::PyObject_Call(spec_from, args, kwargs);
    ffi::Py_DECREF(spec_from);
    ffi::Py_DECREF(args);
    if !kwargs.is_null() {
        ffi::Py_DECREF(kwargs);
    }
    if spec.is_null() {
        return Err(PyErr::fetch(py));
    }
    Ok(spec)
}

unsafe fn set_spec_initializing(spec: *mut ffi::PyObject, value: bool) {
    let flag = if value {
        ffi::Py_True()
    } else {
        ffi::Py_False()
    };
    ffi::Py_INCREF(flag);
    if ffi::PyObject_SetAttrString(spec, b"_initializing\0".as_ptr() as *const c_char, flag) != 0 {
        ffi::PyErr_Clear();
    }
    ffi::Py_DECREF(flag);
}

unsafe fn collect_function_codes_from_code(
    code_obj: *mut ffi::PyObject,
    code_map: *mut ffi::PyObject,
) -> Result<(), ()> {
    if ffi::PyObject_TypeCheck(code_obj, std::ptr::addr_of_mut!(ffi::PyCode_Type)) == 0 {
        return Ok(());
    }

    let name_obj = ffi::PyObject_GetAttrString(code_obj, b"co_name\0".as_ptr() as *const c_char);
    if name_obj.is_null() {
        return Err(());
    }
    let name_utf8 = ffi::PyUnicode_AsUTF8(name_obj);
    if !name_utf8.is_null() {
        let name = CStr::from_ptr(name_utf8).to_string_lossy();
        if name.starts_with("_dp_fn_") && ffi::PyDict_SetItem(code_map, name_obj, code_obj) != 0 {
            ffi::Py_DECREF(name_obj);
            return Err(());
        }
    } else {
        ffi::PyErr_Clear();
    }
    ffi::Py_DECREF(name_obj);

    let consts = ffi::PyObject_GetAttrString(code_obj, b"co_consts\0".as_ptr() as *const c_char);
    if consts.is_null() {
        return Err(());
    }
    let len = ffi::PyTuple_Size(consts);
    if len < 0 {
        ffi::Py_DECREF(consts);
        return Err(());
    }
    for idx in 0..len {
        let item = ffi::PyTuple_GetItem(consts, idx);
        if item.is_null() {
            ffi::Py_DECREF(consts);
            return Err(());
        }
        if collect_function_codes_from_code(item, code_map).is_err() {
            ffi::Py_DECREF(consts);
            return Err(());
        }
    }
    ffi::Py_DECREF(consts);
    Ok(())
}

unsafe fn compile_transformed_function_code_map(
    py: Python<'_>,
    path: &str,
    transformed_source: &str,
) -> PyResult<*mut ffi::PyObject> {
    let source_c = CString::new(transformed_source)
        .map_err(|_| PyRuntimeError::new_err("transformed source contains NUL"))?;
    let path_c = CString::new(path).map_err(|_| PyRuntimeError::new_err("invalid path"))?;

    let module_code = ffi::Py_CompileString(source_c.as_ptr(), path_c.as_ptr(), ffi::Py_file_input);
    if module_code.is_null() {
        return Err(PyErr::fetch(py));
    }

    let code_map = ffi::PyDict_New();
    if code_map.is_null() {
        ffi::Py_DECREF(module_code);
        return Err(PyErr::fetch(py));
    }

    if collect_function_codes_from_code(module_code, code_map).is_err() {
        ffi::Py_DECREF(code_map);
        ffi::Py_DECREF(module_code);
        return Err(PyErr::fetch(py));
    }

    ffi::Py_DECREF(module_code);
    Ok(code_map)
}

pub(crate) fn eval_source_impl(py: Python<'_>, path: &str, source: &str) -> PyResult<Py<PyAny>> {
    eval_source_impl_with_name(py, path, source, "eval_source", None)
}

fn eval_source_impl_with_name_and_spec(
    py: Python<'_>,
    path: &str,
    source: &str,
    name: &str,
    package: Option<&str>,
    spec_opt: Option<*mut ffi::PyObject>,
) -> PyResult<Py<PyAny>> {
    let lowering = transform_to_min_ast(source).map_err(TransformToMinAstError::to_py_err)?;
    let module_ast = lowering.min_ast_module;
    let transformed_source = lowering.transformed_source;

    unsafe {
        let module_name =
            CString::new(name).map_err(|_| PyRuntimeError::new_err("invalid __name__"))?;
        let name_obj = ffi::PyUnicode_FromString(module_name.as_ptr());
        if name_obj.is_null() {
            return Err(PyErr::fetch(py));
        }
        let module_obj = ffi::PyModule_NewObject(name_obj);
        ffi::Py_DECREF(name_obj);
        if module_obj.is_null() {
            return Err(PyErr::fetch(py));
        }

        let layout = Box::new(interpreter::ScopeLayout::new(HashSet::new()));
        let layout_ptr = Box::into_raw(layout);
        let scope = Box::new(interpreter::ScopeInstance::new(layout_ptr));
        let scope_ptr = Box::into_raw(scope);

        let module_dict = ffi::PyModule_GetDict(module_obj);
        if module_dict.is_null() {
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }

        let name_cstr =
            CString::new(name).map_err(|_| PyRuntimeError::new_err("invalid __name__"))?;
        let name_obj = ffi::PyUnicode_FromString(name_cstr.as_ptr());
        if name_obj.is_null() || set_module_dict_item(module_obj, "__name__", name_obj).is_err() {
            ffi::Py_XDECREF(name_obj);
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }
        ffi::Py_DECREF(name_obj);

        if let Some(package) = package {
            let package_cstr = CString::new(package)
                .map_err(|_| PyRuntimeError::new_err("invalid __package__"))?;
            let package_obj = ffi::PyUnicode_FromString(package_cstr.as_ptr());
            if package_obj.is_null()
                || set_module_dict_item(module_obj, "__package__", package_obj).is_err()
            {
                ffi::Py_XDECREF(package_obj);
                ffi::Py_DECREF(module_obj);
                return Err(PyErr::fetch(py));
            }
            ffi::Py_DECREF(package_obj);
        }

        let file_obj = ffi::PyUnicode_FromString(CString::new(path).unwrap().as_ptr());
        if file_obj.is_null() || set_module_dict_item(module_obj, "__file__", file_obj).is_err() {
            ffi::Py_XDECREF(file_obj);
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }
        ffi::Py_DECREF(file_obj);

        let module_doc =
            if let Some(min_ast::StmtNode::Expr { value, .. }) = module_ast.body.first() {
                if let min_ast::ExprNode::String { value, .. } = value {
                    ffi::PyUnicode_FromStringAndSize(
                        value.as_ptr() as *const c_char,
                        value.len() as ffi::Py_ssize_t,
                    )
                } else {
                    ffi::Py_INCREF(ffi::Py_None());
                    ffi::Py_None()
                }
            } else {
                ffi::Py_INCREF(ffi::Py_None());
                ffi::Py_None()
            };
        if module_doc.is_null() || set_module_dict_item(module_obj, "__doc__", module_doc).is_err()
        {
            ffi::Py_XDECREF(module_doc);
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }
        ffi::Py_DECREF(module_doc);

        let spec = if let Some(spec) = spec_opt {
            if spec.is_null() {
                ffi::Py_DECREF(module_obj);
                return Err(PyRuntimeError::new_err("invalid __spec__"));
            }
            ffi::Py_INCREF(spec);
            spec
        } else {
            let is_package = path.ends_with("__init__.py");
            build_module_spec(py, name, path, is_package)?
        };

        set_spec_initializing(spec, true);
        if set_module_dict_item(module_obj, "__spec__", spec).is_err() {
            set_spec_initializing(spec, false);
            ffi::Py_DECREF(spec);
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }
        let submodules = ffi::PyObject_GetAttrString(
            spec,
            b"submodule_search_locations\0".as_ptr() as *const c_char,
        );
        if submodules.is_null() {
            ffi::PyErr_Clear();
        } else if submodules != ffi::Py_None() {
            if set_module_dict_item(module_obj, "__path__", submodules).is_err() {
                set_spec_initializing(spec, false);
                ffi::Py_DECREF(submodules);
                ffi::Py_DECREF(spec);
                ffi::Py_DECREF(module_obj);
                return Err(PyErr::fetch(py));
            }
            ffi::Py_DECREF(submodules);
        } else {
            ffi::Py_DECREF(submodules);
        }

        let builtins = ffi::PyEval_GetBuiltins();
        if builtins.is_null() || set_module_dict_item(module_obj, "__builtins__", builtins).is_err()
        {
            set_spec_initializing(spec, false);
            ffi::Py_DECREF(spec);
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }

        let dp_module = ffi::PyImport_ImportModule(b"__dp__\0".as_ptr() as *const c_char);
        if dp_module.is_null() {
            set_spec_initializing(spec, false);
            ffi::Py_DECREF(spec);
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }

        let runtime_fns = RuntimeFns::new(builtins, dp_module);
        ffi::Py_DECREF(dp_module);
        let runtime_fns = runtime_fns.map_err(|_| PyErr::fetch(py))?;
        let function_code_map =
            compile_transformed_function_code_map(py, path, transformed_source.as_str())?;

        let modules = ffi::PyImport_GetModuleDict();
        if modules.is_null()
            || ffi::PyDict_SetItemString(modules, name_cstr.as_ptr(), module_obj) != 0
        {
            ffi::Py_DECREF(function_code_map);
            set_spec_initializing(spec, false);
            ffi::Py_DECREF(spec);
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }

        if interpreter::eval_module(
            &module_ast,
            scope_ptr,
            module_dict,
            builtins,
            function_code_map,
            &runtime_fns,
        )
        .is_err()
        {
            if !modules.is_null() {
                ffi::PyDict_DelItemString(modules, name_cstr.as_ptr());
            }
            ffi::Py_DECREF(function_code_map);
            set_spec_initializing(spec, false);
            ffi::Py_DECREF(spec);
            ffi::Py_DECREF(module_obj);
            return Err(PyErr::fetch(py));
        }

        ffi::Py_DECREF(function_code_map);
        set_spec_initializing(spec, false);
        ffi::Py_DECREF(spec);
        let module_bound = Bound::<PyAny>::from_owned_ptr(py, module_obj);
        Ok(module_bound.unbind())
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
    spec: *mut ffi::PyObject,
) -> PyResult<Py<PyAny>> {
    eval_source_impl_with_name_and_spec(py, path, source, name, package, Some(spec))
}

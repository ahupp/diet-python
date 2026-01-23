use dp_transform::{min_ast, transform_str_to_ruff_with_options, ImportStarHandling, Options};
use pyo3::exceptions::PyRuntimeError;
use pyo3::ffi;
use pyo3::prelude::*;
use std::mem::{offset_of, size_of};
use std::os::raw::{c_char, c_int};
use std::sync::Once;

fn transform_to_min_ast(source: &str) -> Result<min_ast::Module, String> {
    let options = Options {
        import_star_handling: ImportStarHandling::Error,
        inject_import: false,
        lower_attributes: true,
        truthy: false,
        cleanup_dp_globals: false,
        force_import_rewrite: true,
        ..Options::default()
    };

    let result = transform_str_to_ruff_with_options(source, options).map_err(|err| err.to_string())?;

    Ok(min_ast::Module::from(result.module))
}

fn count_min_ast_nodes(module: min_ast::Module) -> usize {
    fn count_stmt(stmt: &min_ast::StmtNode) -> usize {
        match stmt {
            min_ast::StmtNode::FunctionDef(func) => {
                let mut count = 1;
                for param in &func.params {
                    count += count_param(param);
                }
                if let Some(returns) = &func.returns {
                    count += count_expr(returns);
                }
                for stmt in &func.body {
                    count += count_stmt(stmt);
                }
                count
            }
            min_ast::StmtNode::While {
                test, body, orelse, ..
            }
            | min_ast::StmtNode::If {
                test, body, orelse, ..
            } => {
                1 + count_expr(test)
                    + body.iter().map(count_stmt).sum::<usize>()
                    + orelse.iter().map(count_stmt).sum::<usize>()
            }
            min_ast::StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
                ..
            } => {
                1 + body.iter().map(count_stmt).sum::<usize>()
                    + handler
                        .iter()
                        .flat_map(|body| body.iter().map(count_stmt))
                        .sum::<usize>()
                    + orelse.iter().map(count_stmt).sum::<usize>()
                    + finalbody.iter().map(count_stmt).sum::<usize>()
            }
            min_ast::StmtNode::ImportFrom { .. } => 1,
            min_ast::StmtNode::Raise { exc, .. } => {
                1 + exc.as_ref().map_or(0, |value| count_expr(value))
            }
            min_ast::StmtNode::Return { value, .. } => {
                1 + value.as_ref().map_or(0, |value| count_expr(value))
            }
            min_ast::StmtNode::Expr { value, .. } => 1 + count_expr(value),
            min_ast::StmtNode::Assign { value, .. } => 1 + count_expr(value),
            min_ast::StmtNode::Delete { .. }
            | min_ast::StmtNode::Break(_)
            | min_ast::StmtNode::Continue(_)
            | min_ast::StmtNode::Pass(_) => 1,
        }
    }

    fn count_param(param: &min_ast::Parameter) -> usize {
        match param {
            min_ast::Parameter::Positional {
                annotation, default, ..
            } => {
                1 + annotation.as_ref().map_or(0, |expr| count_expr(expr))
                    + default.as_ref().map_or(0, |expr| count_expr(expr))
            }
            min_ast::Parameter::VarArg { annotation, .. }
            | min_ast::Parameter::KwArg { annotation, .. } => {
                1 + annotation.as_ref().map_or(0, |expr| count_expr(expr))
            }
            min_ast::Parameter::KwOnly {
                annotation, default, ..
            } => {
                1 + annotation.as_ref().map_or(0, |expr| count_expr(expr))
                    + default.as_ref().map_or(0, |expr| count_expr(expr))
            }
        }
    }

    fn count_expr(expr: &min_ast::ExprNode) -> usize {
        match expr {
            min_ast::ExprNode::Name { .. }
            | min_ast::ExprNode::Number { .. }
            | min_ast::ExprNode::String { .. }
            | min_ast::ExprNode::Bytes { .. } => 1,
            min_ast::ExprNode::Attribute { value, .. } => 1 + count_expr(value),
            min_ast::ExprNode::Tuple { elts, .. } => {
                1 + elts.iter().map(count_expr).sum::<usize>()
            }
            min_ast::ExprNode::Await { value, .. } => 1 + count_expr(value),
            min_ast::ExprNode::Yield { value, .. } => {
                1 + value.as_ref().map_or(0, |expr| count_expr(expr))
            }
            min_ast::ExprNode::Call { func, args, .. } => {
                1 + count_expr(func)
                    + args.iter().map(count_arg).sum::<usize>()
            }
        }
    }

    fn count_arg(arg: &min_ast::Arg) -> usize {
        match arg {
            min_ast::Arg::Positional(expr)
            | min_ast::Arg::Starred(expr)
            | min_ast::Arg::KwStarred(expr) => 1 + count_expr(expr),
            min_ast::Arg::Keyword { value, .. } => 1 + count_expr(value),
        }
    }

    1 + module.body.iter().map(count_stmt).sum::<usize>()
}

#[repr(C)]
struct SoacModule {
    ob_base: ffi::PyObject,
    dict: *mut ffi::PyObject,
    nodes: usize,
}

#[repr(C)]
struct SoacModuleDictProxy {
    ob_base: ffi::PyObject,
    module: *mut SoacModule,
}

unsafe extern "C" fn soac_module_dealloc(obj: *mut ffi::PyObject) {
    let module = obj as *mut SoacModule;
    if !(*module).dict.is_null() {
        ffi::Py_DECREF((*module).dict);
    }
    ffi::PyObject_Free(obj as *mut std::ffi::c_void);
}

unsafe fn soac_module_setattr_impl(
    module: *mut SoacModule,
    name: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> c_int {
    if ffi::PyUnicode_Check(name) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"attribute name must be str\0".as_ptr() as *const c_char,
        );
        return -1;
    }

    if (*module).dict.is_null() {
        (*module).dict = ffi::PyDict_New();
        if (*module).dict.is_null() {
            return -1;
        }
    }

    if !value.is_null() {
        ffi::PySys_WriteStdout(b"setattr\n\0".as_ptr() as *const c_char);
        ffi::PyDict_SetItem((*module).dict, name, value)
    } else {
        ffi::PyDict_DelItem((*module).dict, name)
    }
}

unsafe extern "C" fn soac_module_getattro(
    obj: *mut ffi::PyObject,
    name: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    if ffi::PyUnicode_Check(name) != 0 {
        let mut len: ffi::Py_ssize_t = 0;
        let ptr = ffi::PyUnicode_AsUTF8AndSize(name, &mut len);
        if !ptr.is_null() && len == 8 {
            let bytes = std::slice::from_raw_parts(ptr as *const u8, 8);
            if bytes == b"__dict__" {
                return soac_module_dictproxy_new(obj as *mut SoacModule);
            }
        }
        if !ptr.is_null() && len == 5 {
            let bytes = std::slice::from_raw_parts(ptr as *const u8, 5);
            if bytes == b"nodes" {
                let module = obj as *mut SoacModule;
                return ffi::PyLong_FromSize_t((*module).nodes as _);
            }
        }
    }
    let module = obj as *mut SoacModule;
    if !(*module).dict.is_null() {
        let value = ffi::PyDict_GetItemWithError((*module).dict, name);
        if !value.is_null() {
            ffi::Py_INCREF(value);
            return value;
        }
        if ffi::PyErr_Occurred().is_null() == false {
            return std::ptr::null_mut();
        }
    }
    ffi::PyObject_GenericGetAttr(obj, name)
}

unsafe extern "C" fn soac_module_setattro(
    obj: *mut ffi::PyObject,
    name: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> c_int {
    if ffi::PyUnicode_Check(name) != 0 {
        let mut len: ffi::Py_ssize_t = 0;
        let ptr = ffi::PyUnicode_AsUTF8AndSize(name, &mut len);
        if !ptr.is_null() && len == 8 {
            let bytes = std::slice::from_raw_parts(ptr as *const u8, 8);
            if bytes == b"__dict__" {
                ffi::PyErr_SetString(
                    ffi::PyExc_AttributeError,
                    b"__dict__ is read-only\0".as_ptr() as *const c_char,
                );
                return -1;
            }
        }
    }
    soac_module_setattr_impl(obj as *mut SoacModule, name, value)
}

unsafe extern "C" fn soac_module_dir(
    obj: *mut ffi::PyObject,
    _args: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    let module = obj as *mut SoacModule;
    let list = if (*module).dict.is_null() {
        ffi::PyList_New(0)
    } else {
        ffi::PyDict_Keys((*module).dict)
    };
    if list.is_null() {
        return std::ptr::null_mut();
    }
    if ffi::PyList_Sort(list) != 0 {
        ffi::Py_DECREF(list);
        return std::ptr::null_mut();
    }
    list
}

unsafe extern "C" fn soac_module_setattribute(
    obj: *mut ffi::PyObject,
    args: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    if ffi::PyTuple_Check(args) == 0 || ffi::PyTuple_Size(args) != 2 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"__setattribute__ expects (name, value)\0".as_ptr() as *const c_char,
        );
        return std::ptr::null_mut();
    }
    let name = ffi::PyTuple_GetItem(args, 0);
    let value = ffi::PyTuple_GetItem(args, 1);
    if name.is_null() || value.is_null() {
        return std::ptr::null_mut();
    }
    if soac_module_setattro(obj, name, value) != 0 {
        return std::ptr::null_mut();
    }
    ffi::Py_INCREF(ffi::Py_None());
    ffi::Py_None()
}

unsafe extern "C" fn soac_module_dictproxy_dealloc(obj: *mut ffi::PyObject) {
    let proxy = obj as *mut SoacModuleDictProxy;
    if !(*proxy).module.is_null() {
        ffi::Py_DECREF((*proxy).module as *mut ffi::PyObject);
    }
    ffi::PyObject_Free(obj as *mut std::ffi::c_void);
}

unsafe extern "C" fn soac_module_dictproxy_subscript(
    obj: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    let proxy = obj as *mut SoacModuleDictProxy;
    if (*proxy).module.is_null() || (*(*proxy).module).dict.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"module dict unavailable\0".as_ptr() as *const c_char,
        );
        return std::ptr::null_mut();
    }
    ffi::PyObject_GetItem((*(*proxy).module).dict, key)
}

unsafe extern "C" fn soac_module_dictproxy_ass_subscript(
    obj: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> c_int {
    let proxy = obj as *mut SoacModuleDictProxy;
    if (*proxy).module.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"module dict unavailable\0".as_ptr() as *const c_char,
        );
        return -1;
    }
    soac_module_setattro((*proxy).module as *mut ffi::PyObject, key, value)
}

static mut SOAC_MODULE_DICTPROXY_MAPPING: ffi::PyMappingMethods = ffi::PyMappingMethods {
    mp_length: None,
    mp_subscript: Some(soac_module_dictproxy_subscript),
    mp_ass_subscript: Some(soac_module_dictproxy_ass_subscript),
};

#[allow(clippy::uninit_assumed_init)]
static mut SOAC_MODULE_DICTPROXY_TYPE: ffi::PyTypeObject = ffi::PyTypeObject {
    ob_base: ffi::PyVarObject {
        ob_base: ffi::PyObject_HEAD_INIT,
        ob_size: 0,
    },
    tp_name: b"diet_python.SoacModuleDictProxy\0".as_ptr() as *const _,
    tp_basicsize: size_of::<SoacModuleDictProxy>() as ffi::Py_ssize_t,
    tp_itemsize: 0,
    tp_dealloc: Some(soac_module_dictproxy_dealloc),
    tp_as_mapping: std::ptr::addr_of_mut!(SOAC_MODULE_DICTPROXY_MAPPING),
    tp_flags: ffi::Py_TPFLAGS_DEFAULT,
    ..unsafe { std::mem::zeroed() }
};

static INIT_SOAC_MODULE_DICTPROXY_TYPE: Once = Once::new();

unsafe fn init_soac_module_dictproxy_type() -> PyResult<()> {
    let mut result = Ok(());
    INIT_SOAC_MODULE_DICTPROXY_TYPE.call_once(|| {
        if ffi::PyType_Ready(std::ptr::addr_of_mut!(SOAC_MODULE_DICTPROXY_TYPE)) < 0 {
            result = Err(PyErr::fetch(Python::assume_attached()));
        }
    });
    result?;
    Ok(())
}

unsafe fn soac_module_dictproxy_new(module: *mut SoacModule) -> *mut ffi::PyObject {
    if let Err(err) = init_soac_module_dictproxy_type() {
        err.restore(Python::assume_attached());
        return std::ptr::null_mut();
    }
    let proxy = ffi::_PyObject_New(std::ptr::addr_of_mut!(SOAC_MODULE_DICTPROXY_TYPE))
        as *mut SoacModuleDictProxy;
    if proxy.is_null() {
        return std::ptr::null_mut();
    }
    (*proxy).module = module;
    ffi::Py_INCREF(module as *mut ffi::PyObject);
    proxy as *mut ffi::PyObject
}

static mut SOAC_MODULE_METHODS: [ffi::PyMethodDef; 3] = [
    ffi::PyMethodDef {
        ml_name: b"__dir__\0".as_ptr() as *const c_char,
        ml_meth: ffi::PyMethodDefPointer {
            PyCFunction: soac_module_dir,
        },
        ml_flags: ffi::METH_NOARGS,
        ml_doc: std::ptr::null(),
    },
    ffi::PyMethodDef {
        ml_name: b"__setattribute__\0".as_ptr() as *const c_char,
        ml_meth: ffi::PyMethodDefPointer {
            PyCFunction: soac_module_setattribute,
        },
        ml_flags: ffi::METH_VARARGS,
        ml_doc: std::ptr::null(),
    },
    ffi::PyMethodDef::zeroed(),
];

#[allow(clippy::uninit_assumed_init)]
static mut SOAC_MODULE_TYPE: ffi::PyTypeObject = ffi::PyTypeObject {
    ob_base: ffi::PyVarObject {
        ob_base: ffi::PyObject_HEAD_INIT,
        ob_size: 0,
    },
    tp_name: b"diet_python.SoacModule\0".as_ptr() as *const _,
    tp_basicsize: size_of::<SoacModule>() as ffi::Py_ssize_t,
    tp_itemsize: 0,
    tp_dealloc: Some(soac_module_dealloc),
    tp_getattro: Some(soac_module_getattro),
    tp_setattro: Some(soac_module_setattro),
    tp_methods: std::ptr::addr_of_mut!(SOAC_MODULE_METHODS) as *mut ffi::PyMethodDef,
    tp_flags: ffi::Py_TPFLAGS_DEFAULT,
    tp_dictoffset: offset_of!(SoacModule, dict) as ffi::Py_ssize_t,
    ..unsafe { std::mem::zeroed() }
};

static INIT_SOAC_MODULE_TYPE: Once = Once::new();

unsafe fn init_soac_module_type() -> PyResult<()> {
    let mut result = Ok(());
    INIT_SOAC_MODULE_TYPE.call_once(|| {
        if ffi::PyType_Ready(std::ptr::addr_of_mut!(SOAC_MODULE_TYPE)) < 0 {
            result = Err(PyErr::fetch(Python::assume_attached()));
        }
    });
    result?;
    Ok(())
}

pub(crate) fn eval_source_impl(py: Python<'_>, path: &str, source: &str) -> PyResult<Py<PyAny>> {
    let module = transform_to_min_ast(source).map_err(PyRuntimeError::new_err)?;
    let nodes = count_min_ast_nodes(module);
    unsafe {
        init_soac_module_type()?;
        let module = ffi::_PyObject_New(std::ptr::addr_of_mut!(SOAC_MODULE_TYPE)) as *mut SoacModule;
        if module.is_null() {
            return Err(PyErr::fetch(py));
        }

        (*module).dict = ffi::PyDict_New();
        if (*module).dict.is_null() {
            ffi::PyObject_Free(module as *mut std::ffi::c_void);
            return Err(PyErr::fetch(py));
        }
        (*module).nodes = nodes;

        let module_obj = Bound::<PyAny>::from_owned_ptr(py, module as *mut ffi::PyObject);
        module_obj.setattr("__name__", "eval_source")?;
        module_obj.setattr("__file__", path)?;
        module_obj.setattr("nodes", nodes)?;
        Ok(module_obj.unbind())
    }
}

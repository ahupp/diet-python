use crate::interpreter::{self, RuntimeFns};
use dp_transform::{min_ast, transform_str_to_ruff_with_options, ImportStarHandling, Options};
use pyo3::exceptions::PyRuntimeError;
use pyo3::ffi;
use pyo3::prelude::*;
use std::ffi::CString;
use std::mem::size_of;
use std::os::raw::{c_char, c_int};
use std::sync::Once;

fn transform_to_min_ast(source: &str) -> Result<min_ast::Module, String> {
    let options = Options {
        import_star_handling: ImportStarHandling::Error,
        inject_import: true,
        lower_attributes: true,
        truthy: false,
        cleanup_dp_globals: false,
        force_import_rewrite: true,
        ..Options::default()
    };

    let result = transform_str_to_ruff_with_options(source, options).map_err(|err| err.to_string())?;

    Ok(min_ast::Module::from(result.module))
}

fn count_min_ast_nodes(module: &min_ast::Module) -> usize {
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
    scope: *mut interpreter::ScopeInstance,
    layout: *mut interpreter::ScopeLayout,
    nodes: usize,
}

unsafe extern "C" fn soac_module_dealloc(obj: *mut ffi::PyObject) {
    let module = obj as *mut SoacModule;
    if !(*module).scope.is_null() {
        drop(Box::from_raw((*module).scope));
    }
    if !(*module).layout.is_null() {
        drop(Box::from_raw((*module).layout));
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

    if (*module).scope.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"module scope unavailable\0".as_ptr() as *const c_char,
        );
        return -1;
    }

    let mut len: ffi::Py_ssize_t = 0;
    let ptr = ffi::PyUnicode_AsUTF8AndSize(name, &mut len);
    if ptr.is_null() {
        return -1;
    }
    let key = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
        ptr as *const u8,
        len as usize,
    ));

    let result = if value.is_null() {
        interpreter::scope_delete_name(&mut *(*module).scope, key)
    } else {
        interpreter::scope_assign_name(&mut *(*module).scope, key, value)
    };

    if result.is_err() {
        if value.is_null() && ffi::PyErr_ExceptionMatches(ffi::PyExc_NameError) != 0 {
            ffi::PyErr_Clear();
            let msg = CString::new(format!("module has no attribute '{key}'")).unwrap();
            ffi::PyErr_SetString(ffi::PyExc_AttributeError, msg.as_ptr());
        }
        return -1;
    }
    0
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
                let module = obj as *mut SoacModule;
                if (*module).scope.is_null() {
                    ffi::PyErr_SetString(
                        ffi::PyExc_RuntimeError,
                        b"module scope unavailable\0".as_ptr() as *const c_char,
                    );
                    return std::ptr::null_mut();
                }
                return interpreter::scope_dictproxy_new((*module).scope, obj);
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
    if !(*module).scope.is_null() && ffi::PyUnicode_Check(name) != 0 {
        let mut len: ffi::Py_ssize_t = 0;
        let ptr = ffi::PyUnicode_AsUTF8AndSize(name, &mut len);
        if !ptr.is_null() {
            let key = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                ptr as *const u8,
                len as usize,
            ));
            let value = interpreter::scope_lookup_name(&*(*module).scope, key);
            if !value.is_null() {
                return value;
            }
        } else {
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
    if (*module).scope.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"module scope unavailable\0".as_ptr() as *const c_char,
        );
        return std::ptr::null_mut();
    }
    let dict = match interpreter::scope_to_dict(&*(*module).scope) {
        Ok(dict) => dict,
        Err(()) => return std::ptr::null_mut(),
    };
    let list = ffi::PyDict_Keys(dict);
    ffi::Py_DECREF(dict);
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
    let module_ast = transform_to_min_ast(source).map_err(PyRuntimeError::new_err)?;
    let nodes = count_min_ast_nodes(&module_ast);
    unsafe {
        init_soac_module_type()?;
        let module =
            ffi::_PyObject_New(std::ptr::addr_of_mut!(SOAC_MODULE_TYPE)) as *mut SoacModule;
        if module.is_null() {
            return Err(PyErr::fetch(py));
        }
        (*module).scope = std::ptr::null_mut();
        (*module).layout = std::ptr::null_mut();
        (*module).nodes = nodes;

        let layout = Box::new(interpreter::build_module_layout(&module_ast));
        let layout_ptr = Box::into_raw(layout);
        let scope = Box::new(interpreter::ScopeInstance::new(layout_ptr));
        let scope_ptr = Box::into_raw(scope);
        (*module).layout = layout_ptr;
        (*module).scope = scope_ptr;

        let name_obj = ffi::PyUnicode_FromString(b"eval_source\0".as_ptr() as *const c_char);
        if name_obj.is_null()
            || interpreter::scope_assign_name(&mut *scope_ptr, "__name__", name_obj).is_err()
        {
            ffi::Py_XDECREF(name_obj);
            ffi::Py_DECREF(module as *mut ffi::PyObject);
            return Err(PyErr::fetch(py));
        }
        ffi::Py_DECREF(name_obj);

        let file_obj = ffi::PyUnicode_FromString(CString::new(path).unwrap().as_ptr());
        if file_obj.is_null()
            || interpreter::scope_assign_name(&mut *scope_ptr, "__file__", file_obj).is_err()
        {
            ffi::Py_XDECREF(file_obj);
            ffi::Py_DECREF(module as *mut ffi::PyObject);
            return Err(PyErr::fetch(py));
        }
        ffi::Py_DECREF(file_obj);

        let builtins = ffi::PyEval_GetBuiltins();
        if builtins.is_null()
            || interpreter::scope_assign_name(&mut *scope_ptr, "__builtins__", builtins).is_err()
        {
            ffi::Py_DECREF(module as *mut ffi::PyObject);
            return Err(PyErr::fetch(py));
        }

        let dp_module = ffi::PyImport_ImportModule(b"__dp__\0".as_ptr() as *const c_char);
        if dp_module.is_null() {
            ffi::Py_DECREF(module as *mut ffi::PyObject);
            return Err(PyErr::fetch(py));
        }

        let runtime_fns = RuntimeFns::new(builtins, dp_module);
        ffi::Py_DECREF(dp_module);
        let runtime_fns = runtime_fns.map_err(|_| PyErr::fetch(py))?;

        if interpreter::eval_module(
            &module_ast,
            scope_ptr,
            module as *mut ffi::PyObject,
            builtins,
            &runtime_fns,
        )
        .is_err()
        {
            ffi::Py_DECREF(module as *mut ffi::PyObject);
            return Err(PyErr::fetch(py));
        }

        let module_obj = Bound::<PyAny>::from_owned_ptr(py, module as *mut ffi::PyObject);
        Ok(module_obj.unbind())
    }
}

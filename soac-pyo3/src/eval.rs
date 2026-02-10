use dp_transform::{Options, min_ast, transform_str_to_ruff_with_options};
use pyo3::exceptions::{PyRuntimeError, PySyntaxError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use soac_eval::tree_walk::{self as interpreter, RuntimeFns};
use std::any::Any;
use std::collections::HashSet;
use std::ffi::CString;

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

fn collect_function_codes_from_code(
    code_obj: &Bound<'_, PyAny>,
    code_map: &Bound<'_, PyDict>,
) -> PyResult<()> {
    unsafe {
        if ffi::PyObject_TypeCheck(code_obj.as_ptr(), std::ptr::addr_of_mut!(ffi::PyCode_Type)) == 0
        {
            return Ok(());
        }
    }

    let name_obj = code_obj.getattr("co_name")?;
    if let Ok(name) = name_obj.extract::<String>() {
        if name.starts_with("_dp_fn_") {
            code_map.set_item(name_obj, code_obj)?;
        }
    }

    let consts = code_obj.getattr("co_consts")?;
    for item in consts.try_iter()? {
        collect_function_codes_from_code(&item?, code_map)?;
    }
    Ok(())
}

fn compile_transformed_function_code_map(
    py: Python<'_>,
    path: &str,
    transformed_source: &str,
) -> PyResult<Py<PyDict>> {
    let builtins = py.import("builtins")?;
    let module_code = builtins
        .getattr("compile")?
        .call1((transformed_source, path, "exec"))?;
    let code_map = PyDict::new(py);
    collect_function_codes_from_code(&module_code, &code_map)?;
    Ok(code_map.unbind())
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
    spec_opt: Option<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    let lowering = transform_to_min_ast(source).map_err(TransformToMinAstError::to_py_err)?;
    let module_ast = lowering.min_ast_module;
    let transformed_source = lowering.transformed_source;

    unsafe {
        let module = PyModule::new(py, name)?;

        let layout = Box::new(interpreter::ScopeLayout::new(HashSet::new()));
        let layout_ptr = Box::into_raw(layout);
        let scope = Box::new(interpreter::ScopeInstance::new(layout_ptr));
        let scope_ptr = Box::into_raw(scope);

        module.setattr("__name__", name)?;

        if let Some(package) = package {
            module.setattr("__package__", package)?;
        }

        module.setattr("__file__", path)?;

        if let Some(min_ast::StmtNode::Expr { value, .. }) = module_ast.body.first() {
            if let min_ast::ExprNode::String { value, .. } = value {
                module.setattr("__doc__", value)?;
            } else {
                module.setattr("__doc__", py.None())?;
            }
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
            let builtins_dict = Bound::<PyAny>::from_borrowed_ptr(py, builtins)
                .cast_into::<PyDict>()?;
            module.setattr("__builtins__", &builtins_dict)?;

            let dp_module = py.import("__dp__")?;
            let runtime_fns = RuntimeFns::new(&builtins_dict, &dp_module.as_any())?;
            let function_code_map =
                compile_transformed_function_code_map(py, path, transformed_source.as_str())?;

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

            if interpreter::eval_module(
                &module_ast,
                scope_ptr,
                module_dict.as_ptr(),
                builtins,
                function_code_map.bind(py).as_ptr(),
                &runtime_fns,
            )
            .is_err()
            {
                ffi::PyDict_DelItemString(modules, name_cstr.as_ptr());
                return Err(PyErr::fetch(py));
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

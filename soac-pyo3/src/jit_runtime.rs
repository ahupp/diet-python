use crate::lowering_error_to_pyerr;
use log::info;
use pyo3::exceptions::{
    PyAttributeError, PyNotImplementedError, PyRuntimeError, PyTypeError, PyValueError,
};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyFunction, PyModule, PyString, PyTuple};
use soac_blockpy::block_py::{BlockPyFunction, BlockPyModule, FunctionId, ParamKind};
use soac_blockpy::lower_python_to_blockpy;
use soac_blockpy::pass_tracker::NoopPassTracker;
use soac_blockpy::passes::CodegenBlockPyPass;
use soac_eval::module_type::SoacExtModule;
use std::time::Instant;

unsafe extern "C" {
    static mut PyCell_Type: ffi::PyTypeObject;
    fn PyCell_New(obj: *mut ffi::PyObject) -> *mut ffi::PyObject;
}

fn is_cell_object(obj: *mut ffi::PyObject) -> bool {
    unsafe { !obj.is_null() && ffi::Py_TYPE(obj) == std::ptr::addr_of_mut!(PyCell_Type) }
}

fn import_dp_module<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyModule>> {
    PyModule::import(py, "soac.runtime")
}

pub(crate) fn register_lowered_module_plans<P>(
    output: &soac_blockpy::LoweringResult<P>,
    module_name: &str,
) -> PyResult<()> {
    register_blockpy_module_plans(module_name, &output.codegen_module)
}

fn register_blockpy_module_plans(
    module_name: &str,
    module: &BlockPyModule<CodegenBlockPyPass>,
) -> PyResult<()> {
    soac_eval::jit::register_clif_module_plans(module_name, module).map_err(|err| {
        pyo3::exceptions::PyRuntimeError::new_err(format!(
            "failed to register BB plans for {module_name}: {err}"
        ))
    })?;
    if module_name.ends_with(".__main__") && module_name != "__main__" {
        soac_eval::jit::register_clif_module_plans("__main__", module).map_err(|err| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "failed to register BB plans alias for __main__ from {module_name}: {err}"
            ))
        })?;
    }
    Ok(())
}

fn make_lazy_clif_entry<'py>(
    py: Python<'py>,
    dp: &Bound<'py, PyModule>,
    function_name: &str,
    module_globals: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let module_globals = module_globals
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("module_globals must be a dict"))?;
    let template = dp.getattr("_dp_entry_template")?;
    let code = template.getattr("__code__")?;
    unsafe {
        let func = ffi::PyFunction_New(code.as_ptr(), module_globals.as_ptr());
        if func.is_null() {
            return Err(PyErr::fetch(py));
        }
        let func = Bound::from_owned_ptr(py, func);
        func.setattr("__name__", function_name)?;
        Ok(func)
    }
}

fn register_clif_vectorcall_raw(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    module_name: &str,
    function_id: FunctionId,
) -> PyResult<()> {
    unsafe {
        soac_eval::tree_walk::register_clif_vectorcall(func.as_ptr(), module_name, function_id.0)
            .map_err(|_| {
                if ffi::PyErr_Occurred().is_null() {
                    PyRuntimeError::new_err("failed to register CLIF vectorcall")
                } else {
                    PyErr::fetch(py)
                }
            })
    }
}

fn eager_clif_compile_requested() -> bool {
    std::env::var("DIET_PYTHON_JIT_COMPILE_MODE")
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("eager"))
        .unwrap_or(false)
}

fn maybe_eager_compile_clif_entry(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    module_name: &str,
    function_id: FunctionId,
) -> PyResult<()> {
    if !eager_clif_compile_requested() {
        return Ok(());
    }
    let start = Instant::now();
    let compile_result = unsafe {
        soac_eval::tree_walk::compile_clif_vectorcall(func.as_ptr()).map_err(|_| {
            if ffi::PyErr_Occurred().is_null() {
                PyRuntimeError::new_err("failed to eagerly compile CLIF entry")
            } else {
                PyErr::fetch(py)
            }
        })
    };
    match compile_result {
        Ok(()) => {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            info!(
                "soac_jit_eager_compile module={} function_id={} elapsed_ms={elapsed_ms:.3}",
                module_name, function_id.0
            );
            Ok(())
        }
        Err(err) if err.is_instance_of::<PyNotImplementedError>(py) => Err(err),
        Err(err) => Err(PyRuntimeError::new_err(format!(
            "failed to eagerly compile CLIF entry for {module_name} function_id={}: {err}",
            function_id.0
        ))),
    }
}

fn register_lazy_clif_vectorcall(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    module_name: &str,
    function_id: FunctionId,
) -> PyResult<()> {
    match register_clif_vectorcall_raw(py, func, module_name, function_id) {
        Ok(()) => maybe_eager_compile_clif_entry(py, func, module_name, function_id),
        Err(err) if err.is_instance_of::<PyNotImplementedError>(py) => Err(err),
        Err(err) => Err(PyRuntimeError::new_err(format!(
            "failed to register lazy CLIF vectorcall for {module_name} function_id={}: {err}",
            function_id.0
        ))),
    }
}

fn ignore_attr_or_type_error(py: Python<'_>, result: PyResult<()>) -> PyResult<()> {
    match result {
        Ok(()) => Ok(()),
        Err(err)
            if err.is_instance_of::<PyAttributeError>(py)
                || err.is_instance_of::<PyTypeError>(py) =>
        {
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn ignore_attr_or_value_error<T>(py: Python<'_>, result: PyResult<T>) -> PyResult<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err)
            if err.is_instance_of::<PyAttributeError>(py)
                || err.is_instance_of::<PyValueError>(py) =>
        {
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

fn update_function_metadata(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    qualname: &str,
    name: &str,
    doc: Option<&str>,
    annotate_fn: &Bound<'_, PyAny>,
) -> PyResult<()> {
    ignore_attr_or_type_error(py, func.setattr("__qualname__", qualname))?;
    ignore_attr_or_type_error(py, func.setattr("__name__", name))?;
    if func.cast::<PyFunction>().is_ok() {
        let kwargs = PyDict::new(py);
        kwargs.set_item("co_name", name)?;
        kwargs.set_item("co_qualname", qualname)?;
        if let Some(replaced) = ignore_attr_or_value_error(
            py,
            func.getattr("__code__")?
                .call_method("replace", (), Some(&kwargs)),
        )? {
            ignore_attr_or_type_error(py, func.setattr("__code__", replaced))?;
        }
    }
    if let Some(doc) = doc {
        ignore_attr_or_type_error(py, func.setattr("__doc__", doc))?;
    }
    if !annotate_fn.is_none() {
        ignore_attr_or_type_error(py, func.setattr("__annotate__", annotate_fn))?;
    }
    Ok(())
}

fn resolve_module_name(module_globals: &Bound<'_, PyAny>, operation: &str) -> PyResult<String> {
    let globals = module_globals
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("module_globals must be a dict"))?;
    let Some(module_name_obj) = globals.get_item("__name__")? else {
        return Err(PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} requires module_globals['__name__']"
        )));
    };
    module_name_obj.extract::<String>().map_err(|_| {
        PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} requires module_globals['__name__'] to be a str"
        ))
    })
}

fn resolve_module_package(module_globals: &Bound<'_, PyAny>, operation: &str) -> PyResult<String> {
    let globals = module_globals
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("module_globals must be a dict"))?;
    let Some(module_package_obj) = globals.get_item("__package__")? else {
        return Err(PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} requires module_globals['__package__']"
        )));
    };
    module_package_obj.extract::<String>().map_err(|_| {
        PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} requires module_globals['__package__'] to be a str"
        ))
    })
}

fn lookup_bb_function(
    module_name: &str,
    function_id: usize,
    operation: &str,
) -> PyResult<BlockPyFunction<CodegenBlockPyPass>> {
    soac_eval::jit::lookup_blockpy_function(module_name, function_id).ok_or_else(|| {
        PyRuntimeError::new_err(format!(
            "JIT basic-block {operation} failed to resolve static function metadata for {module_name}.fn#{function_id}"
        ))
    })
}

fn lookup_module_init_function(
    module: &BlockPyModule<CodegenBlockPyPass>,
    module_name: &str,
) -> PyResult<BlockPyFunction<CodegenBlockPyPass>> {
    module
        .callable_defs
        .iter()
        .find(|function| function.names.bind_name == "_dp_module_init")
        .cloned()
        .ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "JIT basic-block module init failed to resolve lowered _dp_module_init for {module_name}"
            ))
        })
}

fn build_capture_map<'py>(
    py: Python<'py>,
    captures: &Bound<'py, PyAny>,
) -> PyResult<(Vec<String>, Bound<'py, PyDict>)> {
    let captures = captures.cast::<PyTuple>().map_err(|_| {
        PyTypeError::new_err(format!(
            "bb captures must be a tuple, got {:?}",
            captures.get_type()
        ))
    })?;
    let closure_values = PyDict::new(py);
    let mut captured_names = Vec::with_capacity(captures.len());
    for item in captures.iter() {
        let item = item
            .cast::<PyTuple>()
            .map_err(|_| PyTypeError::new_err(format!("invalid bb capture payload: {item:?}")))?;
        if item.len() != 2 {
            return Err(PyTypeError::new_err(format!(
                "invalid bb capture payload: {item:?}"
            )));
        }
        let name = item
            .get_item(0)?
            .extract::<String>()
            .map_err(|_| PyTypeError::new_err(format!("invalid bb capture payload: {item:?}")))?;
        let value = item.get_item(1)?;
        closure_values.set_item(name.as_str(), &value)?;
        captured_names.push(name);
    }
    Ok((captured_names, closure_values))
}

fn split_param_defaults<'py>(
    py: Python<'py>,
    function: &BlockPyFunction<CodegenBlockPyPass>,
    param_defaults: &Bound<'py, PyAny>,
) -> PyResult<(Option<Bound<'py, PyTuple>>, Option<Bound<'py, PyDict>>)> {
    let defaults = param_defaults.cast::<PyTuple>().map_err(|_| {
        PyTypeError::new_err(format!(
            "bb param defaults must be a tuple, got {:?}",
            param_defaults.get_type()
        ))
    })?;
    let mut default_index = 0usize;
    let mut positional_defaults = Vec::new();
    let kwdefaults = PyDict::new(py);
    for param in &function.params.params {
        if !param.has_default {
            continue;
        }
        let value = defaults.get_item(default_index).map_err(|_| {
            PyRuntimeError::new_err("bb param defaults payload is shorter than the param spec")
        })?;
        default_index += 1;
        match param.kind {
            ParamKind::PosOnly | ParamKind::Any => positional_defaults.push(value.unbind()),
            ParamKind::KwOnly => kwdefaults.set_item(param.name.as_str(), &value)?,
            ParamKind::VarArg | ParamKind::KwArg => {
                return Err(PyRuntimeError::new_err(format!(
                    "invalid default-bearing bb param kind: {:?}",
                    param.kind
                )));
            }
        }
    }
    if default_index != defaults.len() {
        return Err(PyRuntimeError::new_err(
            "bb param defaults payload is longer than the param spec",
        ));
    }
    let positional_defaults = if positional_defaults.is_empty() {
        None
    } else {
        Some(PyTuple::new(py, positional_defaults)?)
    };
    let kwdefaults = if kwdefaults.is_empty() {
        None
    } else {
        Some(kwdefaults)
    };
    Ok((positional_defaults, kwdefaults))
}

fn inspect_param_kind<'py>(
    inspect_module: &Bound<'py, PyModule>,
    kind: ParamKind,
) -> PyResult<Bound<'py, PyAny>> {
    let parameter = inspect_module.getattr("Parameter")?;
    match kind {
        ParamKind::PosOnly => parameter.getattr("POSITIONAL_ONLY"),
        ParamKind::Any => parameter.getattr("POSITIONAL_OR_KEYWORD"),
        ParamKind::VarArg => parameter.getattr("VAR_POSITIONAL"),
        ParamKind::KwOnly => parameter.getattr("KEYWORD_ONLY"),
        ParamKind::KwArg => parameter.getattr("VAR_KEYWORD"),
    }
}

fn build_bb_signature<'py>(
    py: Python<'py>,
    function: &BlockPyFunction<CodegenBlockPyPass>,
    param_defaults: &Bound<'py, PyAny>,
) -> PyResult<Py<PyAny>> {
    let inspect_module = PyModule::import(py, "inspect")?;
    let parameter = inspect_module.getattr("Parameter")?;
    let signature = inspect_module.getattr("Signature")?;
    let empty_default = inspect_module.getattr("_empty")?;
    let defaults = param_defaults.cast::<PyTuple>().map_err(|_| {
        PyTypeError::new_err(format!(
            "bb param defaults must be a tuple, got {:?}",
            param_defaults.get_type()
        ))
    })?;
    let mut default_index = 0usize;
    let mut signature_params = Vec::with_capacity(function.params.params.len());
    for param in &function.params.params {
        let kind = inspect_param_kind(&inspect_module, param.kind)?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("name", param.name.as_str())?;
        kwargs.set_item("kind", &kind)?;
        if param.has_default {
            let value = defaults.get_item(default_index).map_err(|_| {
                PyRuntimeError::new_err("bb param defaults payload is shorter than the param spec")
            })?;
            default_index += 1;
            kwargs.set_item("default", &value)?;
        } else {
            kwargs.set_item("default", &empty_default)?;
        }
        signature_params.push(parameter.call((), Some(&kwargs))?.unbind());
    }
    if default_index != defaults.len() {
        return Err(PyRuntimeError::new_err(
            "bb param defaults payload is longer than the param spec",
        ));
    }
    let signature_obj = signature.call1((PyTuple::new(py, signature_params)?,))?;
    Ok(signature_obj.unbind())
}

fn build_generator_code<'py>(
    py: Python<'py>,
    dp: &Bound<'py, PyModule>,
    async_gen: bool,
    name: &str,
    qualname: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let template_name = if async_gen {
        "_dp_async_gen_code_template"
    } else {
        "_dp_gen_code_template"
    };
    let template = dp.getattr(template_name)?;
    let code = template.getattr("__code__")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("co_name", name)?;
    kwargs.set_item("co_qualname", qualname)?;
    code.call_method("replace", (), Some(&kwargs))
}

fn build_wrapped_entry<'py>(
    py: Python<'py>,
    dp: &Bound<'py, PyModule>,
    raw_entry: &Bound<'py, PyAny>,
    module_globals: &Bound<'py, PyAny>,
    qualname: &str,
    captured_names: &[String],
    captured_values: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    if captured_names.is_empty() || raw_entry.getattr("__closure__")?.is_truthy()? {
        return Ok(raw_entry.clone());
    }
    let code = dp.getattr("code_with_freevars")?.call1((
        PyTuple::new(py, captured_names)?,
        false,
        false,
    ))?;
    let freevars_obj = code.getattr("co_freevars")?;
    let freevars = freevars_obj.cast::<PyTuple>()?;
    let mut closure_cells = Vec::with_capacity(freevars.len());
    for name_obj in freevars.iter() {
        let name = name_obj.extract::<String>()?;
        let value = captured_values.get_item(name.as_str())?.ok_or_else(|| {
            PyRuntimeError::new_err(format!(
                "missing captured value for closure freevar {name:?}"
            ))
        })?;
        if is_cell_object(value.as_ptr()) {
            closure_cells.push(value.clone().unbind());
        } else {
            let cell = unsafe { PyCell_New(value.as_ptr()) };
            if cell.is_null() {
                return Err(PyErr::fetch(py));
            }
            closure_cells.push(unsafe { Bound::from_owned_ptr(py, cell) }.unbind());
        }
    }
    let closure = PyTuple::new(py, closure_cells)?;
    let qualname = PyString::new(py, qualname);
    let func = unsafe {
        let ptr = ffi::PyFunction_NewWithQualName(
            code.as_ptr(),
            module_globals.as_ptr(),
            qualname.as_ptr(),
        );
        if ptr.is_null() {
            return Err(PyErr::fetch(py));
        }
        Bound::from_owned_ptr(py, ptr)
    };
    if unsafe { ffi::PyFunction_SetClosure(func.as_ptr(), closure.as_ptr()) } != 0 {
        return Err(PyErr::fetch(py));
    }
    func.setattr(
        "__dp_closure_slot_names__",
        PyTuple::new(py, captured_names)?,
    )?;
    let kwdefaults = PyDict::new(py);
    kwdefaults.set_item("__dp_entry", raw_entry)?;
    if unsafe { ffi::PyFunction_SetKwDefaults(func.as_ptr(), kwdefaults.as_ptr()) } != 0 {
        return Err(PyErr::fetch(py));
    }
    raw_entry.setattr("__dp_public_function__", &func)?;
    Ok(func.into_any())
}

fn apply_function_defaults(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    positional_defaults: Option<&Bound<'_, PyTuple>>,
    kwdefaults: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let defaults_obj = positional_defaults.map_or_else(
        || py.None().into_any(),
        |value| value.clone().into_any().unbind(),
    );
    if unsafe { ffi::PyFunction_SetDefaults(func.as_ptr(), defaults_obj.as_ptr()) } != 0 {
        return Err(PyErr::fetch(py));
    }
    let kwdefaults_obj = kwdefaults.map_or_else(
        || py.None().into_any(),
        |value| value.clone().into_any().unbind(),
    );
    if unsafe { ffi::PyFunction_SetKwDefaults(func.as_ptr(), kwdefaults_obj.as_ptr()) } != 0 {
        return Err(PyErr::fetch(py));
    }
    Ok(())
}

fn instantiate_bb_function(
    py: Python<'_>,
    dp: &Bound<'_, PyModule>,
    module_name: &str,
    function: &BlockPyFunction<CodegenBlockPyPass>,
    captures: &Bound<'_, PyAny>,
    param_defaults: &Bound<'_, PyAny>,
    module_globals: &Bound<'_, PyAny>,
    annotate_fn: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let signature = build_bb_signature(py, function, param_defaults)?;
    let (raw_entry, entry) = instantiate_closure_backed_entry(
        py,
        dp,
        module_name,
        function,
        captures,
        module_globals,
        function.names.display_name.as_str(),
        function.names.qualname.as_str(),
    )?;
    let (positional_defaults, mut kwdefaults) = split_param_defaults(py, function, param_defaults)?;
    if !std::ptr::eq(entry.as_ptr(), raw_entry.as_ptr()) {
        if let Some(kwdefaults) = kwdefaults.as_ref() {
            kwdefaults.set_item("__dp_entry", &raw_entry)?;
        } else {
            let merged = PyDict::new(py);
            merged.set_item("__dp_entry", &raw_entry)?;
            kwdefaults = Some(merged);
        }
    }
    apply_function_defaults(
        py,
        &entry,
        positional_defaults.as_ref(),
        kwdefaults.as_ref(),
    )?;
    entry.setattr("__signature__", signature.bind(py))?;
    update_function_metadata(
        py,
        &entry,
        function.names.qualname.as_str(),
        function.names.display_name.as_str(),
        function.doc.as_deref(),
        annotate_fn,
    )?;
    entry.setattr("__module__", module_name)?;
    Ok(entry.unbind())
}

fn instantiate_closure_backed_entry<'py>(
    py: Python<'py>,
    dp: &Bound<'py, PyModule>,
    module_name: &str,
    function: &BlockPyFunction<CodegenBlockPyPass>,
    captures: &Bound<'py, PyAny>,
    module_globals: &Bound<'py, PyAny>,
    entry_name: &str,
    qualname: &str,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let (captured_names, closure_values) = build_capture_map(py, captures)?;
    let raw_entry = make_lazy_clif_entry(py, dp, entry_name, module_globals)?;
    register_lazy_clif_vectorcall(py, &raw_entry, module_name, function.function_id)?;
    let entry = build_wrapped_entry(
        py,
        dp,
        &raw_entry,
        module_globals,
        qualname,
        &captured_names,
        &closure_values,
    )?;
    Ok((raw_entry, entry))
}

#[pyfunction]
fn make_bb_function(
    py: Python<'_>,
    function_id: usize,
    captures: Py<PyAny>,
    param_defaults: Py<PyAny>,
    module_globals: Py<PyAny>,
    annotate_fn: Py<PyAny>,
) -> PyResult<Py<PyAny>> {
    let dp = import_dp_module(py)?;
    let module_globals = module_globals.bind(py);
    let module_name = resolve_module_name(&module_globals, "function instantiation")?;
    let function = lookup_bb_function(&module_name, function_id, "function instantiation")?;
    instantiate_bb_function(
        py,
        &dp,
        &module_name,
        &function,
        captures.bind(py).as_any(),
        param_defaults.bind(py).as_any(),
        &module_globals,
        annotate_fn.bind(py),
    )
}

#[pyfunction]
fn create_module(py: Python<'_>, source: &str, spec: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let session = soac_eval::CompileSession::new();
    let output: soac_blockpy::LoweringResult<NoopPassTracker> =
        lower_python_to_blockpy(source, session.module_name_gen())
            .map_err(lowering_error_to_pyerr)?;
    SoacExtModule::new(py, spec.bind(py).as_any(), output.codegen_module)
}

#[pyfunction]
fn exec_module(py: Python<'_>, module: Py<PyAny>) -> PyResult<()> {
    let module = module.bind(py);
    let module_globals = module.getattr("__dict__")?;
    SoacExtModule::with_data(module.as_any(), |module_data| {
        let module_name = resolve_module_name(&module_globals, "module execution")?;
        assert_eq!(
            module_name, module_data.module_name,
            "module.__dict__['__name__'] did not match the module spec captured at create_module time"
        );
        let package_name = resolve_module_package(&module_globals, "module execution")?;
        assert_eq!(
            package_name, module_data.package_name,
            "module.__dict__['__package__'] did not match the module spec captured at create_module time"
        );
        register_blockpy_module_plans(&module_name, &module_data.lowered_module)?;
        let function = lookup_module_init_function(&module_data.lowered_module, &module_name)?;
        let dp = import_dp_module(py)?;
        let empty = PyTuple::empty(py);
        let none = py.None();
        let module_init = instantiate_bb_function(
            py,
            &dp,
            &module_name,
            &function,
            empty.as_any(),
            empty.as_any(),
            &module_globals,
            none.bind(py),
        )?;
        module_init.call0(py)?;
        Ok(())
    })
}

#[pyfunction]
#[pyo3(signature = (function_id, resume, module_globals, async_gen=false))]
fn make_bb_generator(
    py: Python<'_>,
    function_id: usize,
    resume: Py<PyAny>,
    module_globals: Py<PyAny>,
    async_gen: bool,
) -> PyResult<Py<PyAny>> {
    let dp = import_dp_module(py)?;
    let module_globals = module_globals.bind(py);
    let operation = if async_gen {
        "async generator construction"
    } else {
        "generator construction"
    };
    let module_name = resolve_module_name(&module_globals, operation)?;
    let function = lookup_bb_function(&module_name, function_id, operation)?;
    let name = function.names.display_name.clone();
    let qualname = function.names.qualname.clone();
    let code = build_generator_code(py, &dp, async_gen, name.as_str(), qualname.as_str())?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("resume", resume.bind(py))?;
    kwargs.set_item("name", name.as_str())?;
    kwargs.set_item("qualname", qualname.as_str())?;
    kwargs.set_item("code", code)?;
    let cls_name = if async_gen {
        "_DpClosureAsyncGenerator"
    } else {
        "_DpClosureGenerator"
    };
    let generator = dp.getattr(cls_name)?.call((), Some(&kwargs))?;
    Ok(generator.unbind())
}

pub(crate) fn add_module_functions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(create_module, module)?)?;
    module.add_function(wrap_pyfunction!(exec_module, module)?)?;
    module.add_function(wrap_pyfunction!(make_bb_function, module)?)?;
    module.add_function(wrap_pyfunction!(make_bb_generator, module)?)?;
    Ok(())
}

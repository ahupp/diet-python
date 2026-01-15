use crate::c_api::callable_from_functiondef;
use crate::module_symbols::ModuleSymbols;
use crate::scope::{Scope, ScopeStack};
use diet_python::min_ast::{Arg, ExprNode, FunctionDef, Number, Parameter, StmtNode};
use pyo3::exceptions::{PyNameError, PySyntaxError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyModule, PyString, PyTuple};

/// Control flow signals used during evaluation.
#[derive(Debug, Clone, Copy)]
pub enum LoopControl {
    Break,
    Continue,
}

/// Non-`Ok` outcomes during evaluation.
#[derive(Debug)]
pub enum PyControl {
    Loop(LoopControl),
    Err(PyErr),
    Return(PyObject),
}

impl From<PyErr> for PyControl {
    fn from(err: PyErr) -> Self {
        PyControl::Err(err)
    }
}

fn evaluate_name(py: Python<'_>, stack: &ScopeStack, name: &str) -> Result<PyObject, PyControl> {
    if let Some(obj) = stack.get_by_name(py, name) {
        Ok(obj)
    } else {
        Err(PyControl::Err(PyErr::new::<PyNameError, _>(format!(
            "name '{}' is not defined",
            name
        ))))
    }
}

fn evaluate_number(py: Python<'_>, n: &Number) -> Result<PyObject, PyControl> {
    let s = match n {
        Number::Int(s) | Number::Float(s) => s,
    };
    Ok(py.eval(s, None, None)?.into())
}

fn assign_by_name(stack: &mut ScopeStack, name: &str, value: PyObject) {
    let idx = stack
        .index_of(name)
        .unwrap_or_else(|| panic!("{} not in scope", name));
    stack.set_by_index(idx, Some(value.as_ptr()));
}

fn evaluate_call(
    py: Python<'_>,
    stack: &mut ScopeStack,
    func: &ExprNode,
    args: &[Arg],
) -> Result<PyObject, PyControl> {
    let func_obj = evaluate_expr(py, stack, func)?;
    let func_ref = func_obj.as_ref(py);
    let mut pos_args = Vec::new();
    let mut kw_args = Vec::new();
    for arg in args {
        match arg {
            Arg::Positional(e) => pos_args.push(evaluate_expr(py, stack, e)?),
            Arg::Keyword { name, value } => kw_args.push((name, evaluate_expr(py, stack, value)?)),
            Arg::Starred(_) | Arg::KwStarred(_) => {
                // TODO: handle starred calls
                return Ok(py.None());
            }
        }
    }
    let args_tuple = PyTuple::new(py, &pos_args);
    let kwargs: Option<&PyDict> = if kw_args.is_empty() {
        None
    } else {
        let dict = PyDict::new(py);
        for (name, val) in kw_args {
            dict.set_item(name, val)?;
        }
        Some(dict)
    };
    let obj = func_ref.call(args_tuple, kwargs)?;
    Ok(obj.into())
}

fn evaluate_expr(
    py: Python<'_>,
    stack: &mut ScopeStack,
    expr: &ExprNode,
) -> Result<PyObject, PyControl> {
    match expr {
        ExprNode::Name(name) => evaluate_name(py, stack, name),
        ExprNode::String(s) => Ok(PyString::new(py, s).into()),
        ExprNode::Number(n) => evaluate_number(py, n),
        ExprNode::Bytes(b) => Ok(PyBytes::new(py, b).into()),
        ExprNode::Tuple(elts) => {
            let mut values = Vec::new();
            for e in elts {
                values.push(evaluate_expr(py, stack, e)?);
            }
            Ok(PyTuple::new(py, values).into())
        }
        ExprNode::Call { func, args } => evaluate_call(py, stack, func, args),
        ExprNode::Await(_) | ExprNode::Yield(_) => Ok(py.None()),
    }
}

fn evaluate_while(
    py: Python<'_>,
    stack: &mut ScopeStack,
    test: &ExprNode,
    body: &[StmtNode],
    orelse: &[StmtNode],
) -> Result<(), PyControl> {
    let mut broken = false;
    while evaluate_expr(py, stack, test)?
        .extract::<bool>(py)
        .map_err(PyControl::Err)?
    {
        match evaluate_stmts(py, stack, body) {
            Ok(_) => {}
            Err(PyControl::Loop(ctrl)) => match ctrl {
                LoopControl::Break => {
                    broken = true;
                    break;
                }
                LoopControl::Continue => continue,
            },
            Err(PyControl::Err(e)) => return Err(PyControl::Err(e)),
            Err(PyControl::Return(v)) => return Err(PyControl::Return(v)),
        }
    }
    if !broken {
        evaluate_stmts(py, stack, orelse)?;
    }
    Ok(())
}

fn evaluate_if(
    py: Python<'_>,
    stack: &mut ScopeStack,
    test: &ExprNode,
    body: &[StmtNode],
    orelse: &[StmtNode],
) -> Result<(), PyControl> {
    let cond_obj = evaluate_expr(py, stack, test)?;
    let cond = cond_obj.extract::<bool>(py).map_err(PyControl::Err)?;
    let branch = if cond { body } else { orelse };
    evaluate_stmts(py, stack, branch)?;
    Ok(())
}

fn evaluate_try(
    py: Python<'_>,
    stack: &mut ScopeStack,
    body: &[StmtNode],
    handler: &Option<Vec<StmtNode>>,
    orelse: &[StmtNode],
    finalbody: &[StmtNode],
) -> Result<(), PyControl> {
    let result = match evaluate_stmts(py, stack, body) {
        Ok(_) => evaluate_stmts(py, stack, orelse),
        Err(PyControl::Err(err)) => {
            if let Some(h_body) = handler {
                evaluate_stmts(py, stack, h_body)?;
                Ok(())
            } else {
                Err(PyControl::Err(err))
            }
        }
        Err(other) => Err(other),
    };
    evaluate_stmts(py, stack, finalbody)?;
    result
}

fn evaluate_stmt(py: Python<'_>, stack: &mut ScopeStack, stmt: &StmtNode) -> Result<(), PyControl> {
    match stmt {
        StmtNode::FunctionDef(func) => {
            let func_ptr = callable_from_functiondef(func);
            if func_ptr.is_null() {
                panic!("failed to create function {}", func.name);
            }
            let func_obj = unsafe { PyObject::from_owned_ptr(py, func_ptr) };
            assign_by_name(stack, &func.name, func_obj);
            Ok(())
        }
        StmtNode::Expr(expr) => {
            evaluate_expr(py, stack, expr)?;
            Ok(())
        }
        StmtNode::Assign { target, value } => {
            let val = evaluate_expr(py, stack, value)?;
            assign_by_name(stack, target, val);
            Ok(())
        }
        StmtNode::Break => Err(PyControl::Loop(LoopControl::Break)),
        StmtNode::Continue => Err(PyControl::Loop(LoopControl::Continue)),
        StmtNode::While { test, body, orelse } => evaluate_while(py, stack, test, body, orelse),
        StmtNode::If { test, body, orelse } => evaluate_if(py, stack, test, body, orelse),
        StmtNode::Try {
            body,
            handler,
            orelse,
            finalbody,
        } => evaluate_try(py, stack, body, handler, orelse, finalbody),
        StmtNode::Return { value } => {
            let ret_obj = if let Some(expr) = value {
                evaluate_expr(py, stack, expr)?
            } else {
                py.None()
            };
            Err(PyControl::Return(ret_obj))
        }
        StmtNode::Delete { .. } | StmtNode::Pass => Ok(()),
    }
}

fn evaluate_stmts(
    py: Python<'_>,
    stack: &mut ScopeStack,
    body: &[StmtNode],
) -> Result<(), PyControl> {
    for stmt in body {
        evaluate_stmt(py, stack, stmt)?;
    }
    Ok(())
}

/// Evaluate a sequence of statements and return the last expression.
///
/// Any loop control signals (`break`/`continue`) reaching this level are
/// considered a bug in the caller and will cause a panic.  Only Python errors
/// are propagated to the caller as `PyErr` values.
pub fn evaluate_module(py: Python<'_>, stack: &mut ScopeStack, body: &[StmtNode]) -> PyResult<()> {
    match evaluate_stmts(py, stack, body) {
        Ok(_) => Ok(()),
        Err(PyControl::Err(e)) => Err(e),
        Err(PyControl::Loop(ctrl)) => panic!("unexpected loop control: {:?}", ctrl),
        Err(PyControl::Return(_)) => Err(PySyntaxError::new_err("return outside function")),
    }
}

/// Evaluate a call from the CPython side for a given function definition.
///
/// Args are provided as a Python tuple and keyword arguments as an optional dict.
pub fn evaluate_call_from_py(
    py: Python<'_>,
    func: &FunctionDef,
    args: &PyTuple,
    kwargs: Option<&PyDict>,
) -> PyResult<PyObject> {
    // TODO: support keyword arguments and defaults
    if let Some(kw) = kwargs {
        if !kw.is_empty() {
            return Err(pyo3::exceptions::PyNotImplementedError::new_err(
                "keyword arguments not supported yet",
            ));
        }
    }

    let mut param_names = Vec::new();
    for p in &func.params {
        match p {
            Parameter::Positional { name, .. } => param_names.push(name.as_str()),
            Parameter::VarArg { .. } | Parameter::KwOnly { .. } | Parameter::KwArg { .. } => {
                return Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    "unsupported parameter type",
                ));
            }
        }
    }

    let symbols = ModuleSymbols::default();
    let mut scope_names = param_names.clone();
    scope_names.push("__dp__");
    scope_names.push("getattr");
    let mut scope = Scope::new(&symbols, &scope_names);

    for (i, name) in param_names.iter().enumerate() {
        let idx = scope.index_of(name).unwrap();
        if i >= args.len() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "missing positional argument",
            ));
        }
        let val = args.get_item(i)?;
        scope.set_by_index(idx, Some(val.as_ptr()));
    }

    let builtins = PyModule::import(py, "builtins")?;
    let dp: PyObject = PyModule::from_code(
        py,
        diet_python::intrinsics::DP_SOURCE,
        "__dp__.py",
        "__dp__",
    )?
    .into();
    let getattr: PyObject = builtins.getattr("getattr")?.into();
    if let Some(idx) = scope.index_of("__dp__") {
        scope.set_by_index(idx, Some(dp.as_ptr()));
    }
    if let Some(idx) = scope.index_of("getattr") {
        scope.set_by_index(idx, Some(getattr.as_ptr()));
    }
    // TODO: add access to globals
    let mut stack = ScopeStack::new(builtins.into(), scope);

    match evaluate_stmts(py, &mut stack, &func.body) {
        Ok(_) => Ok(py.None()),
        Err(PyControl::Return(v)) => Ok(v),
        Err(PyControl::Err(e)) => Err(e),
        Err(PyControl::Loop(ctrl)) => panic!("unexpected loop control: {:?}", ctrl),
    }
}

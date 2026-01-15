use diet_python::transform_min_ast;
use pyo3::prelude::*;
use soac_exec::c_api::callable_from_functiondef;
use diet_python::min_ast::StmtNode;

#[test]
fn call_function_returns_result() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let src = "def identity(a):\n    return a\n";
        let module = transform_min_ast(src, None).unwrap();
        let func = match &module.body[0] {
            StmtNode::FunctionDef(f) => f,
            _ => panic!("expected function def"),
        };
        let ptr = callable_from_functiondef(func);
        assert!(!ptr.is_null());
        let func_obj = unsafe { PyObject::from_owned_ptr(py, ptr) };
        let result = func_obj.call1(py, (5,)).unwrap();
        assert_eq!(result.extract::<i32>(py).unwrap(), 5);
    });
}

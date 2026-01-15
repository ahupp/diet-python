use diet_python::transform_min_ast;
use pyo3::prelude::*;
use soac_exec::module_symbols::module_symbols;
use soac_exec::scope::Scope;

#[test]
fn scope_set_and_get() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let src = "x = 1\ny = 2";
        let module = transform_min_ast(src, None).unwrap();
        let symbols = module_symbols(&module);
        let mut scope = Scope::new(&symbols, &[]);

        // Initially all values are None
        let x_idx = symbols.globals.get("x").unwrap().index;
        let y_idx = symbols.globals.get("y").unwrap().index;
        assert!(scope.get_by_index(x_idx).is_none());
        assert!(scope.get_by_index(y_idx).is_none());

        // Set by index
        let val_x: PyObject = 42.into_py(py);
        scope.set_by_index(x_idx, Some(val_x.as_ptr()));
        assert_eq!(
            unsafe { py.from_borrowed_ptr::<PyAny>(scope.get_by_index(x_idx).unwrap()) }
                .extract::<i32>()
                .unwrap(),
            42
        );

        let val_y: PyObject = 99.into_py(py);
        scope.set_by_index(y_idx, Some(val_y.as_ptr()));
        assert_eq!(
            unsafe { py.from_borrowed_ptr::<PyAny>(scope.get_by_index(y_idx).unwrap()) }
                .extract::<i32>()
                .unwrap(),
            99
        );
    });
}
#[test]
fn index_of_unknown_name() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|_py| {
        let src = "x = 1";
        let module = transform_min_ast(src, None).unwrap();
        let symbols = module_symbols(&module);
        let scope = Scope::new(&symbols, &[]);
        assert!(scope.index_of("z").is_none());
    });
}

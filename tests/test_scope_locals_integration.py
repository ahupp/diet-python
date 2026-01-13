def test_scope_locals_integration(run_integration_module):
    with run_integration_module("scope_locals") as module:
        func_locals = module.function_locals()
        assert "h" in func_locals
        assert "_dp_fn_h" not in func_locals
        del func_locals["h"]
        assert func_locals == {"x": 2, "y": 7, "w": 6}

        class_locals = set(module.class_locals())
        assert "x" not in class_locals
        assert "y" in class_locals

        assert module.class_namespace_overrides_closure() == 43

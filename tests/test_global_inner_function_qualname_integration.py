def test_global_inner_function_qualname(run_integration_module):
    with run_integration_module("global_inner_function_qualname") as module:
        qualname, inner_qualname = module.RESULT
        assert qualname == "inner_global_function"
        assert inner_qualname == "inner_global_function.<locals>.inner_function2"

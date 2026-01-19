def test_lambda_qualname_minimal(run_integration_module):
    with run_integration_module("lambda_qualname_minimal") as module:
        qualname, name = module.RESULT
        assert qualname == "global_function.<locals>.<lambda>"
        assert name == "<lambda>"

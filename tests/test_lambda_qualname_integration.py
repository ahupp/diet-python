def test_lambda_qualname(run_integration_module):
    with run_integration_module("lambda_qualname") as module:
        qualname, name = module.global_function()
        assert qualname == "global_function.<locals>.<lambda>"
        assert name == "<lambda>"

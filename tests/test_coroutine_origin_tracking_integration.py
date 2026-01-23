import diet_import_hook


def test_coroutine_origin_tracking_line(run_integration_module):
    with run_integration_module("coroutine_origin_tracking") as module:
        origin = module.RESULT
        assert origin is not None
        ((filename, lineno, funcname),) = origin
        assert funcname == "a1"

        transformed = diet_import_hook._transform_source(module.__file__)
        target_line = None
        for idx, line in enumerate(transformed.splitlines(), 1):
            if "return corofn$0()" in line:
                target_line = idx
                break
        assert target_line is not None
        assert lineno == target_line

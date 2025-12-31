def test_class_method_import_does_not_break_class_locals(run_integration_module):
    with run_integration_module("class_method_import_shadowing") as module:
        assert module.VALUE == "atexit"
        assert module.CLASS_ATTR == "class"

def test_disable_transform_temp_module(tmp_path, run_integration_module):
    with run_integration_module("disable_transform_temp_module") as module:
        assert module.import_without_transform(tmp_path) is False

def test_skip_outside_repo_transform(tmp_path, run_integration_module):
    with run_integration_module("skip_outside_repo_transform") as module:
        assert module.imported_without_transform(tmp_path) is False

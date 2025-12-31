def test_meta_path_pathfinder_preserved(run_integration_module):
    with run_integration_module("meta_path_pathfinder_preserved") as module:
        assert module.import_with_filtered_meta_path() is True

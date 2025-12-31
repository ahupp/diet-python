def test_dotted_import_alias_rebind(tmp_path, run_integration_module):
    with run_integration_module("dotted_import_alias_rebind") as module:
        from_attr, direct_attr = module.alias_rebind_attrs(tmp_path)
        assert from_attr == "rebound"
        assert direct_attr == "rebound"

def test_pep695_type_aliases(run_integration_module):
    with run_integration_module("pep695_type_aliases") as module:
        assert module.Alias.__value__ is int
        assert module.X.U.__value__ is int
        assert module.X.V.__value__ is module.Alias

        type_param = module.Y.__type_params__[0]
        assert module.Y.V.__type_params__ == ()
        assert module.Y.V.__value__ is type_param
        assert module.Y_HINTS["value"] is type_param

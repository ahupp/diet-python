def test_ast_visit_ellipsis(run_integration_module):
    with run_integration_module("ast_visit_ellipsis") as module:
        assert module.visit_ellipsis() == [("Ellipsis", ...)]

from __future__ import annotations


def test_class_scope_cases(run_integration_module):
    with run_integration_module("class_scope") as module:
        results = module.results

        assert results["class_attr_vs_global"] == ("class", "global", "global")
        assert results["class_global_assignment"] == (
            "class-global",
            None,
            "class-attr",
        )
        assert results["class_method_global_assignment"] == (
            "method-global",
            "method-global",
        )
        assert results["class_method_nonlocal_inner"] == "inner"
        assert results["def_with_inner_class_capture"] == "outer"
        assert results["def_with_inner_class_global_assignment"] == (
            ("outer", None, "class-attr"),
            "class-global",
        )
        assert results["def_with_class_with_method_reads_global"] == (
            "class",
            "outer",
            "outer",
        )
        assert results["def_with_nonlocal_and_inner_class"] == (
            "inner",
            "inner",
        )
        assert results["class_nonlocal_syntaxerror"] is not None
        assert results["class_nonlocal_syntaxerror"]

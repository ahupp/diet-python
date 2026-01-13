from __future__ import annotations


def test_named_expression_cases(run_integration_module):
    with run_integration_module("named_expression_cases") as module:
        assert module.dict_comp_fib() == {
            1: 2,
            2: 3,
            3: 5,
            5: 8,
            8: 13,
            13: 21,
        }
        has_c_before, values, c_value = module.genexp_scope_state()
        assert has_c_before is False
        assert values == [2, 3, 4, 5]
        assert c_value == 5
        assert module.mangled_global_value() == 2

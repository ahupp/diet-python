from tests._integration import transformed_module


def test_genexpr_named_expr_uses_correct_inherited_capture_slots(tmp_path):
    source = """
def genexpr_scope():
    a = 1
    gen = (b := a + i for i in range(2))
    return a, list(gen), b
"""

    with transformed_module(tmp_path, "genexpr_inherited_capture_order", source) as module:
        a, values, b = module.genexpr_scope()
        assert a == 1
        assert values == [1, 2]
        assert b == 2

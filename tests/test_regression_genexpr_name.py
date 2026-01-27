from tests._integration import transformed_module


def test_genexpr_name(tmp_path):
    source = """
def get_name():
    gen = (i for i in ())
    return gen.__name__
"""
    with transformed_module(tmp_path, "genexpr_name_regression", source) as module:
        assert module.get_name() == "<genexpr>"

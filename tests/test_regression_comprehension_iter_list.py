from tests._integration import transformed_module


def test_comprehension_iter_list_literal(tmp_path):
    source = """
def run():
    magic_methods = "m"
    numerics = "n"
    inplace = "i"
    right = "r"
    return {
        "__%s__" % method for method in " ".join([magic_methods, numerics, inplace, right]).split()
    }
"""
    with transformed_module(tmp_path, "comprehension_iter_list", source) as module:
        assert module.run() == {"__m__", "__n__", "__i__", "__r__"}

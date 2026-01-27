from tests._integration import transformed_module


def test_closure_cell_nonlocal(tmp_path):
    source = """

def outer():
    x = 5
    def inner():
        nonlocal x
        x = 2
        return x
    return inner()
"""
    with transformed_module(tmp_path, "closure_cell_nonlocal", source) as module:
        assert module.outer() == 2

from tests._integration import transformed_module


def test_eval_sees_closure_cells(tmp_path):
    source = """

def run():
    d1 = 10
    d2 = 32
    def inner():
        _ = (d1, d2)
        return eval("d1 + d2")
    return inner()
"""
    with transformed_module(tmp_path, "eval_closure", source) as module:
        assert module.run() == 42

from tests._integration import transformed_module


def test_nonlocal_binding_via_del(tmp_path):
    source = """

def outer():
    def gen():
        nonlocal value
        value = 10
        yield
    g = gen()
    next(g)
    assert value == 10
    del value
    return "ok"


def main():
    return outer()
"""
    with transformed_module(tmp_path, "nonlocal_del_binding", source) as module:
        assert module.main() == "ok"

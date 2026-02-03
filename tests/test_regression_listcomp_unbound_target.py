from tests._integration import transformed_module


def test_listcomp_unbound_target_raises(tmp_path):
    source = """

def run():
    l = [None]
    return [1 for (l[0], l) in [[1, 2]]]
"""
    with transformed_module(tmp_path, "listcomp_unbound_target", source) as module:
        try:
            module.run()
        except UnboundLocalError:
            return
        raise AssertionError("Expected UnboundLocalError")

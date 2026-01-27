from tests._integration import transformed_module


def test_for_loop_empty_iterable(tmp_path):
    source = """
def run():
    out = []
    for item in []:
        out.append(item)
    return out
"""
    with transformed_module(tmp_path, "for_loop_empty", source) as module:
        assert module.run() == []

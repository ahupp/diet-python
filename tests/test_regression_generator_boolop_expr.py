from tests._integration import transformed_module


def test_generator_boolop_yield_uses_post_blockpy_lowering(tmp_path):
    source = """
def gen(flag):
    yield flag and 1

def main():
    return list(gen(True)), list(gen(False))
"""
    with transformed_module(tmp_path, "generator_boolop_expr", source) as module:
        assert module.main() == ([1], [False])

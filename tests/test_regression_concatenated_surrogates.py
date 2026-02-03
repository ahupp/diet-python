from tests._integration import transformed_module


def test_concatenated_surrogate_literal(tmp_path):
    source = """

def run():
    s = ('a\\udca7'
         "b")
    return s
"""
    with transformed_module(tmp_path, "concat_surrogates", source) as module:
        assert module.run() == "a\udca7b"

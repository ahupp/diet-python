from tests._integration import transformed_module


def test_fstring_surrogate_escapes_preserved(tmp_path):
    source = """

def run():
    s1 = "X"
    s2 = "Y"
    return f"\\ud83d{s1}\\udc0d{s2}"
"""
    with transformed_module(tmp_path, "fstring_surrogates", source) as module:
        assert module.run() == "\ud83dX\udc0dY"

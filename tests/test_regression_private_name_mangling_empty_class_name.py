from tests._integration import transformed_module


def test_private_name_mangling_empty_class_name(tmp_path):
    source = """
class _:
    def __a(self):
        return "ok"


def run():
    return "__a" in _.__dict__, "___a" in _.__dict__
"""
    with transformed_module(tmp_path, "private_name_mangling_empty_class_name", source) as module:
        assert module.run() == (True, False)

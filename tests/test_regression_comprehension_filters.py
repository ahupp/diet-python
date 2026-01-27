from tests._integration import transformed_module


def test_generator_filter_uses_target_binding(tmp_path):
    source = """
class Field:
    def __init__(self, init, kw_only):
        self.init = init
        self.kw_only = kw_only


def run(fields):
    return tuple(f for f in fields if f.init and not f.kw_only)
"""
    with transformed_module(tmp_path, "comprehension_filters", source) as module:
        fields = [module.Field(True, False), module.Field(True, True)]
        assert module.run(fields) == (fields[0],)

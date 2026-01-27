from tests._integration import transformed_module


def test_nested_class_bound_to_outer_class(tmp_path):
    source = """
class Container:
    class Member:
        pass


def get_member():
    return getattr(Container, "Member", None)
"""
    with transformed_module(tmp_path, "nested_class_binding", source) as module:
        assert module.get_member() is module.Container.Member

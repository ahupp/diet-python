from tests._integration import integration_module


def test_eval_class_getitem_wrapped_as_classmethod(tmp_path):
    source = """
import types


class C:
    def __class_getitem__(self, item):
        return types.GenericAlias(self, item)


def run():
    alias = C[int]
    return (
        alias.__origin__ is C,
        alias.__args__,
        type(C.__dict__["__class_getitem__"]).__name__,
    )
"""
    with integration_module(tmp_path, "eval_class_getitem", source, mode="eval") as module:
        origin_is_c, args, dict_entry_type = module.run()
        assert origin_is_c is True
        assert args == (int,)
        assert dict_entry_type == "classmethod"

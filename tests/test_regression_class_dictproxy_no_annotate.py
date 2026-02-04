from tests._integration import transformed_module


def test_class_dictproxy_omits_dunder_annotate(tmp_path):
    source = """
class C:
    def meth(self):
        pass

def run():
    return "__annotate__" in C.__dict__
"""
    with transformed_module(tmp_path, "class_dictproxy_no_annotate", source) as module:
        assert module.run() is False


def test_class_annotations_use_dunder_annotate_func(tmp_path):
    source = """
from annotationlib import Format

class C:
    value: int

def run():
    return (
        "__annotate__" in C.__dict__,
        "__annotate_func__" in C.__dict__,
        callable(getattr(C, "__annotate__", None)),
        C.__annotate__(Format.VALUE),
    )
"""
    with transformed_module(tmp_path, "class_dictproxy_annotate_func", source) as module:
        has_annotate, has_annotate_func, callable_annotate, annotations = module.run()
        assert has_annotate is False
        assert has_annotate_func is True
        assert callable_annotate is True
        assert annotations == {"value": int}

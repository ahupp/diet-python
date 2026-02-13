from tests._integration import integration_module


def test_eval_annotation_helper_for_loop_mro(tmp_path):
    source = """
import annotationlib

def _make_annotate_function(__class__):
    def __annotate__(format, /):
        cls_annotations = {}
        for base in reversed(__class__.__mro__):
            cls_annotations.update(annotationlib.get_annotations(base, format=format))
        return cls_annotations
    return __annotate__

class Base:
    x: int

class Derived(Base):
    y: str

def run():
    annotate = _make_annotate_function(Derived)
    ann = annotationlib.call_annotate_function(annotate, annotationlib.Format.VALUE)
    return ann["x"], ann["y"]
"""

    with integration_module(
        tmp_path, "eval_annotation_helper_for_loop_mro", source, mode="eval"
    ) as module:
        assert module.run() == (int, str)

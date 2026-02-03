from tests._integration import transformed_module


def test_metaclass_annotate_masking(tmp_path):
    source = """
from annotationlib import Format, get_annotations

class Meta(type):
    a: int

class X(metaclass=Meta):
    pass

class Y(metaclass=Meta):
    b: float


def run():
    return (
        get_annotations(Meta),
        get_annotations(X),
        get_annotations(Y),
        Meta.__annotate__(Format.VALUE),
        X.__annotate__,
        Y.__annotate__(Format.VALUE),
    )
"""
    with transformed_module(tmp_path, "annotationlib_meta", source) as module:
        ann_meta, ann_x, ann_y, meta_annotate, x_annotate, y_annotate = module.run()
        assert ann_meta == {"a": int}
        assert ann_x == {}
        assert ann_y == {"b": float}
        assert meta_annotate == {"a": int}
        assert x_annotate is None
        assert y_annotate == {"b": float}

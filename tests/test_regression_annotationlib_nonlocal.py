from tests._integration import transformed_module


def test_forwardref_nonlocal_annotation_scope(tmp_path):
    source = """
import annotationlib
from annotationlib import Format, ForwardRef


def run():
    class Demo:
        nonlocal sequence_b
        x: sequence_b
        y: sequence_b[int]
    fwdrefs = annotationlib.get_annotations(Demo, format=Format.FORWARDREF)
    ok_types = isinstance(fwdrefs["x"], ForwardRef) and isinstance(fwdrefs["y"], ForwardRef)
    sequence_b = list
    return ok_types, fwdrefs["x"].evaluate(), fwdrefs["y"].evaluate()
"""
    with transformed_module(tmp_path, "annotationlib_nonlocal_scope", source) as module:
        ok_types, x_val, y_val = module.run()
        assert ok_types is True
        assert x_val is list
        assert y_val == list[int]


def test_forwardref_partial_evaluation_cell(tmp_path):
    source = """
import annotationlib
from annotationlib import Format


def run():
    obj = object()
    class RaisesAttributeError:
        attriberr: obj.missing
    ann = annotationlib.get_annotations(RaisesAttributeError, format=Format.FORWARDREF)
    return ann["attriberr"].__forward_arg__
"""
    with transformed_module(tmp_path, "annotationlib_partial_eval_cell", source) as module:
        assert module.run() == "obj.missing"
